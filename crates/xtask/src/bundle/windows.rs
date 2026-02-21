use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::info;

use walkdir::WalkDir;

use crate::cli::BundleAiChatWindowsArgs;
use crate::cmd::{ensure_command_installed, run_cmd, run_cmd_os, run_cmd_program_os};
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::{Result, XtaskError};
use crate::manifest::{get_main_binary_name, sanitize_identifier};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

pub fn run(args: BundleAiChatWindowsArgs) -> Result<()> {
    ensure_command_installed("cargo-bundle", "cargo install cargo-bundle")?;

    let app_dir = ai_chat_dir()?;
    let workspace_dir = workspace_root()?;

    let target = if let Some(target) = args.target {
        target
    } else {
        let detected_arch = args.arch.unwrap_or_else(detect_arch);
        match detected_arch.as_str() {
            "x86_64" => "x86_64-pc-windows-msvc".to_string(),
            "aarch64" => "aarch64-pc-windows-msvc".to_string(),
            other => {
                return Err(XtaskError::msg(format!(
                    "unsupported architecture: {other} (expected x86_64 or aarch64)"
                )));
            }
        }
    };

    info!(target, "using target");
    run_cmd("rustup", &["target", "add", &target], None)?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;

    let build_start = SystemTime::now();
    let mut manifest_original = None;

    if main_bin_name.contains('-') {
        let original = fs::read_to_string(&manifest_path).map_err(|err| {
            XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
        })?;
        let mut doc = original.parse::<DocumentMut>().map_err(|err| {
            XtaskError::msg(format!(
                "failed to parse {}: {err}",
                manifest_path.display()
            ))
        })?;
        let temporary_bin_name = format!("_bundle_{}_msi", sanitize_identifier(&main_bin_name));
        apply_msi_manifest_patch(&mut doc, &temporary_bin_name)?;
        let patched = doc.to_string();

        fs::write(&manifest_path, patched).map_err(|err| {
            XtaskError::msg(format!(
                "failed to patch {}: {err}",
                manifest_path.display()
            ))
        })?;
        manifest_original = Some(original);

        info!(
            "Applied temporary MSI binary-name workaround for cargo-bundle issue #77: {} -> {}",
            main_bin_name, temporary_bin_name
        );
    }

    let bundle_result = run_cmd(
        "cargo",
        &[
            "bundle",
            "--format",
            "msi",
            "--release",
            "--target",
            &target,
        ],
        Some(&app_dir),
    );

    if let Some(original) = manifest_original {
        fs::write(&manifest_path, original).map_err(|err| {
            XtaskError::msg(format!(
                "failed to restore {}: {err}",
                manifest_path.display()
            ))
        })?;
    }

    bundle_result?;

    let bundle_dir = workspace_dir
        .join("target")
        .join(&target)
        .join("release")
        .join("bundle");
    if !bundle_dir.exists() {
        return Err(XtaskError::msg(format!(
            "未找到打包目录: {}",
            bundle_dir.display()
        )));
    }

    let mut artifacts = find_windows_artifacts(&bundle_dir)?;
    let fresh_threshold = build_start
        .checked_sub(Duration::from_secs(2))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let fresh: Vec<_> = artifacts
        .iter()
        .filter(|path| {
            path.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|m| m >= fresh_threshold)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if !fresh.is_empty() {
        artifacts = fresh;
    }

    if artifacts.is_empty() {
        info!(bundle_dir = %bundle_dir.display(), "bundle completed but no .msi/.exe artifacts found");
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

fn detect_arch() -> String {
    match env::consts::ARCH {
        "x86_64" => "x86_64".to_string(),
        "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn install_windows_artifact(artifacts: &[PathBuf]) -> Result<()> {
    if !cfg!(target_os = "windows") {
        info!("当前系统不是 Windows，跳过安装步骤");
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

fn apply_msi_manifest_patch(doc: &mut DocumentMut, temporary_bin_name: &str) -> Result<()> {
    let package = doc
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| XtaskError::msg("manifest missing [package] table"))?;
    if package.get("autobins").is_none() {
        package["autobins"] = value(false);
    }

    let mut bin_table = Table::new();
    bin_table["name"] = value(temporary_bin_name);
    bin_table["path"] = value("src/main.rs");

    match doc.get_mut("bin") {
        Some(item) => {
            let bins = item
                .as_array_of_tables_mut()
                .ok_or_else(|| XtaskError::msg("manifest [bin] is not an array of tables"))?;
            bins.push(bin_table);
        }
        None => {
            let mut bins = ArrayOfTables::new();
            bins.push(bin_table);
            doc["bin"] = Item::ArrayOfTables(bins);
        }
    }

    Ok(())
}
