//! App-local GStreamer runtime manifest validation and staging.
//!
//! Release packaging consumes a complete, platform-native runtime prepared by
//! the platform scripts. The manifest freezes the upstream source, runtime
//! layout, selected macOS components and required plugin/element contract; the
//! platform stagers verify that contract before placing the runtime in the
//! installer.

#[cfg(any(target_os = "windows", target_os = "linux", test))]
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::cli::BundleApp;
use crate::error::{Result, XtaskError};

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod windows;

const HTTP_CLIENT_MANIFEST: &str = "build-assets/gstreamer/runtime-manifest.toml";
const RUNTIME_DIRECTORY_ENV: &str = "GPUI_GSTREAMER_RUNTIME_DIR";
const SDK_DIRECTORY_ENV: &str = "GPUI_GSTREAMER_SDK_ROOT";
const GST_INSPECT_ENV: &str = "GPUI_GST_INSPECT";
const PKG_CONFIG_ENV: &str = "GPUI_PKG_CONFIG";
const MINIMUM_SDK_VERSION: &str = "1.20";
const NATIVE_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(any(target_os = "windows", target_os = "linux"))]
static NEXT_VERIFICATION_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn release_build_environment(
    app: BundleApp,
    workspace_dir: &Path,
    app_dir: &Path,
) -> Result<Vec<(OsString, OsString)>> {
    if app != BundleApp::HttpClient {
        return Ok(Vec::new());
    }

    let target = current_target()?;
    let manifest = load_http_client_manifest(app_dir)?;
    let runtime_contract = manifest.platform_for(target)?;
    let sdk_root = sdk_directory(target, workspace_dir)?;
    let bin = sdk_root.join("bin");
    let pkg_config_path = sdk_root.join("lib/pkgconfig");
    if !pkg_config_path.join("gstreamer-1.0.pc").is_file() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK root {} has no lib/pkgconfig/gstreamer-1.0.pc",
            sdk_root.display()
        )));
    }
    let bundled_pkg_config = platform_pkg_config(&bin);
    let pkg_config = pkg_config_command(target, &bundled_pkg_config)?;
    verify_sdk_minimum_version(&pkg_config, &pkg_config_path, MINIMUM_SDK_VERSION)?;
    verify_sdk_exact_version(&pkg_config, &pkg_config_path, &manifest.version)?;

    let runtime = release_runtime_directory(target, workspace_dir, runtime_contract)?;
    let path = prepend_search_path(&bin, env::var_os("PATH"))?;
    let mut environment = vec![
        (OsString::from("PKG_CONFIG"), pkg_config.clone()),
        (OsString::from(PKG_CONFIG_ENV), pkg_config),
        (
            OsString::from("PKG_CONFIG_PATH"),
            pkg_config_path.into_os_string(),
        ),
        (OsString::from("PATH"), path),
        (
            OsString::from(RUNTIME_DIRECTORY_ENV),
            runtime.into_os_string(),
        ),
    ];
    #[cfg(target_os = "macos")]
    environment.push((
        OsString::from("DYLD_FALLBACK_LIBRARY_PATH"),
        prepend_search_path(
            &sdk_root.join("lib"),
            env::var_os("DYLD_FALLBACK_LIBRARY_PATH"),
        )?,
    ));
    #[cfg(target_os = "linux")]
    {
        environment.push((
            OsString::from("LD_LIBRARY_PATH"),
            prepend_search_path(&sdk_root.join("lib"), env::var_os("LD_LIBRARY_PATH"))?,
        ));
        environment.push((
            OsString::from("LIBRARY_PATH"),
            prepend_search_path(&sdk_root.join("lib"), env::var_os("LIBRARY_PATH"))?,
        ));
    }
    Ok(environment)
}

fn sdk_directory(target: &str, workspace_dir: &Path) -> Result<PathBuf> {
    if let Some(explicit) = env::var_os(SDK_DIRECTORY_ENV).map(PathBuf::from) {
        return canonical_sdk_directory(&explicit);
    }
    for candidate in default_sdk_directories(target, workspace_dir) {
        if candidate.is_dir()
            && let Ok(sdk) = canonical_sdk_directory(&candidate)
        {
            return Ok(sdk);
        }
    }
    #[cfg(target_os = "macos")]
    if target == "aarch64-apple-darwin" {
        prepare_macos_sdk(workspace_dir)?;
        return canonical_sdk_directory(&macos_sdk_directory(workspace_dir));
    }
    Err(XtaskError::msg(format!(
        "GStreamer SDK >= {MINIMUM_SDK_VERSION} is required to build a self-contained HTTP Client package; {}",
        sdk_setup_instruction(target)
    )))
}

#[cfg(test)]
fn resolve_sdk_directory(
    explicit: Option<PathBuf>,
    candidates: Vec<PathBuf>,
    setup_instruction: &str,
) -> Result<PathBuf> {
    if let Some(explicit) = explicit {
        return canonical_sdk_directory(&explicit);
    }
    for candidate in candidates {
        if candidate.is_dir()
            && let Ok(sdk) = canonical_sdk_directory(&candidate)
        {
            return Ok(sdk);
        }
    }
    Err(XtaskError::msg(format!(
        "GStreamer SDK >= {MINIMUM_SDK_VERSION} is required to build a self-contained HTTP Client package; {}",
        setup_instruction
    )))
}

fn canonical_sdk_directory(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve GStreamer SDK root {}: {err}",
            path.display()
        ))
    })?;
    if !canonical.join("lib/pkgconfig/gstreamer-1.0.pc").is_file() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK root {} has no lib/pkgconfig/gstreamer-1.0.pc",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn default_sdk_directories(target: &str, workspace_dir: &Path) -> Vec<PathBuf> {
    match target {
        "aarch64-apple-darwin" => vec![macos_sdk_directory(workspace_dir)],
        "x86_64-pc-windows-msvc" => env::var_os("GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC")
            .map(PathBuf::from)
            .into_iter()
            .chain([PathBuf::from(r"C:\gstreamer\1.0\msvc_x86_64")])
            .collect(),
        "x86_64-unknown-linux-gnu" => {
            vec![workspace_dir.join("target/gstreamer-runtime/linux-x86_64")]
        }
        _ => Vec::new(),
    }
}

fn homebrew_pkgconf_prefix() -> Option<PathBuf> {
    let output = Command::new("brew")
        .args(["--prefix", "pkgconf"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!prefix.is_empty()).then(|| PathBuf::from(prefix))
}

fn pkg_config_command(target: &str, bundled_pkg_config: &Path) -> Result<OsString> {
    if let Some(explicit) = env::var_os(PKG_CONFIG_ENV) {
        return Ok(explicit);
    }
    if bundled_pkg_config.is_file() {
        return Ok(bundled_pkg_config.as_os_str().to_owned());
    }
    if target == "aarch64-apple-darwin" {
        if let Some(command) = macos_pkg_config_candidates(homebrew_pkgconf_prefix())
            .into_iter()
            .find(|candidate| candidate.is_file())
        {
            return Ok(command.into_os_string());
        }
        return Err(XtaskError::msg(
            "Homebrew pkgconf is required to build HTTP Client; run `brew install pkgconf`",
        ));
    }
    Ok(OsString::from("pkg-config"))
}

fn macos_pkg_config_candidates(homebrew_prefix: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = homebrew_prefix
        .into_iter()
        .map(|prefix| prefix.join("bin/pkg-config"))
        .collect::<Vec<_>>();
    for fallback in [
        PathBuf::from("/opt/homebrew/opt/pkgconf/bin/pkg-config"),
        PathBuf::from("/usr/local/opt/pkgconf/bin/pkg-config"),
    ] {
        if !candidates.contains(&fallback) {
            candidates.push(fallback);
        }
    }
    candidates
}

fn default_runtime_directories(target: &str, workspace_dir: &Path) -> Vec<PathBuf> {
    match target {
        "aarch64-apple-darwin" => {
            vec![workspace_dir.join("target/gstreamer-runtime/macos/GStreamer.framework")]
        }
        "x86_64-pc-windows-msvc" => env::var_os("GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC")
            .map(PathBuf::from)
            .into_iter()
            .chain([PathBuf::from(r"C:\gstreamer\1.0\msvc_x86_64")])
            .collect(),
        "x86_64-unknown-linux-gnu" => {
            vec![workspace_dir.join("target/gstreamer-runtime/linux-x86_64")]
        }
        _ => Vec::new(),
    }
}

fn sdk_setup_instruction(target: &str) -> &'static str {
    match target {
        "aarch64-apple-darwin" => {
            "run `script/prepare-gstreamer-macos-sdk.sh`; bundle normally runs this producer automatically"
        }
        "x86_64-pc-windows-msvc" => {
            "run `script/install-gstreamer-windows.ps1`, then rerun the bundle command"
        }
        "x86_64-unknown-linux-gnu" => {
            "run `script/build-gstreamer-linux-runtime.sh --output target/gstreamer-runtime/linux-x86_64`, then rerun the bundle command"
        }
        _ => "set GPUI_GSTREAMER_SDK_ROOT to the pinned release SDK",
    }
}

fn macos_sdk_directory(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("target/gstreamer-sdk/macos/GStreamer.framework/Versions/1.0")
}

#[cfg(any(target_os = "macos", test))]
fn prepare_macos_sdk(workspace_dir: &Path) -> Result<()> {
    let (script, output) = macos_sdk_preparation_paths(workspace_dir);
    let status = Command::new(&script)
        .args(["--output"])
        .arg(&output)
        .current_dir(workspace_dir)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to prepare the macOS GStreamer SDK with {}: {err}",
                script.display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "macOS GStreamer SDK preparation failed with {}; run `{}` manually and fix the reported error",
            status,
            script.display()
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn macos_sdk_preparation_paths(workspace_dir: &Path) -> (PathBuf, PathBuf) {
    (
        workspace_dir.join("script/prepare-gstreamer-macos-sdk.sh"),
        macos_sdk_directory(workspace_dir),
    )
}

fn runtime_setup_instruction(target: &str) -> &'static str {
    match target {
        "aarch64-apple-darwin" => {
            "run `script/prepare-gstreamer-macos-runtime.sh`; bundle normally runs this producer automatically when the prepared runtime is absent"
        }
        "x86_64-pc-windows-msvc" => {
            "run `script/install-gstreamer-windows.ps1`, then rerun the bundle command"
        }
        "x86_64-unknown-linux-gnu" => {
            "run `script/build-gstreamer-linux-runtime.sh --output target/gstreamer-runtime/linux-x86_64`, then rerun the bundle command"
        }
        _ => "set GPUI_GSTREAMER_RUNTIME_DIR to the prepared private runtime",
    }
}

fn verify_sdk_minimum_version(
    pkg_config: &std::ffi::OsStr,
    pkg_config_path: &Path,
    minimum_version: &str,
) -> Result<()> {
    let version_check = format!("--atleast-version={minimum_version}");
    let status = Command::new(pkg_config)
        .args([version_check.as_str(), "gstreamer-1.0"])
        .env("PKG_CONFIG_PATH", pkg_config_path)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to query the GStreamer SDK version with {}: {err}",
                Path::new(pkg_config).display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK discovered by `{}` does not satisfy >= {minimum_version}",
            Path::new(pkg_config).display(),
        )));
    }
    Ok(())
}

fn verify_sdk_exact_version(
    pkg_config: &std::ffi::OsStr,
    pkg_config_path: &Path,
    version: &str,
) -> Result<()> {
    let version_check = format!("--exact-version={version}");
    let status = Command::new(pkg_config)
        .args([version_check.as_str(), "gstreamer-1.0"])
        .env("PKG_CONFIG_PATH", pkg_config_path)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to compare the GStreamer SDK and runtime versions with {}: {err}",
                Path::new(pkg_config).display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "GStreamer SDK discovered by `{}` does not match bundled runtime version {version}",
            Path::new(pkg_config).display(),
        )));
    }
    Ok(())
}

fn platform_pkg_config(bin: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        bin.join("pkg-config.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        bin.join("pkg-config")
    }
}

fn prepend_search_path(directory: &Path, inherited: Option<OsString>) -> Result<OsString> {
    let mut paths = vec![directory.to_path_buf()];
    if let Some(inherited) = inherited {
        paths.extend(env::split_paths(&inherited));
    }
    env::join_paths(paths).map_err(|err| {
        XtaskError::msg(format!(
            "failed to construct GStreamer build search path: {err}"
        ))
    })
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    format: u32,
    version: String,
    notices: String,
    platform: Vec<PlatformRuntime>,
}

#[derive(Debug, Deserialize)]
struct PlatformRuntime {
    target: String,
    layout: RuntimeLayout,
    source_url: String,
    #[serde(default)]
    source_sha256: Option<String>,
    #[serde(default)]
    source_revision: Option<String>,
    #[serde(default)]
    build_baseline: Option<String>,
    #[serde(default)]
    component: Vec<String>,
    required_path: Vec<String>,
    #[serde(default)]
    runtime_plugin: Vec<String>,
    #[serde(default)]
    plugin: Vec<PluginContract>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum RuntimeLayout {
    MacosFramework,
    WindowsPrefix,
    LinuxPrefix,
}

#[derive(Debug, Deserialize)]
struct PluginContract {
    element: String,
    plugin: String,
}

#[cfg(target_os = "macos")]
pub(crate) fn stage_macos_runtime(app: BundleApp, app_dir: &Path, app_bundle: &Path) -> Result<()> {
    if app != BundleApp::HttpClient {
        return Ok(());
    }

    let manifest = load_http_client_manifest(app_dir)?;
    let runtime = manifest.platform_for("aarch64-apple-darwin")?;
    runtime.require_layout(RuntimeLayout::MacosFramework)?;
    let source = runtime_directory("aarch64-apple-darwin", None)?;
    validate_runtime_directory(runtime, &source)?;
    macos::stage_private_runtime(
        &source,
        app_bundle,
        &runtime.required_elements(),
        &runtime.required_plugins(),
    )?;
    stage_audit_files(
        app_dir,
        &manifest,
        &app_bundle.join("Contents/Frameworks/GStreamer.framework/Versions/1.0/share/http-client"),
    )
}

#[cfg(any(target_os = "windows", test))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn stage_windows_runtime(
    app: BundleApp,
    app_dir: &Path,
    staging_dir: &Path,
) -> Result<HashMap<String, String>> {
    if app != BundleApp::HttpClient {
        return Ok(HashMap::new());
    }

    let manifest = load_http_client_manifest(app_dir)?;
    let runtime = manifest.platform_for("x86_64-pc-windows-msvc")?;
    runtime.require_layout(RuntimeLayout::WindowsPrefix)?;
    let source = runtime_directory("x86_64-pc-windows-msvc", None)?;
    validate_runtime_directory(runtime, &source)?;
    let inventory = runtime_file_inventory(&source, RuntimeLayout::WindowsPrefix)?;
    let stage = windows::stage_private_runtime(&source, &inventory, staging_dir)?;
    let registry = staging_dir.join("gstreamer-registry.bin");
    let staged_runtime = staging_dir.join("gstreamer");
    windows::verify_private_elements(
        &staged_runtime.join("bin/gst-inspect-1.0.exe"),
        &staged_runtime,
        &registry,
        &runtime.required_elements(),
    )?;
    let mut resources = stage.resources_map.into_iter().collect();
    let audit_root = staging_dir.join("gstreamer/share/http-client");
    stage_audit_files(app_dir, &manifest, &audit_root)?;
    append_resource_tree(&audit_root, "gstreamer/share/http-client", &mut resources)?;
    Ok(resources)
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn stage_linux_runtime(
    app: BundleApp,
    app_dir: &Path,
    staging_dir: &Path,
    main_binary: &Path,
) -> Result<HashMap<String, String>> {
    if app != BundleApp::HttpClient {
        return Ok(HashMap::new());
    }

    let manifest = load_http_client_manifest(app_dir)?;
    let runtime = manifest.platform_for("x86_64-unknown-linux-gnu")?;
    runtime.require_layout(RuntimeLayout::LinuxPrefix)?;
    let source = runtime_directory("x86_64-unknown-linux-gnu", None)?;
    validate_runtime_directory(runtime, &source)?;
    let inventory = runtime_file_inventory(&source, RuntimeLayout::LinuxPrefix)?;
    let stage = linux::stage_private_runtime(&source, &inventory, staging_dir, main_binary)?;
    let registry = staging_dir.join("gstreamer-registry.bin");
    linux::verify_private_elements(
        &stage.runtime_root.join("bin/gst-inspect-1.0"),
        &stage.runtime_root,
        &registry,
        &runtime.required_elements(),
    )?;
    let mut resources = stage.resources_map.into_iter().collect();
    let audit_root = staging_dir.join("gstreamer/share/http-client");
    stage_audit_files(app_dir, &manifest, &audit_root)?;
    append_resource_tree(&audit_root, "gstreamer/share/http-client", &mut resources)?;
    Ok(resources)
}

pub(crate) fn verify_runtime_manifest(manifest_path: &Path, inspect: bool) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    validate_notices_file(manifest_path, &manifest)?;
    if !inspect {
        return Ok(());
    }

    let target = current_target()?;
    let runtime = manifest.platform_for(target)?;
    let runtime_root = runtime_directory(target, None)?;
    validate_runtime_directory(runtime, &runtime_root)?;
    let elements = runtime.required_elements();
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let registry = VerificationRegistry::new()?;

    #[cfg(target_os = "macos")]
    {
        runtime.require_layout(RuntimeLayout::MacosFramework)?;
        macos::verify_private_elements(&runtime_root, &elements)
    }
    #[cfg(target_os = "windows")]
    {
        runtime.require_layout(RuntimeLayout::WindowsPrefix)?;
        windows::verify_private_elements(
            &runtime_root.join("bin/gst-inspect-1.0.exe"),
            &runtime_root,
            registry.path(),
            &elements,
        )
    }
    #[cfg(target_os = "linux")]
    {
        runtime.require_layout(RuntimeLayout::LinuxPrefix)?;
        linux::verify_private_elements(
            &runtime_root.join("bin/gst-inspect-1.0"),
            &runtime_root,
            registry.path(),
            &elements,
        )
    }
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

pub(super) fn command_status_with_timeout(
    command: &mut Command,
    description: &str,
) -> Result<std::process::ExitStatus> {
    command_status_with_timeout_duration(command, description, NATIVE_COMMAND_TIMEOUT)
}

fn command_status_with_timeout_duration(
    command: &mut Command,
    description: &str,
    timeout: Duration,
) -> Result<std::process::ExitStatus> {
    let mut child = command
        .spawn()
        .map_err(|err| XtaskError::msg(format!("failed to start {description}: {err}")))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| XtaskError::msg(format!("failed to wait for {description}: {err}")))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(XtaskError::msg(format!(
                "{description} exceeded the release verification timeout"
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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

fn load_http_client_manifest(app_dir: &Path) -> Result<RuntimeManifest> {
    let path = app_dir.join(HTTP_CLIENT_MANIFEST);
    let manifest = load_manifest(&path)?;
    validate_notices_file(&path, &manifest)?;
    Ok(manifest)
}

fn runtime_directory(target: &str, workspace_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(explicit) = env::var_os(RUNTIME_DIRECTORY_ENV).map(PathBuf::from) {
        return canonical_runtime_directory(&explicit);
    }
    let workspace_dir = workspace_dir
        .map(Path::to_path_buf)
        .or_else(|| crate::context::workspace_root().ok());
    if let Some(workspace_dir) = workspace_dir.as_deref() {
        for candidate in default_runtime_directories(target, workspace_dir) {
            if candidate.is_dir() {
                return canonical_runtime_directory(&candidate);
            }
        }
    }
    #[cfg(target_os = "macos")]
    if target == "aarch64-apple-darwin"
        && let Some(workspace_dir) = workspace_dir.as_deref()
    {
        prepare_macos_runtime(workspace_dir)?;
        return canonical_runtime_directory(&macos_runtime_directory(workspace_dir));
    }
    Err(XtaskError::msg(format!(
        "a prepared GStreamer runtime is required to stage `{target}`; {}",
        runtime_setup_instruction(target)
    )))
}

fn release_runtime_directory(
    target: &str,
    workspace_dir: &Path,
    runtime: &PlatformRuntime,
) -> Result<PathBuf> {
    if let Some(explicit) = env::var_os(RUNTIME_DIRECTORY_ENV).map(PathBuf::from) {
        let directory = canonical_runtime_directory(&explicit)?;
        validate_runtime_directory(runtime, &directory)?;
        return Ok(directory);
    }

    let mut invalid_automatic_runtime = None;
    for candidate in default_runtime_directories(target, workspace_dir) {
        if !candidate.is_dir() {
            continue;
        }
        let result = validated_runtime_candidate(runtime, &candidate);
        match result {
            Ok(directory) => return Ok(directory),
            Err(error) => invalid_automatic_runtime = Some(error),
        }
    }

    #[cfg(target_os = "macos")]
    if target == "aarch64-apple-darwin" {
        prepare_macos_runtime(workspace_dir)?;
        let directory = canonical_runtime_directory(&macos_runtime_directory(workspace_dir))?;
        validate_runtime_directory(runtime, &directory)?;
        return Ok(directory);
    }

    if let Some(error) = invalid_automatic_runtime {
        return Err(error);
    }
    Err(XtaskError::msg(format!(
        "a prepared GStreamer runtime is required to stage `{target}`; {}",
        runtime_setup_instruction(target)
    )))
}

fn validated_runtime_candidate(runtime: &PlatformRuntime, candidate: &Path) -> Result<PathBuf> {
    let directory = canonical_runtime_directory(candidate)?;
    validate_runtime_directory(runtime, &directory)?;
    Ok(directory)
}

#[cfg(any(target_os = "macos", test))]
fn macos_runtime_directory(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("target/gstreamer-runtime/macos/GStreamer.framework")
}

#[cfg(any(target_os = "macos", test))]
fn prepare_macos_runtime(workspace_dir: &Path) -> Result<()> {
    let (script, output) = macos_runtime_preparation_paths(workspace_dir);
    let status = Command::new(&script)
        .args(["--output"])
        .arg(&output)
        .current_dir(workspace_dir)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to prepare the macOS GStreamer runtime with {}: {err}",
                script.display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "macOS GStreamer runtime preparation failed with {}; run `{}` manually and fix the reported error",
            status,
            script.display()
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn macos_runtime_preparation_paths(workspace_dir: &Path) -> (PathBuf, PathBuf) {
    (
        workspace_dir.join("script/prepare-gstreamer-macos-runtime.sh"),
        macos_runtime_directory(workspace_dir),
    )
}

fn canonical_runtime_directory(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve GStreamer runtime directory {}: {err}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime directory {} is not a directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn validate_runtime_directory(runtime: &PlatformRuntime, runtime_dir: &Path) -> Result<()> {
    for relative in &runtime.required_path {
        let path = runtime_dir.join(checked_relative_path(relative, "required runtime path")?);
        if !path.exists() {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime for `{}` is missing required path `{relative}`",
                runtime.target
            )));
        }
    }
    validate_runtime_provenance(runtime, runtime_dir)?;
    Ok(())
}

fn validate_runtime_provenance(runtime: &PlatformRuntime, runtime_dir: &Path) -> Result<()> {
    let marker_root = if runtime.layout == RuntimeLayout::MacosFramework {
        Path::new("Versions/1.0/share/http-client-runtime")
    } else {
        Path::new("share/http-client-runtime")
    };
    let (file_name, expected) = if let Some(sha256) = &runtime.source_sha256 {
        ("source-sha256.txt", sha256.as_str())
    } else if let Some(revision) = &runtime.source_revision {
        ("source-revision.txt", revision.as_str())
    } else {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime `{}` has no source identity",
            runtime.target
        )));
    };
    let marker = runtime_dir.join(marker_root).join(file_name);
    let actual = fs::read_to_string(&marker).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read GStreamer source identity marker {}: {err}",
            marker.display()
        ))
    })?;
    if actual.trim() != expected {
        return Err(XtaskError::msg(format!(
            "GStreamer source identity marker for `{}` does not match runtime-manifest.toml",
            runtime.target
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn runtime_file_inventory(runtime_dir: &Path, layout: RuntimeLayout) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(runtime_dir).follow_links(false) {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to inventory GStreamer runtime {}: {err}",
                runtime_dir.display()
            ))
        })?;
        if !entry.file_type().is_file() && !entry.file_type().is_symlink() {
            continue;
        }
        let relative = entry.path().strip_prefix(runtime_dir).map_err(|err| {
            XtaskError::msg(format!(
                "failed to make GStreamer runtime path {} relative to {}: {err}",
                entry.path().display(),
                runtime_dir.display()
            ))
        })?;
        if !is_runtime_payload_path(relative, layout) {
            continue;
        }
        files.push(relative.to_path_buf());
    }
    files.sort();
    if files.is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime directory {} is empty",
            runtime_dir.display()
        )));
    }
    Ok(files)
}

#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn is_runtime_payload_path(path: &Path, layout: RuntimeLayout) -> bool {
    let Some(first) = path.components().next() else {
        return false;
    };
    let Component::Normal(first) = first else {
        return false;
    };
    if matches!(first.to_str(), Some("bin" | "libexec" | "etc" | "share")) {
        return true;
    }
    if first != "lib" {
        return false;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match layout {
        RuntimeLayout::WindowsPrefix => file_name.ends_with(".dll"),
        RuntimeLayout::LinuxPrefix => file_name.contains(".so"),
        RuntimeLayout::MacosFramework => false,
    }
}

fn stage_audit_files(
    app_dir: &Path,
    manifest: &RuntimeManifest,
    destination_root: &Path,
) -> Result<()> {
    fs::create_dir_all(destination_root).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create GStreamer audit directory {}: {err}",
            destination_root.display()
        ))
    })?;
    let manifest_path = app_dir.join(HTTP_CLIENT_MANIFEST);
    let notice_source = manifest_path
        .parent()
        .ok_or_else(|| {
            XtaskError::msg(format!(
                "failed to resolve GStreamer manifest directory for {}",
                manifest_path.display()
            ))
        })?
        .join(checked_relative_path(&manifest.notices, "notices")?);
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

#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn append_resource_tree(
    root: &Path,
    destination_root: &str,
    resources: &mut HashMap<String, String>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to walk staged GStreamer audit files under {}: {err}",
                root.display()
            ))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root).map_err(|err| {
            XtaskError::msg(format!(
                "failed to make {} relative to {}: {err}",
                entry.path().display(),
                root.display()
            ))
        })?;
        resources.insert(
            entry.path().to_string_lossy().into_owned(),
            Path::new(destination_root)
                .join(relative)
                .to_string_lossy()
                .into_owned(),
        );
    }
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

impl PlatformRuntime {
    fn require_layout(&self, expected: RuntimeLayout) -> Result<()> {
        if self.layout != expected {
            return Err(XtaskError::msg(format!(
                "GStreamer target `{}` uses layout `{:?}` instead of `{:?}`",
                self.target, self.layout, expected
            )));
        }
        Ok(())
    }

    fn required_elements(&self) -> Vec<String> {
        self.plugin
            .iter()
            .map(|plugin| plugin.element.clone())
            .collect()
    }

    fn required_plugins(&self) -> Vec<String> {
        let mut plugins = self
            .plugin
            .iter()
            .map(|plugin| plugin.plugin.clone())
            .chain(self.runtime_plugin.iter().cloned())
            .collect::<Vec<_>>();
        plugins.sort();
        plugins.dedup();
        plugins
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

fn validate_notices_file(path: &Path, manifest: &RuntimeManifest) -> Result<()> {
    let directory = path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve GStreamer manifest directory for {}",
            path.display()
        ))
    })?;
    let notices = directory.join(checked_relative_path(&manifest.notices, "notices")?);
    if !notices.is_file() {
        return Err(XtaskError::msg(format!(
            "required GStreamer notices file {} is missing",
            notices.display()
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
struct VerificationRegistry {
    directory: PathBuf,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl VerificationRegistry {
    fn new() -> Result<Self> {
        let id = NEXT_VERIFICATION_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| XtaskError::msg(format!("failed to read system time: {err}")))?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "xtask-gstreamer-verify-{timestamp}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).map_err(|err| {
            XtaskError::msg(format!(
                "failed to create GStreamer verification directory {}: {err}",
                directory.display()
            ))
        })?;
        Ok(Self { directory })
    }

    fn path(&self) -> &Path {
        &self.directory
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Drop for VerificationRegistry {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn validate_manifest(manifest: &RuntimeManifest, path: &Path) -> Result<()> {
    if manifest.format != 1 {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} uses unsupported format {}",
            path.display(),
            manifest.format
        )));
    }
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
    if !runtime.source_url.starts_with("https://") {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} needs an HTTPS source_url for `{}`",
            path.display(),
            runtime.target
        )));
    }
    match (&runtime.source_sha256, &runtime.source_revision) {
        (Some(sha256), None)
            if sha256.len() == 64
                && sha256.bytes().all(|byte| {
                    byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte.is_ascii_hexdigit())
                }) => {}
        (None, Some(revision))
            if revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) => {}
        _ => {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime manifest {} must declare exactly one valid source_sha256 or source_revision for `{}`",
                path.display(),
                runtime.target
            )));
        }
    }
    if runtime.required_path.is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer runtime manifest {} has no required paths for `{}`",
            path.display(),
            runtime.target
        )));
    }
    let mut required_paths = HashSet::new();
    for required_path in &runtime.required_path {
        checked_relative_path(required_path, "required runtime path")?;
        if !required_paths.insert(required_path) {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime manifest {} has duplicate required path `{required_path}` for `{}`",
                path.display(),
                runtime.target
            )));
        }
    }
    if runtime.layout == RuntimeLayout::MacosFramework && runtime.component.is_empty() {
        return Err(XtaskError::msg(format!(
            "GStreamer macOS runtime in {} must declare selected package components",
            path.display()
        )));
    }
    if runtime.layout == RuntimeLayout::LinuxPrefix
        && runtime
            .build_baseline
            .as_deref()
            .is_none_or(|baseline| baseline.trim().is_empty())
    {
        return Err(XtaskError::msg(format!(
            "GStreamer Linux runtime in {} must declare its build_baseline",
            path.display()
        )));
    }

    let mut plugin_elements = HashSet::new();
    for plugin in &runtime.plugin {
        if plugin.element.trim().is_empty()
            || plugin.plugin.trim().is_empty()
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
    let mut runtime_plugins = HashSet::new();
    for plugin in &runtime.runtime_plugin {
        if plugin.trim().is_empty() || !runtime_plugins.insert(plugin) {
            return Err(XtaskError::msg(format!(
                "GStreamer runtime manifest {} has an empty or duplicate runtime plugin `{plugin}` for `{}`",
                path.display(),
                runtime.target
            )));
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
    use super::{RuntimeLayout, load_manifest, runtime_file_inventory, verify_runtime_manifest};
    use crate::error::Result;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn write_manifest(root: &Path) -> Result<PathBuf> {
        let manifest_dir = root.join("build-assets/gstreamer");
        fs::create_dir_all(&manifest_dir)?;
        fs::write(manifest_dir.join("THIRD_PARTY_NOTICES.md"), "notices\n")?;
        let manifest_path = manifest_dir.join("runtime-manifest.toml");
        fs::write(
            &manifest_path,
            r#"format = 1
version = "1.28.6"
notices = "THIRD_PARTY_NOTICES.md"

[[platform]]
target = "aarch64-apple-darwin"
layout = "macos-framework"
source_url = "https://gstreamer.freedesktop.org/example.pkg"
source_sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
component = ["core"]
required_path = ["Versions/1.0/lib/libgstreamer-1.0.0.dylib"]

[[platform.plugin]]
element = "playbin"
plugin = "playback"

[[platform]]
target = "x86_64-pc-windows-msvc"
layout = "windows-prefix"
source_url = "https://gstreamer.freedesktop.org/example.exe"
source_sha256 = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
required_path = ["bin/gstreamer-1.0-0.dll"]

[[platform.plugin]]
element = "playbin"
plugin = "playback"

[[platform]]
target = "x86_64-unknown-linux-gnu"
layout = "linux-prefix"
source_url = "https://gitlab.freedesktop.org/gstreamer/cerbero.git"
source_revision = "78666745b34b6245a85510ac47a03a5033af4711"
build_baseline = "x86_64 glibc >= 2.35 (Ubuntu 22.04)"
required_path = ["lib/libgstreamer-1.0.so.0"]

[[platform.plugin]]
element = "playbin"
plugin = "playback"
"#,
        )?;
        Ok(manifest_path)
    }

    #[test]
    fn valid_manifest_freezes_three_private_runtime_layouts() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path)?;
        let manifest = load_manifest(&manifest_path)?;
        assert_eq!(manifest.version, "1.28.6");
        assert_eq!(manifest.platform.len(), 3);
        Ok(())
    }

    #[test]
    fn manifest_rejects_placeholder_checksum() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path)?;
        let content = fs::read_to_string(&manifest_path)?;
        fs::write(
            &manifest_path,
            content.replace(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "not-a-checksum",
            ),
        )?;

        let error = load_manifest(&manifest_path).expect_err("invalid checksum should fail");
        assert!(error.to_string().contains("source_sha256"));
        Ok(())
    }

    #[test]
    fn manifest_rejects_escaping_runtime_path() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path)?;
        let content = fs::read_to_string(&manifest_path)?;
        fs::write(
            &manifest_path,
            content.replace(
                "required_path = [\"Versions/1.0/lib/libgstreamer-1.0.0.dylib\"]",
                "required_path = [\"../lib/libgstreamer-1.0.0.dylib\"]",
            ),
        )?;

        let error = load_manifest(&manifest_path).expect_err("escaping path should fail");
        assert!(error.to_string().contains("relative path"));
        Ok(())
    }

    #[test]
    fn verification_requires_the_referenced_notices_file() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path)?;
        fs::remove_file(
            manifest_path
                .parent()
                .unwrap()
                .join("THIRD_PARTY_NOTICES.md"),
        )?;

        let error = verify_runtime_manifest(&manifest_path, false)
            .expect_err("verification must reject a missing notices file");
        assert!(error.to_string().contains("notices file"));
        Ok(())
    }

    #[test]
    fn runtime_source_identity_must_match_the_manifest() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = write_manifest(&temp_dir.path)?;
        let manifest = load_manifest(&manifest_path)?;
        let runtime = manifest.platform_for("aarch64-apple-darwin")?;
        let runtime_root = temp_dir.path.join("runtime");
        fs::create_dir_all(runtime_root.join("Versions/1.0/lib"))?;
        fs::write(
            runtime_root.join("Versions/1.0/lib/libgstreamer-1.0.0.dylib"),
            "runtime",
        )?;
        let marker = runtime_root.join("Versions/1.0/share/http-client-runtime/source-sha256.txt");
        fs::create_dir_all(marker.parent().unwrap())?;
        fs::write(&marker, "wrong\n")?;
        let error = super::validated_runtime_candidate(runtime, &runtime_root)
            .expect_err("a runtime from another source must be rejected");
        assert!(error.to_string().contains("does not match"));

        fs::write(
            &marker,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
        )?;
        assert_eq!(
            super::validated_runtime_candidate(runtime, &runtime_root)?,
            fs::canonicalize(runtime_root)?
        );
        Ok(())
    }

    #[test]
    fn runtime_inventory_is_sorted_and_relative() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let runtime_dir = temp_dir.path.join("runtime");
        fs::create_dir_all(runtime_dir.join("lib"))?;
        fs::write(runtime_dir.join("lib/z.so"), "z")?;
        fs::write(runtime_dir.join("lib/devel.a"), "devel")?;
        assert_eq!(
            runtime_file_inventory(&runtime_dir, RuntimeLayout::LinuxPrefix)?,
            vec![PathBuf::from("lib/z.so")]
        );
        Ok(())
    }

    #[test]
    fn macos_pkg_config_candidates_prefer_homebrew_pkgconf() {
        assert_eq!(
            super::macos_pkg_config_candidates(Some(PathBuf::from("/custom/homebrew/pkgconf"))),
            vec![
                PathBuf::from("/custom/homebrew/pkgconf/bin/pkg-config"),
                PathBuf::from("/opt/homebrew/opt/pkgconf/bin/pkg-config"),
                PathBuf::from("/usr/local/opt/pkgconf/bin/pkg-config"),
            ]
        );
    }

    #[test]
    fn standard_runtime_locations_are_derived_from_the_workspace() {
        let workspace = Path::new("/workspace");
        assert_eq!(
            super::macos_sdk_preparation_paths(workspace),
            (
                PathBuf::from("/workspace/script/prepare-gstreamer-macos-sdk.sh"),
                PathBuf::from(
                    "/workspace/target/gstreamer-sdk/macos/GStreamer.framework/Versions/1.0"
                ),
            )
        );
        assert_eq!(
            super::default_sdk_directories("aarch64-apple-darwin", workspace),
            vec![PathBuf::from(
                "/workspace/target/gstreamer-sdk/macos/GStreamer.framework/Versions/1.0"
            )]
        );
        assert_eq!(
            super::macos_runtime_preparation_paths(workspace),
            (
                PathBuf::from("/workspace/script/prepare-gstreamer-macos-runtime.sh"),
                PathBuf::from("/workspace/target/gstreamer-runtime/macos/GStreamer.framework"),
            )
        );
        assert_eq!(
            super::default_runtime_directories("aarch64-apple-darwin", workspace),
            vec![PathBuf::from(
                "/workspace/target/gstreamer-runtime/macos/GStreamer.framework"
            )]
        );
        assert_eq!(
            super::default_runtime_directories("x86_64-unknown-linux-gnu", workspace),
            vec![PathBuf::from(
                "/workspace/target/gstreamer-runtime/linux-x86_64"
            )]
        );
    }

    #[test]
    fn sdk_root_requires_gstreamer_pkg_config_metadata() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let error = super::canonical_sdk_directory(&temp_dir.path)
            .expect_err("an arbitrary directory must not be accepted as an SDK");
        assert!(error.to_string().contains("gstreamer-1.0.pc"));
        Ok(())
    }

    #[test]
    fn an_invalid_explicit_sdk_does_not_fall_back_to_another_installation() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let explicit = temp_dir.path.join("explicit");
        let fallback = temp_dir.path.join("fallback");
        fs::create_dir_all(&explicit)?;
        fs::create_dir_all(fallback.join("lib/pkgconfig"))?;
        fs::write(fallback.join("lib/pkgconfig/gstreamer-1.0.pc"), "valid")?;

        let error =
            super::resolve_sdk_directory(Some(explicit.clone()), vec![fallback], "install the SDK")
                .expect_err("an invalid explicit override must fail without fallback");
        assert!(error.to_string().contains(&explicit.display().to_string()));
        assert!(error.to_string().contains("gstreamer-1.0.pc"));
        Ok(())
    }

    #[test]
    fn invalid_automatic_sdk_candidate_falls_back_to_the_next_candidate() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let invalid = temp_dir.path.join("invalid");
        let valid = temp_dir.path.join("valid");
        fs::create_dir_all(&invalid)?;
        fs::create_dir_all(valid.join("lib/pkgconfig"))?;
        fs::write(valid.join("lib/pkgconfig/gstreamer-1.0.pc"), "valid")?;

        assert_eq!(
            super::resolve_sdk_directory(None, vec![invalid, valid.clone()], "install the SDK")?,
            fs::canonicalize(valid)?
        );
        Ok(())
    }

    #[test]
    fn missing_sdk_and_runtime_instructions_are_actionable() {
        assert!(
            super::sdk_setup_instruction("aarch64-apple-darwin")
                .contains("prepare-gstreamer-macos-sdk.sh")
        );
        assert!(
            super::runtime_setup_instruction("aarch64-apple-darwin")
                .contains("prepare-gstreamer-macos-runtime.sh")
        );
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
    fn native_verification_timeout_terminates_a_stuck_command() {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "sleep 1"]);
        let error = super::command_status_with_timeout_duration(
            &mut command,
            "stuck verifier",
            std::time::Duration::from_millis(10),
        )
        .expect_err("stuck native verifier should be terminated");
        assert!(error.to_string().contains("timeout"));
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

    #[cfg(unix)]
    #[test]
    fn release_sdk_verification_requires_the_minimum_abi_and_is_hermetic() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TestDir::new()?;
        let pkg_config = temp_dir.path.join("pkg-config");
        fs::write(
            &pkg_config,
            "#!/usr/bin/env sh\n[ \"$1\" = \"--atleast-version=1.20\" ] && [ \"$2\" = \"gstreamer-1.0\" ] && [ -n \"$PKG_CONFIG_PATH\" ] && [ -z \"${PKG_CONFIG_LIBDIR:-}\" ]\n",
        )?;
        fs::set_permissions(&pkg_config, fs::Permissions::from_mode(0o755))?;

        super::verify_sdk_minimum_version(pkg_config.as_os_str(), &temp_dir.path, "1.20")?;
        let error =
            super::verify_sdk_minimum_version(pkg_config.as_os_str(), &temp_dir.path, "1.21")
                .expect_err("the SDK must satisfy the requested ABI minimum");
        assert!(error.to_string().contains("does not satisfy"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn release_sdk_version_must_match_the_private_runtime() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TestDir::new()?;
        let pkg_config = temp_dir.path.join("pkg-config");
        fs::write(
            &pkg_config,
            "#!/usr/bin/env sh\n[ \"$1\" = \"--exact-version=1.28.6\" ] && [ \"$2\" = \"gstreamer-1.0\" ] && [ -n \"$PKG_CONFIG_PATH\" ]\n",
        )?;
        fs::set_permissions(&pkg_config, fs::Permissions::from_mode(0o755))?;

        super::verify_sdk_exact_version(pkg_config.as_os_str(), &temp_dir.path, "1.28.6")?;
        let error =
            super::verify_sdk_exact_version(pkg_config.as_os_str(), &temp_dir.path, "1.28.5")
                .expect_err("the SDK must match the private runtime release");
        assert!(error.to_string().contains("does not match"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn macos_runtime_preparation_reports_script_failures() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TestDir::new()?;
        let script = temp_dir
            .path
            .join("script/prepare-gstreamer-macos-runtime.sh");
        fs::create_dir_all(script.parent().expect("script parent"))?;
        fs::write(&script, "#!/usr/bin/env sh\nexit 1\n")?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;

        let error = super::prepare_macos_runtime(&temp_dir.path)
            .expect_err("a failed runtime preparation script must be reported");
        assert!(error.to_string().contains("runtime preparation failed"));
        assert!(
            error
                .to_string()
                .contains("prepare-gstreamer-macos-runtime.sh")
        );

        let sdk_script = temp_dir.path.join("script/prepare-gstreamer-macos-sdk.sh");
        fs::write(&sdk_script, "#!/usr/bin/env sh\nexit 1\n")?;
        fs::set_permissions(&sdk_script, fs::Permissions::from_mode(0o755))?;
        let error = super::prepare_macos_sdk(&temp_dir.path)
            .expect_err("a failed SDK preparation script must be reported");
        assert!(error.to_string().contains("SDK preparation failed"));
        assert!(error.to_string().contains("prepare-gstreamer-macos-sdk.sh"));
        Ok(())
    }
}
