//! App-local GStreamer runtime manifest validation and staging.
//!
//! The manifest is deliberately an allow-list.  A bundle never discovers
//! libraries or plugins by walking an SDK directory: every copied file, plugin
//! element, and licence must be declared by `app/http-client`.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::cli::BundleApp;
use crate::error::{Result, XtaskError};

const HTTP_CLIENT_MANIFEST: &str = "build-assets/gstreamer/runtime-manifest.toml";
const RUNTIME_DIRECTORY_ENV: &str = "GPUI_GSTREAMER_RUNTIME_DIR";
const GST_INSPECT_ENV: &str = "GPUI_GST_INSPECT";
const PKG_CONFIG_ENV: &str = "GPUI_PKG_CONFIG";

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    version: String,
    notices: String,
    platform: Vec<PlatformRuntime>,
}

#[derive(Debug, Deserialize)]
struct PlatformRuntime {
    target: String,
    deployment: Deployment,
    #[serde(default)]
    minimum_version: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    package: Vec<String>,
    #[serde(default)]
    files: Vec<RuntimeFile>,
    #[serde(default)]
    plugin: Vec<PluginContract>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Deployment {
    PrivateBundle,
    SystemPackages,
}

#[derive(Debug, Deserialize)]
struct RuntimeFile {
    source: String,
    destination: String,
    license: String,
}

#[derive(Debug, Deserialize)]
struct PluginContract {
    element: String,
    plugin: String,
    license: String,
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn configure_linux_bundle(
    app: BundleApp,
    app_dir: &Path,
    bundle_settings: &mut tauri_bundler::BundleSettings,
) -> Result<()> {
    if app != BundleApp::HttpClient {
        return Ok(());
    }

    let manifest = load_manifest(&app_dir.join(HTTP_CLIENT_MANIFEST))?;
    let runtime = manifest.platform_for("x86_64-unknown-linux-gnu")?;
    if runtime.deployment != Deployment::SystemPackages {
        return Err(XtaskError::msg(
            "HTTP Client Linux GStreamer runtime must use deployment = \"system-packages\"",
        ));
    }

    let mut dependencies = bundle_settings.deb.depends.take().unwrap_or_default();
    for package in &runtime.package {
        if !dependencies.iter().any(|dependency| dependency == package) {
            dependencies.push(package.clone());
        }
    }
    bundle_settings.deb.depends = Some(dependencies);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn stage_macos_runtime(app: BundleApp, app_dir: &Path, app_bundle: &Path) -> Result<()> {
    if app != BundleApp::HttpClient {
        return Ok(());
    }

    stage_private_runtime(
        app_dir,
        "aarch64-apple-darwin",
        &app_bundle.join("Contents/Frameworks/gstreamer"),
    )
}

#[cfg(target_os = "windows")]
pub(crate) fn stage_windows_runtime(
    app: BundleApp,
    app_dir: &Path,
    staging_dir: &Path,
) -> Result<()> {
    if app != BundleApp::HttpClient {
        return Ok(());
    }

    stage_private_runtime(
        app_dir,
        "x86_64-pc-windows-msvc",
        &staging_dir.join("gstreamer"),
    )
}

pub(crate) fn verify_runtime_manifest(manifest_path: &Path, inspect: bool) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    if !inspect {
        return Ok(());
    }

    let target = current_target()?;
    let runtime = manifest.platform_for(target)?;
    let command = gst_inspect_command();
    for plugin in &runtime.plugin {
        let status = Command::new(&command)
            .arg(&plugin.element)
            .status()
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to execute `{}` while checking GStreamer element `{}`: {err}",
                    Path::new(&command).display(),
                    plugin.element
                ))
            })?;
        if !status.success() {
            return Err(XtaskError::msg(format!(
                "required GStreamer element `{}` from plugin `{}` is unavailable",
                plugin.element, plugin.plugin
            )));
        }
    }

    Ok(())
}

/// Verifies that the host development SDK is discoverable without requiring a
/// release runtime manifest. Release packaging remains guarded by
/// [`verify_runtime_manifest`] and staging.
pub(crate) fn verify_sdk(minimum_version: &str) -> Result<()> {
    let pkg_config = env::var_os(PKG_CONFIG_ENV).unwrap_or_else(|| "pkg-config".into());
    let gst_inspect = gst_inspect_command();
    verify_sdk_with_commands(minimum_version, &pkg_config, &gst_inspect)
}

fn verify_sdk_with_commands(
    minimum_version: &str,
    pkg_config: &std::ffi::OsStr,
    gst_inspect: &std::ffi::OsStr,
) -> Result<()> {
    let minimum_version = minimum_version.trim();
    if minimum_version.is_empty() {
        return Err(XtaskError::msg(
            "GStreamer SDK minimum version must not be empty",
        ));
    }

    let version_check = format!("--atleast-version={minimum_version}");
    let status = Command::new(pkg_config)
        .args([version_check.as_str(), "gstreamer-1.0"])
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute `{}` while checking for GStreamer SDK >= {minimum_version}: {err}",
                Path::new(pkg_config).display(),
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK discovered by `{}` does not satisfy >= {minimum_version}",
            Path::new(pkg_config).display(),
        )));
    }

    let status = Command::new(gst_inspect)
        .arg("--version")
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute `{}` while checking the GStreamer SDK tools: {err}",
                Path::new(gst_inspect).display(),
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK inspector `{}` did not report a version",
            Path::new(gst_inspect).display(),
        )));
    }

    Ok(())
}

fn gst_inspect_command() -> std::ffi::OsString {
    env::var_os(GST_INSPECT_ENV).unwrap_or_else(|| "gst-inspect-1.0".into())
}

fn current_target() -> Result<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Ok("aarch64-apple-darwin")
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Ok("x86_64-pc-windows-msvc")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Ok("x86_64-unknown-linux-gnu")
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    {
        Err(XtaskError::msg(
            "GStreamer manifest inspection is unsupported on this host target",
        ))
    }
}

fn stage_private_runtime(app_dir: &Path, target: &str, destination_root: &Path) -> Result<()> {
    let runtime_dir = env::var_os(RUNTIME_DIRECTORY_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| {
            XtaskError::msg(format!(
                "{RUNTIME_DIRECTORY_ENV} is required to stage the GStreamer runtime for `{target}`"
            ))
        })?;

    stage_private_runtime_from_directory(app_dir, target, destination_root, &runtime_dir)
}

fn stage_private_runtime_from_directory(
    app_dir: &Path,
    target: &str,
    destination_root: &Path,
    runtime_dir: &Path,
) -> Result<()> {
    let manifest_path = app_dir.join(HTTP_CLIENT_MANIFEST);
    let manifest = load_manifest(&manifest_path)?;
    let runtime = manifest.platform_for(target)?;
    if runtime.deployment != Deployment::PrivateBundle {
        return Err(XtaskError::msg(format!(
            "GStreamer target `{target}` must use deployment = \"private-bundle\""
        )));
    }

    let runtime_dir = fs::canonicalize(runtime_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve GStreamer runtime directory {}: {err}",
            runtime_dir.display()
        ))
    })?;
    if !runtime_dir.is_dir() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime directory {} is not a directory",
            runtime_dir.display()
        )));
    }

    if destination_root.exists() {
        fs::remove_dir_all(destination_root).map_err(|err| {
            XtaskError::msg(format!(
                "failed to remove previous staged GStreamer runtime {}: {err}",
                destination_root.display()
            ))
        })?;
    }
    fs::create_dir_all(destination_root).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create GStreamer runtime destination {}: {err}",
            destination_root.display()
        ))
    })?;

    for file in &runtime.files {
        let source_relative = checked_relative_path(&file.source, "runtime file source")?;
        let source = runtime_dir.join(source_relative);
        let source = fs::canonicalize(&source).map_err(|err| {
            XtaskError::msg(format!(
                "required GStreamer runtime file {} is missing: {err}",
                source.display()
            ))
        })?;
        if !source.starts_with(&runtime_dir) || !source.is_file() {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime file {} is outside the declared runtime root or is not a regular file",
                source.display()
            )));
        }

        let destination = destination_root.join(checked_relative_path(
            &file.destination,
            "runtime file destination",
        )?);
        if !destination.starts_with(destination_root) {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime destination {} escapes {}",
                destination.display(),
                destination_root.display()
            )));
        }
        let parent = destination.parent().ok_or_else(|| {
            XtaskError::msg(format!(
                "failed to resolve parent directory for {}",
                destination.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            XtaskError::msg(format!(
                "failed to create GStreamer runtime directory {}: {err}",
                parent.display()
            ))
        })?;
        fs::copy(&source, &destination).map_err(|err| {
            XtaskError::msg(format!(
                "failed to stage GStreamer runtime file {} to {}: {err}",
                source.display(),
                destination.display()
            ))
        })?;
    }

    let notice_source = app_dir.join(checked_relative_path(&manifest.notices, "notices")?);
    if !notice_source.is_file() {
        return Err(XtaskError::msg(format!(
            "required GStreamer notices file {} is missing",
            notice_source.display()
        )));
    }
    fs::copy(
        &notice_source,
        destination_root.join("THIRD_PARTY_NOTICES.md"),
    )
    .map_err(|err| {
        XtaskError::msg(format!(
            "failed to stage GStreamer notices from {}: {err}",
            notice_source.display()
        ))
    })?;
    fs::copy(
        &manifest_path,
        destination_root.join("runtime-manifest.toml"),
    )
    .map_err(|err| {
        XtaskError::msg(format!(
            "failed to stage GStreamer runtime manifest from {}: {err}",
            manifest_path.display()
        ))
    })?;

    Ok(())
}

impl RuntimeManifest {
    fn platform_for(&self, target: &str) -> Result<&PlatformRuntime> {
        self.platform
            .iter()
            .find(|platform| platform.target == target)
            .ok_or_else(|| {
                XtaskError::msg(format!(
                    "GStreamer runtime manifest has no platform entry for `{target}`"
                ))
            })
    }
}

fn load_manifest(path: &Path) -> Result<RuntimeManifest> {
    let content = fs::read_to_string(path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read GStreamer runtime manifest {}: {err}",
            path.display()
        ))
    })?;
    let manifest: RuntimeManifest = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse GStreamer runtime manifest {}: {err}",
            path.display()
        ))
    })?;
    validate_manifest(&manifest, path)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &RuntimeManifest, path: &Path) -> Result<()> {
    if manifest.version.trim().is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} has an empty version",
            path.display()
        )));
    }
    checked_relative_path(&manifest.notices, "notices")?;
    if manifest.platform.is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} has no platform entries",
            path.display()
        )));
    }

    let mut targets = HashSet::new();
    for runtime in &manifest.platform {
        if runtime.target.trim().is_empty() || !targets.insert(&runtime.target) {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime manifest {} has an empty or duplicate target `{}`",
                path.display(),
                runtime.target
            )));
        }
        validate_runtime(runtime, path)?;
    }
    Ok(())
}

fn validate_runtime(runtime: &PlatformRuntime, path: &Path) -> Result<()> {
    let mut plugin_elements = HashSet::new();
    for plugin in &runtime.plugin {
        if plugin.element.trim().is_empty()
            || plugin.plugin.trim().is_empty()
            || plugin.license.trim().is_empty()
            || !plugin_elements.insert(&plugin.element)
        {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime manifest {} has an incomplete or duplicate plugin entry for `{}` on `{}`",
                path.display(),
                plugin.element,
                runtime.target
            )));
        }
    }
    if runtime.plugin.is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} has no plugin contract for `{}`",
            path.display(),
            runtime.target
        )));
    }

    match runtime.deployment {
        Deployment::PrivateBundle => {
            let source_url = runtime.source_url.as_deref().unwrap_or_default();
            if !source_url.starts_with("https://") {
                return Err(XtaskError::msg(format!(
                    "GStreamer runtime manifest {} needs an HTTPS source_url for `{}`",
                    path.display(),
                    runtime.target
                )));
            }
            let sha256 = runtime.sha256.as_deref().unwrap_or_default();
            if sha256.len() != 64
                || !sha256.bytes().all(|byte| {
                    byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte.is_ascii_hexdigit())
                })
            {
                return Err(XtaskError::msg(format!(
                    "GStreamer runtime manifest {} needs a lowercase 64-character sha256 for `{}`",
                    path.display(),
                    runtime.target
                )));
            }
            if !runtime.package.is_empty() || runtime.files.is_empty() {
                return Err(XtaskError::msg(format!(
                    "GStreamer private-bundle target `{}` in {} must have files and no system packages",
                    runtime.target,
                    path.display()
                )));
            }

            let mut destinations = HashSet::new();
            for file in &runtime.files {
                checked_relative_path(&file.source, "runtime file source")?;
                checked_relative_path(&file.destination, "runtime file destination")?;
                if file.license.trim().is_empty() || !destinations.insert(&file.destination) {
                    return Err(XtaskError::msg(format!(
                        "GStreamer runtime manifest {} has an incomplete or duplicate runtime file `{}` for `{}`",
                        path.display(),
                        file.destination,
                        runtime.target
                    )));
                }
            }
        }
        Deployment::SystemPackages => {
            if runtime
                .minimum_version
                .as_deref()
                .is_none_or(|version| version.trim().is_empty())
                || runtime.package.is_empty()
                || runtime.source_url.is_some()
                || runtime.sha256.is_some()
                || !runtime.files.is_empty()
            {
                return Err(XtaskError::msg(format!(
                    "GStreamer system-packages target `{}` in {} must declare minimum_version and packages only",
                    runtime.target,
                    path.display()
                )));
            }
            if runtime
                .package
                .iter()
                .any(|package| package.trim().is_empty())
            {
                return Err(XtaskError::msg(format!(
                    "GStreamer system-packages target `{}` in {} contains an empty package name",
                    runtime.target,
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

fn checked_relative_path<'a>(value: &'a str, field: &str) -> Result<&'a Path> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(XtaskError::msg(format!(
            "GStreamer {field} `{value}` must be a non-empty relative path"
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{
        BundleApp, configure_linux_bundle, load_manifest, stage_private_runtime_from_directory,
    };
    use crate::error::Result;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri_bundler::BundleSettings;

    static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let id = NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "xtask-gstreamer-{suffix}-{}-{id}",
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

    fn write_manifest(root: &Path, extra: &str) -> Result<PathBuf> {
        let manifest_dir = root.join("build-assets/gstreamer");
        fs::create_dir_all(&manifest_dir)?;
        fs::write(manifest_dir.join("THIRD_PARTY_NOTICES.md"), "notices\n")?;
        let manifest_path = manifest_dir.join("runtime-manifest.toml");
        fs::write(
            &manifest_path,
            format!(
                r#"version = "1.28.5"
notices = "build-assets/gstreamer/THIRD_PARTY_NOTICES.md"

[[platform]]
target = "aarch64-apple-darwin"
deployment = "private-bundle"
source_url = "https://gstreamer.freedesktop.org/example.pkg"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[[platform.files]]
source = "lib/libgstreamer-1.0.0.dylib"
destination = "lib/libgstreamer-1.0.0.dylib"
license = "LGPL-2.1-or-later"

[[platform.plugin]]
element = "playbin"
plugin = "playback"
license = "LGPL-2.1-or-later"

[[platform]]
target = "x86_64-pc-windows-msvc"
deployment = "private-bundle"
source_url = "https://gstreamer.freedesktop.org/example.msi"
sha256 = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"

[[platform.files]]
source = "bin/gstreamer-1.0-0.dll"
destination = "bin/gstreamer-1.0-0.dll"
license = "LGPL-2.1-or-later"

[[platform.plugin]]
element = "playbin"
plugin = "playback"
license = "LGPL-2.1-or-later"

[[platform]]
target = "x86_64-unknown-linux-gnu"
deployment = "system-packages"
minimum_version = "1.20"
package = ["gstreamer1.0-tools", "gstreamer1.0-plugins-base"]

[[platform.plugin]]
element = "playbin"
plugin = "playback"
license = "LGPL-2.1-or-later"

{extra}
"#
            ),
        )?;
        Ok(manifest_path)
    }

    #[test]
    fn valid_manifest_configures_linux_deb_dependencies() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path, "")?;
        assert_eq!(
            load_manifest(&manifest_path)?.version,
            "1.28.5",
            "the complete fixture should pass validation"
        );

        let mut bundle_settings = BundleSettings::default();
        configure_linux_bundle(BundleApp::HttpClient, &temp_dir.path, &mut bundle_settings)?;

        assert_eq!(
            bundle_settings.deb.depends,
            Some(vec![
                "gstreamer1.0-tools".to_string(),
                "gstreamer1.0-plugins-base".to_string(),
            ])
        );
        Ok(())
    }

    #[test]
    fn manifest_rejects_placeholder_checksum() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(
            &temp_dir.path,
            "# fixture remains valid until this line is rewritten by the test\n",
        )?;
        let content = fs::read_to_string(&manifest_path)?;
        fs::write(
            &manifest_path,
            content.replace(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "not-a-checksum",
            ),
        )?;

        let error = load_manifest(&manifest_path).expect_err("invalid checksum should fail");
        assert!(error.to_string().contains("64-character sha256"));
        Ok(())
    }

    #[test]
    fn manifest_rejects_escaping_runtime_path() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path, "")?;
        let content = fs::read_to_string(&manifest_path)?;
        fs::write(
            &manifest_path,
            content.replace(
                "source = \"lib/libgstreamer-1.0.0.dylib\"",
                "source = \"../lib/libgstreamer-1.0.0.dylib\"",
            ),
        )?;

        let error = load_manifest(&manifest_path).expect_err("escaping path should fail");
        assert!(error.to_string().contains("relative path"));
        Ok(())
    }

    #[test]
    fn staging_copies_only_the_manifest_allow_list_and_audit_files() -> Result<()> {
        let temp_dir = TestDir::new()?;
        write_manifest(&temp_dir.path, "")?;
        let runtime_dir = temp_dir.path.join("runtime");
        fs::create_dir_all(runtime_dir.join("lib"))?;
        fs::write(runtime_dir.join("lib/libgstreamer-1.0.0.dylib"), "runtime")?;
        fs::write(runtime_dir.join("not-listed.dylib"), "do not copy")?;
        let destination = temp_dir.path.join("staged");

        stage_private_runtime_from_directory(
            &temp_dir.path,
            "aarch64-apple-darwin",
            &destination,
            &runtime_dir,
        )?;

        assert!(destination.join("lib/libgstreamer-1.0.0.dylib").is_file());
        assert!(!destination.join("not-listed.dylib").exists());
        assert!(destination.join("THIRD_PARTY_NOTICES.md").is_file());
        assert!(destination.join("runtime-manifest.toml").is_file());
        Ok(())
    }

    #[test]
    fn sdk_verification_rejects_an_empty_minimum_version() {
        let error = super::verify_sdk_with_commands(
            " ",
            std::ffi::OsStr::new("unused-pkg-config"),
            std::ffi::OsStr::new("unused-gst-inspect"),
        )
        .expect_err("an empty SDK minimum version must fail before executing commands");
        assert!(error.to_string().contains("minimum version"));
    }

    #[cfg(unix)]
    #[test]
    fn sdk_verification_checks_pkg_config_and_gst_inspect() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TestDir::new()?;
        let pkg_config = temp_dir.path.join("pkg-config");
        let gst_inspect = temp_dir.path.join("gst-inspect-1.0");
        fs::write(
            &pkg_config,
            "#!/usr/bin/env sh\n[ \"$1\" = \"--atleast-version=1.20\" ] && [ \"$2\" = \"gstreamer-1.0\" ]\n",
        )?;
        fs::write(
            &gst_inspect,
            "#!/usr/bin/env sh\n[ \"$1\" = \"--version\" ]\n",
        )?;
        fs::set_permissions(&pkg_config, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&gst_inspect, fs::Permissions::from_mode(0o755))?;

        super::verify_sdk_with_commands("1.20", pkg_config.as_os_str(), gst_inspect.as_os_str())
    }
}
