use crate::error::{Result, XtaskError};
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tauri_bundler::{AppCategory, BundleSettings, PackageSettings};

#[derive(Deserialize)]
struct Manifest {
    package: ManifestPackage,
}

#[derive(Deserialize)]
struct ManifestPackage {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    authors: Option<Vec<String>>,
    #[serde(rename = "default-run", default)]
    default_run: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(rename = "license-file", default)]
    license_file: Option<String>,
    #[serde(default)]
    metadata: Option<ManifestMetadata>,
}

#[derive(Deserialize)]
struct ManifestMetadata {
    #[serde(default)]
    bundle: Option<ManifestBundle>,
}

#[derive(Deserialize)]
struct ManifestBundle {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    icon: Option<Vec<String>>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
}

pub fn read_bundle_settings(manifest_path: &Path) -> Result<(PackageSettings, BundleSettings)> {
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve manifest dir for {}",
            manifest_path.display()
        ))
    })?;
    let manifest: Manifest = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse {}: {err}",
            manifest_path.display()
        ))
    })?;

    let Manifest { package } = manifest;
    let ManifestPackage {
        name,
        version,
        description,
        homepage,
        authors,
        default_run,
        license,
        license_file,
        metadata,
    } = package;
    let bundle = metadata.and_then(|metadata| metadata.bundle);

    let product_name = bundle
        .as_ref()
        .and_then(|bundle| bundle.name.clone())
        .unwrap_or(name);
    let description = description.unwrap_or_default();

    let mut bundle_settings = BundleSettings::default();
    if let Some(bundle) = bundle {
        bundle_settings.identifier = bundle.identifier;
        bundle_settings.publisher = bundle.publisher.or_else(|| {
            bundle_settings
                .identifier
                .as_deref()
                .and_then(infer_publisher_from_identifier)
        });
        bundle_settings.icon = bundle
            .icon
            .map(|paths| resolve_manifest_paths(manifest_dir, paths));
        bundle_settings.category = bundle
            .category
            .as_deref()
            .map(parse_app_category)
            .transpose()?;
        bundle_settings.short_description = bundle.short_description;
        bundle_settings.long_description = bundle.long_description;
        bundle_settings.homepage = bundle.homepage.or_else(|| homepage.clone());
    }

    if bundle_settings.homepage.is_none() {
        bundle_settings.homepage = homepage.clone();
    }

    bundle_settings.license = license;
    bundle_settings.license_file =
        license_file.map(|path| resolve_manifest_path(manifest_dir, &path));
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

fn parse_app_category(category: &str) -> Result<AppCategory> {
    category.parse().map_err(|suggestion| {
        let message = match suggestion {
            Some(suggestion) => {
                format!("invalid bundle category `{category}`, did you mean `{suggestion}`?")
            }
            None => format!("invalid bundle category `{category}`"),
        };
        XtaskError::msg(message)
    })
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
            let path = std::env::temp_dir().join(format!(
                "xtask-bundle-settings-{suffix}-{}",
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
category = "DeveloperTool"
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
        assert_eq!(bundle_settings.category, Some(AppCategory::DeveloperTool));
        assert_eq!(bundle_settings.windows.icon_path, expected_ico);

        Ok(())
    }
}
