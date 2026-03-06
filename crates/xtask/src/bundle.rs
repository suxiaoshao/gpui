use std::env;
use std::path::{Path, PathBuf};

pub mod common;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod settings;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::cli::BundleAiChatArgs;
use crate::cmd::run_cmd;
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::Result;
use crate::manifest::get_main_binary_name;
use tauri_bundler::{BundleBinary, PackageType, SettingsBuilder};
use tracing::{info, warn};

pub fn run(args: BundleAiChatArgs) -> Result<()> {
    let app_dir = ai_chat_dir()?;
    let workspace_dir = workspace_root()?;
    let bundle_dir = workspace_dir.join("target/release/bundle");

    validate_platform_args(&args);
    prepare_platform_bundle(&app_dir)?;

    run_cmd(
        "cargo",
        &["build", "-p", "ai-chat", "--release"],
        Some(&workspace_dir),
    )?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;
    let (package_settings, bundle_settings) = settings::read_bundle_settings(&manifest_path)?;

    let out_dir = bundle_out_dir(&workspace_dir, &main_bin_name)?;
    info!(bundle_out_dir = %out_dir.display(), "using bundle output dir");

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

    finalize_platform_bundle(&args, &app_dir, &bundle_dir, &out_dir, bundles)?;

    info!(bundle_dir = %bundle_dir.display(), "打包完成");
    Ok(())
}

fn validate_platform_args(args: &BundleAiChatArgs) {
    #[cfg(not(target_os = "windows"))]
    if args.install || args.arch.is_some() || args.target.is_some() {
        warn!("--install/--arch/--target are only used on Windows and will be ignored");
    }

    #[cfg(target_os = "windows")]
    if args.arch.is_some() || args.target.is_some() {
        warn!("--arch/--target are ignored on Windows; cargo build uses default host target");
    }
}

fn prepare_platform_bundle(app_dir: &Path) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    common::prepare_bundle_icons(app_dir)?;

    Ok(())
}

fn finalize_platform_bundle(
    _args: &BundleAiChatArgs,
    app_dir: &Path,
    bundle_dir: &Path,
    _out_dir: &Path,
    _bundles: Vec<tauri_bundler::Bundle>,
) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if let Some(app_path) = macos::first_app_bundle(bundle_dir)? {
            macos::inject_liquid_glass_icon(app_dir, &app_path)?;
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
        let _ = (_args, app_dir, bundle_dir, _out_dir, _bundles);
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
fn bundle_out_dir(workspace_dir: &Path, main_bin_name: &str) -> Result<PathBuf> {
    let target_root = windows::resolve_target_root(workspace_dir);
    windows::prepare_windows_bundle_staging(&target_root, main_bin_name)
}

#[cfg(not(target_os = "windows"))]
fn bundle_out_dir(workspace_dir: &Path, _main_bin_name: &str) -> Result<PathBuf> {
    Ok(workspace_dir.join("target/release"))
}

#[cfg(target_os = "windows")]
fn is_windows_artifact(path: &Path) -> bool {
    use std::ffi::OsStr;

    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}
