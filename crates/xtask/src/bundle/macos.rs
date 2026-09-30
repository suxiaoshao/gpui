use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::bundle::{common::BundleIconAssets, settings::BundleLocalization};
use crate::cmd::{command_exists, run_cmd_os};
use crate::error::{Result, XtaskError};
use tauri_bundler::{BundleSettings, PlistKind};

pub fn prepare_bundle_settings(
    bundle_settings: &mut BundleSettings,
    localizations: &[BundleLocalization],
) -> Result<()> {
    bundle_settings.macos.info_plist = Some(PlistKind::Plist(
        bundle_info_plist_overrides(localizations).into(),
    ));
    Ok(())
}

pub fn find_app_bundle(bundle_dir: &Path, product_name: &str) -> Result<Option<PathBuf>> {
    let app_name = format!("{product_name}.app");
    for bundle_subdir in ["macos", "osx"] {
        let app_bundle_dir = bundle_dir.join(bundle_subdir);
        let app_path = app_bundle_dir.join(&app_name);
        if app_path.is_dir() {
            return Ok(Some(app_path));
        }
    }

    Ok(None)
}

pub fn inject_liquid_glass_icon(
    app_dir: &Path,
    app_path: &Path,
    bundle_icon_assets: &BundleIconAssets,
) -> Result<()> {
    let Some((icon_dir, icon_dirs)) = find_liquid_glass_icon_dirs(app_dir)? else {
        warn!(app_dir = %app_dir.display(), "未找到 .icon 目录，跳过 Liquid Glass 图标注入");
        return Ok(());
    };
    let icon_name = icon_dir
        .file_stem()
        .and_then(|x| x.to_str())
        .filter(|x| !x.is_empty())
        .unwrap_or("Icon");

    if !command_exists("xcrun") {
        warn!("未找到 xcrun，跳过 Liquid Glass 图标注入（保留普通图标）");
        return Ok(());
    }

    let staging = tempfile::Builder::new()
        .prefix("xtask-bundle-assets-")
        .tempdir()
        .map_err(|err| {
            XtaskError::msg(format!("failed to create icon compiler directory: {err}"))
        })?;
    let tmp_dir = staging.path();
    let actool_source_dir = tmp_dir.join("source");
    let actool_output_dir = tmp_dir.join("output");
    fs::create_dir_all(&actool_output_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create actool output dir {}: {err}",
            actool_output_dir.display()
        ))
    })?;
    let staged_icons = icon_dirs
        .iter()
        .map(|icon| {
            stage_liquid_glass_icon_dir(
                icon,
                &bundle_icon_assets.source_base_icon(),
                &actool_source_dir,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let actool_plist = actool_output_dir.join("assetcatalog_generated_info.plist");
    let mut actool_args: Vec<&OsStr> = vec![OsStr::new("actool")];
    actool_args.extend(staged_icons.iter().map(|icon| icon.as_os_str()));
    actool_args.extend([
        OsStr::new("--compile"),
        actool_output_dir.as_os_str(),
        OsStr::new("--output-format"),
        OsStr::new("human-readable-text"),
        OsStr::new("--notices"),
        OsStr::new("--warnings"),
        OsStr::new("--errors"),
        OsStr::new("--output-partial-info-plist"),
        actool_plist.as_os_str(),
        OsStr::new("--app-icon"),
        OsStr::new(icon_name),
        OsStr::new("--include-all-app-icons"),
        OsStr::new("--enable-on-demand-resources"),
        OsStr::new("NO"),
        OsStr::new("--development-region"),
        OsStr::new("en"),
        OsStr::new("--target-device"),
        OsStr::new("mac"),
        OsStr::new("--platform"),
        OsStr::new("macosx"),
        OsStr::new("--minimum-deployment-target"),
        OsStr::new("26.0"),
    ]);

    let actool_result = run_cmd_os("xcrun", &actool_args, None);

    if let Err(err) = actool_result {
        warn!(error = %err, "actool 编译失败，跳过 Liquid Glass 图标注入（保留普通图标）");
        return Ok(());
    }

    let assets_car = actool_output_dir.join("Assets.car");
    if !assets_car.exists() {
        warn!("未生成 Assets.car，跳过 Liquid Glass 图标注入（保留普通图标）");
        return Ok(());
    }

    let target_assets = app_path.join("Contents/Resources/Assets.car");
    fs::copy(&assets_car, &target_assets).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy {} to {}: {err}",
            assets_car.display(),
            target_assets.display()
        ))
    })?;

    let plist = app_path.join("Contents/Info.plist");
    update_bundle_icon_name(&plist, icon_name)?;

    info!(app_path = %app_path.display(), "已注入 Liquid Glass 图标");
    Ok(())
}

/// Applies the development signature after every bundle mutation has completed.
///
/// Runtime staging and rpath rewriting invalidate any signature produced by the
/// bundler. Call this only after icon injection and native runtime staging.
pub(crate) fn finalize_ad_hoc_codesign(app_path: &Path) -> Result<()> {
    if !command_exists("codesign") {
        return Err(XtaskError::msg(
            "codesign is required to finalize a macOS application bundle",
        ));
    }

    let args: Vec<&OsStr> = vec![
        OsStr::new("--force"),
        OsStr::new("--deep"),
        OsStr::new("--sign"),
        OsStr::new("-"),
        app_path.as_os_str(),
    ];
    run_cmd_os("codesign", &args, None)?;

    let verify_args: Vec<&OsStr> = vec![
        OsStr::new("--verify"),
        OsStr::new("--deep"),
        OsStr::new("--strict"),
        app_path.as_os_str(),
    ];
    run_cmd_os("codesign", &verify_args, None)
}

fn stage_liquid_glass_icon_dir(
    source_icon_dir: &Path,
    source_base_icon: &Path,
    stage_root: &Path,
) -> Result<PathBuf> {
    let icon_dir_name = source_icon_dir.file_name().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve .icon dir name for {}",
            source_icon_dir.display()
        ))
    })?;
    let staged_icon_dir = stage_root.join(icon_dir_name);
    copy_dir_all(source_icon_dir, &staged_icon_dir)?;

    let staged_assets_dir = staged_icon_dir.join("Assets");
    fs::create_dir_all(&staged_assets_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create Liquid Glass icon assets dir {}: {err}",
            staged_assets_dir.display()
        ))
    })?;

    let staged_layer = staged_assets_dir.join("app-icon-liquid-glass.png");
    fs::copy(source_base_icon, &staged_layer).map_err(|err| {
        XtaskError::msg(format!(
            "failed to stage Liquid Glass icon layer {} from {}: {err}",
            staged_layer.display(),
            source_base_icon.display()
        ))
    })?;

    Ok(staged_icon_dir)
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create directory {}: {err}",
            target.display()
        ))
    })?;

    for entry in fs::read_dir(source)
        .map_err(|err| XtaskError::msg(format!("failed to read {}: {err}", source.display())))?
    {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to read entry under {}: {err}",
                source.display()
            ))
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to copy {} to {}: {err}",
                    source_path.display(),
                    target_path.display()
                ))
            })?;
        }
    }

    Ok(())
}

fn find_liquid_glass_icon_dirs(app_dir: &Path) -> Result<Option<(PathBuf, Vec<PathBuf>)>> {
    let icon_root = app_dir.join("build-assets/icon");
    if !icon_root.exists() {
        return Ok(None);
    }

    let mut icon_dirs = Vec::new();
    for entry in fs::read_dir(&icon_root)
        .map_err(|err| XtaskError::msg(format!("failed to read {}: {err}", icon_root.display())))?
    {
        let path = entry
            .map_err(|err| XtaskError::msg(format!("failed to read icon dir entry: {err}")))?
            .path();
        if path.is_dir() && path.extension().and_then(OsStr::to_str) == Some("icon") {
            icon_dirs.push(path);
        }
    }

    icon_dirs.sort();
    if icon_dirs.is_empty() {
        return Ok(None);
    }
    let selection = icon_root.join("default-icon");
    let default = if selection.exists() {
        let name = fs::read_to_string(&selection)?;
        icon_dirs
            .iter()
            .find(|path| path.file_stem().and_then(OsStr::to_str) == Some(name.trim()))
            .cloned()
            .ok_or_else(|| XtaskError::msg("default-icon must name an existing .icon directory"))?
    } else if icon_dirs.len() == 1 {
        icon_dirs[0].clone()
    } else {
        return Err(XtaskError::msg(
            "multiple .icon directories require build-assets/icon/default-icon",
        ));
    };
    Ok(Some((default, icon_dirs)))
}

fn update_bundle_icon_name(plist_path: &Path, icon_name: &str) -> Result<()> {
    let mut value = plist::Value::from_file(plist_path)?;
    let dict = value.as_dictionary_mut().ok_or_else(|| {
        XtaskError::msg(format!(
            "unexpected plist root type for {}: expected dictionary",
            plist_path.display()
        ))
    })?;
    dict.insert(
        "CFBundleIconName".to_string(),
        plist::Value::String(icon_name.to_string()),
    );
    value.to_file_xml(plist_path)?;
    Ok(())
}

fn bundle_info_plist_overrides(localizations: &[BundleLocalization]) -> plist::Dictionary {
    let mut dict = plist::Dictionary::new();
    dict.insert(
        "CFBundleDevelopmentRegion".to_string(),
        plist::Value::String("en".to_string()),
    );
    dict.insert(
        "CFBundleAllowMixedLocalizations".to_string(),
        plist::Value::Boolean(true),
    );
    dict.insert(
        "CFBundleLocalizations".to_string(),
        plist::Value::Array(
            localizations
                .iter()
                .map(|localization| {
                    plist::Value::String(localization.bundle_locale_tag.to_string())
                })
                .collect(),
        ),
    );
    dict
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::settings::resolve_bundle_localizations;

    #[test]
    fn plist_overrides_include_bundle_localizations() {
        let localizations = resolve_bundle_localizations(None).unwrap();
        let dict = bundle_info_plist_overrides(&localizations);

        assert_eq!(
            dict.get("CFBundleDevelopmentRegion"),
            Some(&plist::Value::String("en".to_string()))
        );
        assert_eq!(
            dict.get("CFBundleAllowMixedLocalizations"),
            Some(&plist::Value::Boolean(true))
        );
        assert_eq!(
            dict.get("CFBundleLocalizations"),
            Some(&plist::Value::Array(vec![
                plist::Value::String("en".to_string()),
                plist::Value::String("zh_CN".to_string()),
            ]))
        );
    }
}
