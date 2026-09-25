use std::env;
#[cfg(any(target_os = "windows", test))]
use std::ffi::OsStr;
#[cfg(target_os = "linux")]
use std::fs;
use std::path::{Path, PathBuf};

pub mod common;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod settings;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::cli::BundleArgs;
use crate::cmd::run_cmd;
use crate::context::workspace_root;
use crate::error::Result;
use crate::manifest::get_main_binary_name;
use tauri_bundler::{BundleBinary, PackageType, SettingsBuilder};
use tracing::info;
#[cfg(not(target_os = "windows"))]
use tracing::warn;

pub fn run(args: BundleArgs) -> Result<()> {
    let workspace_dir = workspace_root()?;
    let app_dir = workspace_dir.join("app").join(args.app.app_dir_name());
    let bundle_dir = resolve_target_root(&workspace_dir).join("release/bundle");

    validate_platform_args(&args);
    let bundle_icon_assets = prepare_platform_bundle(&app_dir)?;

    run_cmd(
        "cargo",
        &["build", "-p", args.app.package_name(), "--release"],
        Some(&workspace_dir),
    )?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;
    let (package_settings, mut bundle_settings, _localizations) =
        settings::read_bundle_settings(&manifest_path)?;
    bundle_icon_assets.apply_to_bundle_settings(&mut bundle_settings);
    let product_name = package_settings.product_name.clone();

    let out_dir = bundle_out_dir(&workspace_dir, &main_bin_name, args.app)?;
    info!(bundle_out_dir = %out_dir.display(), "using bundle output dir");
    #[cfg(target_os = "macos")]
    let bundle_settings = {
        let mut bundle_settings = bundle_settings;
        macos::prepare_bundle_settings(&mut bundle_settings, &_localizations)?;
        bundle_settings
    };

    let mut settings_builder = SettingsBuilder::new()
        .project_out_directory(&out_dir)
        .package_types(default_package_types())
        .package_settings(package_settings)
        .bundle_settings(bundle_settings)
        .binaries(vec![BundleBinary::new(main_bin_name, true)]);

    if let Ok(local_tools_dir) = env::var("TAURI_BUNDLER_TOOLS_DIR") {
        settings_builder = settings_builder.local_tools_directory(local_tools_dir);
        info!("using local tauri-bundler tools dir from TAURI_BUNDLER_TOOLS_DIR");
    }

    let settings = settings_builder.build().map_err(|err| {
        crate::error::XtaskError::msg(format!("failed to build tauri bundle settings: {err}"))
    })?;

    let bundles = tauri_bundler::bundle_project(&settings).map_err(|err| {
        crate::error::XtaskError::msg(format!("failed to bundle app with tauri-bundler: {err}"))
    })?;

    finalize_platform_bundle(
        &args,
        &app_dir,
        &bundle_dir,
        &out_dir,
        &product_name,
        bundles,
        &bundle_icon_assets,
    )?;

    info!(app = args.app.package_name(), bundle_dir = %bundle_dir.display(), "打包完成");
    Ok(())
}

fn resolve_target_root(workspace_dir: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                workspace_dir.join(path)
            }
        })
        .unwrap_or_else(|| workspace_dir.join("target"))
}

fn validate_platform_args(_args: &BundleArgs) {
    #[cfg(not(target_os = "windows"))]
    if _args.install {
        warn!("--install is only used on Windows and will be ignored");
    }
}

fn prepare_platform_bundle(_app_dir: &Path) -> Result<common::BundleIconAssets> {
    common::prepare_bundle_icons(_app_dir)
}

fn finalize_platform_bundle(
    _args: &BundleArgs,
    _app_dir: &Path,
    _bundle_dir: &Path,
    _out_dir: &Path,
    _product_name: &str,
    _bundles: Vec<tauri_bundler::Bundle>,
    _bundle_icon_assets: &common::BundleIconAssets,
) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if let Some(app_path) = macos::find_app_bundle(_bundle_dir, _product_name)? {
            macos::inject_liquid_glass_icon(_app_dir, &app_path, _bundle_icon_assets)?;
            macos::finalize_ad_hoc_codesign(&app_path)?;
        } else {
            warn!("未找到 .app 包，跳过 Liquid Glass 图标注入");
        }
    }

    #[cfg(target_os = "windows")]
    {
        let mut artifacts: Vec<PathBuf> = _bundles
            .into_iter()
            .flat_map(|bundle| bundle.bundle_paths.into_iter())
            .filter(|path| is_windows_artifact(path))
            .collect();

        artifacts.sort();
        if artifacts.is_empty() {
            artifacts = windows::find_windows_artifacts(&_out_dir.join("bundle"))?;
        }

        if artifacts.is_empty() {
            info!("bundle completed but no .msi/.exe artifacts found");
        } else {
            info!("bundle completed. artifacts:");
            for item in &artifacts {
                info!(artifact = %item.display());
            }

            if _args.install {
                windows::install_windows_artifact(&artifacts)?;
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (
            _args,
            _app_dir,
            _bundle_dir,
            _out_dir,
            _product_name,
            _bundles,
            _bundle_icon_assets,
        );
    }

    Ok(())
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn preferred_windows_artifact(artifacts: &[PathBuf]) -> Option<&PathBuf> {
    artifacts
        .iter()
        .find(|path| {
            path.extension()
                .and_then(OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
                && path
                    .file_stem()
                    .and_then(OsStr::to_str)
                    .and_then(|stem| stem.rsplit('_').next())
                    .is_some_and(|locale| locale.eq_ignore_ascii_case("en-US"))
        })
        .or_else(|| {
            artifacts.iter().min_by_key(|path| {
                if path
                    .extension()
                    .and_then(OsStr::to_str)
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
                {
                    0
                } else {
                    1
                }
            })
        })
}

#[cfg(target_os = "macos")]
fn default_package_types() -> Vec<PackageType> {
    vec![PackageType::MacOsBundle]
}

#[cfg(target_os = "linux")]
fn default_package_types() -> Vec<PackageType> {
    vec![PackageType::Deb]
}

#[cfg(target_os = "windows")]
fn default_package_types() -> Vec<PackageType> {
    vec![PackageType::WindowsMsi]
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn default_package_types() -> Vec<PackageType> {
    vec![]
}

#[cfg(target_os = "windows")]
fn bundle_out_dir(
    workspace_dir: &Path,
    main_bin_name: &str,
    app: crate::cli::BundleApp,
) -> Result<PathBuf> {
    let target_root = resolve_target_root(workspace_dir);
    let _ = app;
    windows::prepare_windows_bundle_staging(&target_root, main_bin_name)
}

#[cfg(target_os = "linux")]
fn bundle_out_dir(
    workspace_dir: &Path,
    main_bin_name: &str,
    _app: crate::cli::BundleApp,
) -> Result<PathBuf> {
    prepare_linux_bundle_staging(workspace_dir, main_bin_name)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn bundle_out_dir(
    workspace_dir: &Path,
    _main_bin_name: &str,
    _app: crate::cli::BundleApp,
) -> Result<PathBuf> {
    Ok(resolve_target_root(workspace_dir).join("release"))
}

#[cfg(target_os = "linux")]
fn prepare_linux_bundle_staging(workspace_dir: &Path, main_bin_name: &str) -> Result<PathBuf> {
    let target_root = resolve_target_root(workspace_dir);
    let source = target_root.join("release").join(main_bin_name);
    if !source.is_file() {
        return Err(crate::error::XtaskError::msg(format!(
            "failed to find built Linux binary {}",
            source.display()
        )));
    }
    let staging = target_root.join("xtask-bundle/release");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|err| {
            crate::error::XtaskError::msg(format!(
                "failed to clean Linux bundle staging {}: {err}",
                staging.display()
            ))
        })?;
    }
    fs::create_dir_all(&staging)?;
    fs::copy(&source, staging.join(main_bin_name))?;
    Ok(staging)
}

#[cfg(target_os = "windows")]
fn is_windows_artifact(path: &Path) -> bool {
    use std::ffi::OsStr;

    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::preferred_windows_artifact;
    use std::path::PathBuf;

    #[test]
    fn localized_install_prefers_english_and_falls_back_to_an_msi() {
        let localized = vec![
            PathBuf::from("Gupi_0.1.0_x64_de-DE.msi"),
            PathBuf::from("Gupi_0.1.0_x64_en-US.msi"),
            PathBuf::from("Gupi_0.1.0_x64.exe"),
        ];
        assert_eq!(
            preferred_windows_artifact(&localized).and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("Gupi_0.1.0_x64_en-US.msi"))
        );

        let fallback = vec![
            PathBuf::from("Gupi_0.1.0_x64.exe"),
            PathBuf::from("Gupi_0.1.0_x64_zh-CN.msi"),
        ];
        assert_eq!(
            preferred_windows_artifact(&fallback).and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("Gupi_0.1.0_x64_zh-CN.msi"))
        );
    }
}
