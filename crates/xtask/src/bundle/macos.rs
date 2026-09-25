use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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

#[cfg(test)]
pub fn first_app_bundle(bundle_dir: &Path) -> Result<Option<PathBuf>> {
    for bundle_subdir in ["macos", "osx"] {
        let app_bundle_dir = bundle_dir.join(bundle_subdir);
        if let Some(app_path) = first_app_bundle_in_dir(&app_bundle_dir)? {
            return Ok(Some(app_path));
        }
    }

    Ok(None)
}

#[cfg(test)]
fn first_app_bundle_in_dir(app_bundle_dir: &Path) -> Result<Option<PathBuf>> {
    if !app_bundle_dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(app_bundle_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read {}: {err}",
            app_bundle_dir.display()
        ))
    })? {
        let path = entry
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to read entry under {}: {err}",
                    app_bundle_dir.display()
                ))
            })?
            .path();
        if path.is_dir() && path.extension().and_then(OsStr::to_str) == Some("app") {
            return Ok(Some(path));
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

    let tmp_dir = env::temp_dir().join(format!(
        "xtask-bundle-assets-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| XtaskError::msg(format!("failed to read system time: {err}")))?
            .as_millis()
    ));
    fs::create_dir_all(&tmp_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create temp dir {}: {err}",
            tmp_dir.display()
        ))
    })?;

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
        let _ = fs::remove_dir_all(&tmp_dir);
        warn!(error = %err, "actool 编译失败，跳过 Liquid Glass 图标注入（保留普通图标）");
        return Ok(());
    }

    let assets_car = actool_output_dir.join("Assets.car");
    if !assets_car.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
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

    let _ = fs::remove_dir_all(&tmp_dir);
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
    use super::{bundle_info_plist_overrides, find_app_bundle, first_app_bundle};
    use crate::bundle::settings::resolve_bundle_localizations;
    use crate::error::Result;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let id = NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "xtask-macos-bundle-{suffix}-{}-{id}",
                std::process::id(),
            ));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn icon_catalog_requires_an_explicit_default_for_multiple_themes() -> Result<()> {
        let temp = TestDir::new()?;
        let icons = temp.path.join("build-assets/icon");
        let classic = icons.join("Classic.icon");
        fs::create_dir_all(&classic)?;
        assert_eq!(
            super::find_liquid_glass_icon_dirs(&temp.path)?.unwrap().0,
            classic
        );
        fs::create_dir_all(icons.join("Color.icon"))?;
        assert!(super::find_liquid_glass_icon_dirs(&temp.path).is_err());
        fs::write(icons.join("default-icon"), "Classic\n")?;
        let (default, all) = super::find_liquid_glass_icon_dirs(&temp.path)?.unwrap();
        assert_eq!(default, classic);
        assert_eq!(all.len(), 2);
        fs::write(icons.join("default-icon"), "Missing")?;
        assert!(super::find_liquid_glass_icon_dirs(&temp.path).is_err());
        Ok(())
    }

    #[test]
    fn first_app_bundle_prefers_macos_directory() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let macos_app = temp_dir.path.join("macos/Jaco.app");
        let osx_app = temp_dir.path.join("osx/Legacy.app");
        fs::create_dir_all(&macos_app)?;
        fs::create_dir_all(&osx_app)?;

        let app_path = first_app_bundle(&temp_dir.path)?;

        assert_eq!(app_path, Some(macos_app));
        Ok(())
    }

    #[test]
    fn first_app_bundle_falls_back_to_osx_directory() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let osx_app = temp_dir.path.join("osx/Jaco.app");
        fs::create_dir_all(&osx_app)?;

        let app_path = first_app_bundle(&temp_dir.path)?;

        assert_eq!(app_path, Some(osx_app));
        Ok(())
    }

    #[test]
    fn find_app_bundle_uses_product_name() -> Result<()> {
        let temp_dir = TestDir::new()?;
        fs::create_dir_all(temp_dir.path.join("macos/Jaco.app"))?;
        let feiwen_app = temp_dir.path.join("macos/Feiwen.app");
        fs::create_dir_all(&feiwen_app)?;

        let app_path = find_app_bundle(&temp_dir.path, "Feiwen")?;

        assert_eq!(app_path, Some(feiwen_app));
        Ok(())
    }

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
