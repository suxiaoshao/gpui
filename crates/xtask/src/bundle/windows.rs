use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::info;
use walkdir::WalkDir;

use crate::cmd::{run_cmd_os, run_cmd_program_os};
use crate::error::{Result, XtaskError};

pub(crate) fn resolve_target_root(workspace_dir: &Path) -> PathBuf {
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

pub(crate) fn prepare_windows_bundle_staging(
    target_root: &Path,
    main_bin_name: &str,
) -> Result<PathBuf> {
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

pub(crate) fn install_windows_artifact(artifacts: &[PathBuf]) -> Result<()> {
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

pub(crate) fn find_windows_artifacts(bundle_dir: &Path) -> Result<Vec<PathBuf>> {
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
