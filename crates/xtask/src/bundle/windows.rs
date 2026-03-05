use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use tauri_bundler::{BundleBinary, PackageType, SettingsBuilder};
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::bundle::settings::read_bundle_settings;
use crate::cli::BundleAiChatArgs;
use crate::cmd::{run_cmd, run_cmd_os, run_cmd_program_os};
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::{Result, XtaskError};
use crate::manifest::get_main_binary_name;

pub fn run(args: BundleAiChatArgs) -> Result<()> {
    let app_dir = ai_chat_dir()?;
    let workspace_dir = workspace_root()?;
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

    if args.arch.is_some() || args.target.is_some() {
        warn!("--arch/--target are ignored on Windows; cargo build uses default host target");
    }

    run_cmd(
        "cargo",
        &["build", "-p", "ai-chat", "--release"],
        Some(&workspace_dir),
    )?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;
    let (package_settings, bundle_settings) = read_bundle_settings(&manifest_path)?;

    let out_dir = prepare_windows_bundle_staging(&target_root, &main_bin_name)?;
    info!(bundle_out_dir = %out_dir.display(), "using isolated bundle staging dir");

    let mut settings_builder = SettingsBuilder::new()
        .project_out_directory(&out_dir)
        .package_types(vec![PackageType::WindowsMsi])
        .package_settings(package_settings)
        .bundle_settings(bundle_settings)
        .binaries(vec![BundleBinary::new(main_bin_name, true)]);

    if let Ok(local_tools_dir) = env::var("TAURI_BUNDLER_TOOLS_DIR") {
        settings_builder = settings_builder.local_tools_directory(local_tools_dir);
        info!("using local tauri-bundler tools dir from TAURI_BUNDLER_TOOLS_DIR");
    }

    let settings = settings_builder
        .build()
        .map_err(|err| XtaskError::msg(format!("failed to build tauri bundle settings: {err}")))?;

    let bundles = tauri_bundler::bundle_project(&settings)
        .map_err(|err| XtaskError::msg(format!("failed to bundle MSI with tauri-bundler: {err}")))?;

    let mut artifacts: Vec<PathBuf> = bundles
        .into_iter()
        .flat_map(|bundle| bundle.bundle_paths.into_iter())
        .filter(|path| {
            path.extension()
                .and_then(OsStr::to_str)
                .map(|ext| ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        })
        .collect();

    artifacts.sort();
    if artifacts.is_empty() {
        let bundle_dir = out_dir.join("bundle");
        artifacts = find_windows_artifacts(&bundle_dir)?;
    }

    if artifacts.is_empty() {
        info!("bundle completed but no .msi/.exe artifacts found");
        return Ok(());
    }

    info!("bundle completed. artifacts:");
    for item in &artifacts {
        info!(artifact = %item.display());
    }

    if args.install {
        install_windows_artifact(&artifacts)?;
    }

    Ok(())
}

fn prepare_windows_bundle_staging(target_root: &Path, main_bin_name: &str) -> Result<PathBuf> {
    let build_out_dir = target_root.join("release");
    let staging_out_dir = target_root.join("xtask-bundle").join("release");

    if staging_out_dir.exists() {
        fs::remove_dir_all(&staging_out_dir).map_err(|err| {
            XtaskError::msg(format!(
                "failed to clean staging dir {}: {err}",
                staging_out_dir.display()
            ))
        })?;
    }

    fs::create_dir_all(&staging_out_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create staging dir {}: {err}",
            staging_out_dir.display()
        ))
    })?;

    let main_exe = build_out_dir.join(format!("{main_bin_name}.exe"));
    let main_no_ext = build_out_dir.join(main_bin_name);
    let main_source = if main_exe.exists() {
        main_exe
    } else if main_no_ext.exists() {
        main_no_ext
    } else {
        return Err(XtaskError::msg(format!(
            "failed to find built binary in {} (expected {} or {})",
            build_out_dir.display(),
            build_out_dir.join(format!("{main_bin_name}.exe")).display(),
            build_out_dir.join(main_bin_name).display()
        )));
    };

    let main_filename = main_source
        .file_name()
        .ok_or_else(|| XtaskError::msg("failed to resolve built binary file name"))?;
    fs::copy(&main_source, staging_out_dir.join(main_filename)).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy {} to {}: {err}",
            main_source.display(),
            staging_out_dir.display()
        ))
    })?;

    let webview2_loader = build_out_dir.join("WebView2Loader.dll");
    if webview2_loader.exists() {
        fs::copy(&webview2_loader, staging_out_dir.join("WebView2Loader.dll")).map_err(|err| {
            XtaskError::msg(format!(
                "failed to copy {} to {}: {err}",
                webview2_loader.display(),
                staging_out_dir.display()
            ))
        })?;
    }

    Ok(staging_out_dir)
}

fn install_windows_artifact(artifacts: &[PathBuf]) -> Result<()> {
    if !cfg!(target_os = "windows") {
        info!("current OS is not Windows, skipping installer launch");
        return Ok(());
    }

    let installer = artifacts
        .iter()
        .min_by_key(|path| {
            if path.extension().and_then(OsStr::to_str) == Some("msi") {
                0
            } else {
                1
            }
        })
        .ok_or_else(|| XtaskError::msg("no installer artifact found"))?;

    info!(installer = %installer.display(), "installing artifact");
    if installer.extension().and_then(OsStr::to_str) == Some("msi") {
        let args: Vec<&OsStr> = vec![OsStr::new("/i"), installer.as_os_str()];
        run_cmd_os("msiexec.exe", &args, None)?;
    } else {
        let args: Vec<&OsStr> = Vec::new();
        run_cmd_program_os(installer.as_os_str(), &args, None)?;
    }

    Ok(())
}

fn find_windows_artifacts(bundle_dir: &Path) -> Result<Vec<PathBuf>> {
    if !bundle_dir.exists() {
        return Ok(Vec::new());
    }

    let mut artifacts = Vec::new();
    for entry in WalkDir::new(bundle_dir).into_iter() {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!("failed to walk {}: {err}", bundle_dir.display()))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(ext) = path.extension().and_then(OsStr::to_str) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("exe") {
            artifacts.push(path.to_path_buf());
        }
    }

    artifacts.sort();
    Ok(artifacts)
}