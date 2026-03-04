use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use tauri_bundler::{BundleBinary, BundleSettings, PackageSettings, PackageType, SettingsBuilder};
use tracing::info;
use walkdir::WalkDir;

use crate::cli::BundleAiChatWindowsArgs;
use crate::cmd::{run_cmd, run_cmd_os, run_cmd_program_os};
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::{Result, XtaskError};
use crate::manifest::get_main_binary_name;

type TomlTable = toml::value::Table;

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

    let bundles = tauri_bundler::bundle_project(&settings).map_err(|err| {
        XtaskError::msg(format!("failed to bundle MSI with tauri-bundler: {err}"))
    })?;

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
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve manifest dir for {}",
            manifest_path.display()
        ))
    })?;
    let root: toml::Value = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse {}: {err}",
            manifest_path.display()
        ))
    })?;

    let package = required_table(&root, &["package"], "manifest missing [package] table")?;
    let bundle = nested_table(package, &["metadata", "bundle"]);

    let homepage = optional_string(package, "homepage").map(ToString::to_string);
    let product_name = bundle
        .and_then(|bundle| optional_string(bundle, "name"))
        .or_else(|| optional_string(package, "name"))
        .ok_or_else(|| XtaskError::msg("failed to resolve product name"))?
        .to_string();
    let version = optional_string(package, "version")
        .ok_or_else(|| XtaskError::msg("failed to resolve package version"))?
        .to_string();
    let description = optional_string(package, "description")
        .unwrap_or_default()
        .to_string();
    let authors = optional_string_array(package, "authors");
    let default_run = optional_string(package, "default-run").map(ToString::to_string);

    let mut bundle_settings = BundleSettings::default();
    if let Some(bundle) = bundle {
        bundle_settings.identifier = optional_string(bundle, "identifier").map(ToString::to_string);
        bundle_settings.publisher = optional_string(bundle, "publisher")
            .map(ToString::to_string)
            .or_else(|| {
                bundle_settings
                    .identifier
                    .as_deref()
                    .and_then(infer_publisher_from_identifier)
            });
        bundle_settings.icon = optional_string_array(bundle, "icon")
            .map(|paths| resolve_manifest_paths(manifest_dir, paths));
        bundle_settings.short_description =
            optional_string(bundle, "short_description").map(ToString::to_string);
        bundle_settings.long_description =
            optional_string(bundle, "long_description").map(ToString::to_string);
        bundle_settings.homepage = optional_string(bundle, "homepage")
            .map(ToString::to_string)
            .or(homepage.clone());
    }

    if bundle_settings.homepage.is_none() {
        bundle_settings.homepage = homepage.clone();
    }

    bundle_settings.license = optional_string(package, "license").map(ToString::to_string);
    bundle_settings.license_file = optional_string(package, "license-file")
        .map(|path| resolve_manifest_path(manifest_dir, path));
    sync_windows_icon_path(&mut bundle_settings);

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

fn required_table<'a>(
    root: &'a toml::Value,
    path: &[&str],
    missing_message: &str,
) -> Result<&'a TomlTable> {
    nested_value(root, path)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| XtaskError::msg(missing_message))
}

fn nested_table<'a>(table: &'a TomlTable, path: &[&str]) -> Option<&'a TomlTable> {
    let value = table.get(path.first().copied()?)?;
    nested_value(value, &path[1..]).and_then(toml::Value::as_table)
}

fn nested_value<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn optional_string<'a>(table: &'a TomlTable, key: &str) -> Option<&'a str> {
    table.get(key).and_then(toml::Value::as_str)
}

fn optional_string_array(table: &TomlTable, key: &str) -> Option<Vec<String>> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
}

fn resolve_manifest_paths(manifest_dir: &Path, paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| resolve_manifest_path(manifest_dir, &path))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn resolve_manifest_path(manifest_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

#[allow(deprecated)]
fn sync_windows_icon_path(bundle_settings: &mut BundleSettings) {
    if bundle_settings.windows.icon_path != default_windows_icon_path() {
        return;
    }

    let Some(icon_path) = bundle_settings
        .icon
        .as_ref()
        .and_then(|paths| {
            paths.iter().find(|path| {
                Path::new(path)
                    .extension()
                    .and_then(OsStr::to_str)
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
            })
        })
        .map(PathBuf::from)
    else {
        return;
    };

    bundle_settings.windows.icon_path = icon_path;
}

fn default_windows_icon_path() -> PathBuf {
    PathBuf::from("icons/icon.ico")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = env::temp_dir().join(format!(
                "xtask-bundle-windows-{suffix}-{}",
                std::process::id()
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

    #[allow(deprecated)]
    #[test]
    fn read_bundle_settings_resolves_relative_bundle_paths() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = temp_dir.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "ai-chat"
version = "0.1.0"
license-file = "LICENSE"

[package.metadata.bundle]
name = "AI Chat"
identifier = "top.sushao.ai-chat"
icon = [
  "build-assets/icon/app-icon.ico",
  "build-assets/icon/app-icon.png",
]
"#,
        )?;

        let (_, bundle_settings) = read_bundle_settings(&manifest_path)?;
        let expected_ico = temp_dir.path.join("build-assets/icon/app-icon.ico");
        let expected_png = temp_dir.path.join("build-assets/icon/app-icon.png");

        assert_eq!(
            bundle_settings.icon,
            Some(vec![
                expected_ico.to_string_lossy().into_owned(),
                expected_png.to_string_lossy().into_owned(),
            ])
        );
        assert_eq!(
            bundle_settings.license_file,
            Some(temp_dir.path.join("LICENSE"))
        );
        assert_eq!(bundle_settings.windows.icon_path, expected_ico);

        Ok(())
    }
}
