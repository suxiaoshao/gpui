use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use tauri_bundler::{
    BundleBinary, BundleSettings, PackageSettings, PackageType, SettingsBuilder,
};
use tracing::info;
use walkdir::WalkDir;

use crate::cli::BundleAiChatWindowsArgs;
use crate::cmd::{run_cmd, run_cmd_os, run_cmd_program_os};
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::{Result, XtaskError};
use crate::manifest::get_main_binary_name;

pub fn run(args: BundleAiChatWindowsArgs) -> Result<()> {
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

    run_cmd(
        "cargo",
        &["build", "-p", "ai-chat", "--release", "--target", &target],
        Some(&workspace_dir),
    )?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;

    let (package_settings, bundle_settings) = read_bundle_settings(&manifest_path)?;

    let out_dir = workspace_dir.join("target").join(&target).join("release");
    let mut settings_builder = SettingsBuilder::new()
        .project_out_directory(&out_dir)
        .package_types(vec![PackageType::WindowsMsi])
        .package_settings(package_settings)
        .bundle_settings(bundle_settings)
        .binaries(vec![BundleBinary::new(main_bin_name, true)])
        .target(target.clone());

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

fn detect_arch() -> String {
    match env::consts::ARCH {
        "x86_64" => "x86_64".to_string(),
        "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn read_bundle_settings(manifest_path: &Path) -> Result<(PackageSettings, BundleSettings)> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|err| XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display())))?;
    let root: toml::Value = toml::from_str(&content)
        .map_err(|err| XtaskError::msg(format!("failed to parse {}: {err}", manifest_path.display())))?;

    let package = root
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| XtaskError::msg("manifest missing [package] table"))?;

    let product_name = package
        .get("metadata")
        .and_then(toml::Value::as_table)
        .and_then(|m| m.get("bundle"))
        .and_then(toml::Value::as_table)
        .and_then(|b| b.get("name"))
        .and_then(toml::Value::as_str)
        .or_else(|| package.get("name").and_then(toml::Value::as_str))
        .ok_or_else(|| XtaskError::msg("failed to resolve product name"))?
        .to_string();

    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| XtaskError::msg("failed to resolve package version"))?
        .to_string();

    let description = package
        .get("description")
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_string();

    let homepage = package
        .get("homepage")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    let authors = package
        .get("authors")
        .and_then(toml::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());

    let default_run = package
        .get("default-run")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    let mut bundle_settings = BundleSettings::default();
    let bundle = package
        .get("metadata")
        .and_then(toml::Value::as_table)
        .and_then(|m| m.get("bundle"))
        .and_then(toml::Value::as_table);

    if let Some(bundle) = bundle {
        bundle_settings.identifier = bundle
            .get("identifier")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string);

        bundle_settings.publisher = bundle
            .get("publisher")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string)
            .or_else(|| {
                bundle_settings
                    .identifier
                    .as_deref()
                    .and_then(infer_publisher_from_identifier)
            });

        bundle_settings.icon = bundle
            .get("icon")
            .and_then(toml::Value::as_array)
            .map(|icons| {
                icons
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|icons| !icons.is_empty());
        if let Some(icon_path) = bundle_settings
            .icon
            .as_ref()
            .and_then(|icons| icons.iter().find(|path| path.to_ascii_lowercase().ends_with(".ico")))
        {
            let icon_abs = manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(icon_path);
            bundle_settings.windows.icon_path = icon_abs;
        }

        bundle_settings.short_description = bundle
            .get("short_description")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string);

        bundle_settings.long_description = bundle
            .get("long_description")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string);

        bundle_settings.homepage = bundle
            .get("homepage")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string)
            .or(homepage.clone());
    }

    if bundle_settings.homepage.is_none() {
        bundle_settings.homepage = homepage.clone();
    }

    bundle_settings.license = package
        .get("license")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    bundle_settings.license_file = package
        .get("license-file")
        .and_then(toml::Value::as_str)
        .map(PathBuf::from);

    let package_settings = PackageSettings {
        product_name,
        version,
        description,
        homepage,
        authors,
        default_run,
    };

    Ok((package_settings, bundle_settings))
}

fn infer_publisher_from_identifier(identifier: &str) -> Option<String> {
    let mut parts = identifier.split('.');
    parts.next()?;
    parts
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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
        let entry = entry
            .map_err(|err| XtaskError::msg(format!("failed to walk {}: {err}", bundle_dir.display())))?;
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
