use std::env;
#[cfg(target_os = "linux")]
use std::fs;
use std::path::{Path, PathBuf};

pub mod common;
pub mod gstreamer;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod settings;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::cli::BundleArgs;
use crate::cmd::run_cmd_with_env;
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
    let bundle_dir = workspace_dir.join("target/release/bundle");

    validate_platform_args(&args);
    let bundle_icon_assets = prepare_platform_bundle(&app_dir)?;

    let build_environment =
        gstreamer::release_build_environment(args.app, &workspace_dir, &app_dir)?;
    run_cmd_with_env(
        "cargo",
        &["build", "-p", args.app.package_name(), "--release"],
        Some(&workspace_dir),
        &build_environment,
    )?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;
    let (package_settings, mut bundle_settings) = settings::read_bundle_settings(&manifest_path)?;
    bundle_icon_assets.apply_to_bundle_settings(&mut bundle_settings);
    let product_name = package_settings.product_name.clone();

    let out_dir = bundle_out_dir(&workspace_dir, &main_bin_name, args.app)?;
    info!(bundle_out_dir = %out_dir.display(), "using bundle output dir");
    let runtime_resources = prepare_gstreamer_bundle(args.app, &app_dir, &out_dir, &main_bin_name)?;
    merge_bundle_resources(&mut bundle_settings, runtime_resources);

    #[cfg(target_os = "macos")]
    let bundle_settings = {
        let mut bundle_settings = bundle_settings;
        macos::prepare_bundle_settings(&mut bundle_settings)?;
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
            gstreamer::stage_macos_runtime(_args.app, _app_dir, &app_path)?;
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
    let target_root = windows::resolve_target_root(workspace_dir);
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
    Ok(workspace_dir.join("target/release"))
}

#[cfg(target_os = "linux")]
fn prepare_linux_bundle_staging(workspace_dir: &Path, main_bin_name: &str) -> Result<PathBuf> {
    let target_root = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                workspace_dir.join(path)
            }
        })
        .unwrap_or_else(|| workspace_dir.join("target"));
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

fn merge_bundle_resources(
    settings: &mut tauri_bundler::BundleSettings,
    resources: std::collections::HashMap<String, String>,
) {
    if resources.is_empty() {
        return;
    }
    settings
        .resources_map
        .get_or_insert_with(Default::default)
        .extend(resources);
}

fn prepare_gstreamer_bundle(
    _app: crate::cli::BundleApp,
    _app_dir: &Path,
    _out_dir: &Path,
    _main_bin_name: &str,
) -> Result<std::collections::HashMap<String, String>> {
    #[cfg(target_os = "windows")]
    {
        return gstreamer::stage_windows_runtime(_app, _app_dir, _out_dir);
    }
    #[cfg(target_os = "linux")]
    {
        return gstreamer::stage_linux_runtime(
            _app,
            _app_dir,
            _out_dir,
            &_out_dir.join(_main_bin_name),
        );
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = (_app, _app_dir, _out_dir, _main_bin_name);
        Ok(Default::default())
    }
}

#[cfg(target_os = "windows")]
fn is_windows_artifact(path: &Path) -> bool {
    use std::ffi::OsStr;

    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}
