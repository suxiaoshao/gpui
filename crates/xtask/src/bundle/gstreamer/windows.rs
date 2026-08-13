//! Windows private GStreamer runtime staging.
//!
//! This module is intentionally independent from the app-local runtime
//! manifest parser. Its caller supplies the already allow-listed files; this
//! module only verifies their containment and maps the Windows loader layout.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::error::{Result, XtaskError};

/// Result of staging an allow-listed Windows runtime.
///
/// `resources_map` can be merged into `tauri_bundler::BundleSettings`.
/// Core DLLs are deliberately absent from that map: they live beside the app
/// executable, where Windows resolves its direct GStreamer dependency before
/// Rust application code runs.
#[derive(Debug, Default)]
pub(crate) struct WindowsRuntimeStage {
    pub(crate) app_root_dlls: Vec<PathBuf>,
    pub(crate) resources_map: BTreeMap<String, String>,
}

/// Copies one private runtime allow-list into the Windows bundle layout.
///
/// Files below `bin/*.dll` are copied directly to `bundle_staging`. Every
/// other file is copied below `bundle_staging/gstreamer`, and is returned as a
/// resource mapping whose bundle target is `gstreamer/<source-relative-path>`.
/// The caller must create a fresh staging directory containing the app EXE.
pub(crate) fn stage_private_runtime(
    runtime_root: &Path,
    allowed_files: &[PathBuf],
    bundle_staging: &Path,
) -> Result<WindowsRuntimeStage> {
    let runtime_root = canonical_directory(runtime_root, "Windows GStreamer runtime root")?;
    if !bundle_staging.is_dir() {
        return Err(XtaskError::msg(format!(
            "Windows bundle staging directory {} does not exist",
            bundle_staging.display()
        )));
    }

    let private_root = bundle_staging.join("gstreamer");
    if private_root.exists() {
        fs::remove_dir_all(&private_root).map_err(|err| {
            XtaskError::msg(format!(
                "failed to clean previous Windows GStreamer staging directory {}: {err}",
                private_root.display()
            ))
        })?;
    }
    fs::create_dir_all(&private_root).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create Windows GStreamer staging directory {}: {err}",
            private_root.display()
        ))
    })?;

    let mut copied_sources = BTreeSet::new();
    let mut stage = WindowsRuntimeStage::default();
    for relative in allowed_files {
        let relative = checked_relative_path(relative, "Windows runtime file")?;
        if !copied_sources.insert(relative.to_path_buf()) {
            return Err(XtaskError::msg(format!(
                "Windows GStreamer allow-list repeats {}",
                relative.display()
            )));
        }

        let source = canonical_regular_file(&runtime_root, relative, "Windows runtime file")?;
        if is_bin_dll(relative) {
            let file_name = relative.file_name().ok_or_else(|| {
                XtaskError::msg(format!(
                    "Windows runtime DLL {} has no file name",
                    relative.display()
                ))
            })?;
            let destination = bundle_staging.join(file_name);
            if destination.exists() {
                return Err(XtaskError::msg(format!(
                    "Windows GStreamer DLL destination {} already exists; use a fresh bundle staging directory",
                    destination.display()
                )));
            }
            copy_file(&source, &destination)?;
            stage.app_root_dlls.push(destination);
        }

        let destination = private_root.join(relative);
        copy_file(&source, &destination)?;
        stage.resources_map.insert(
            destination.to_string_lossy().into_owned(),
            format!(
                "gstreamer/{}",
                relative.to_string_lossy().replace('\\', "/")
            ),
        );
    }

    if stage.app_root_dlls.is_empty() {
        return Err(XtaskError::msg(
            "Windows GStreamer allow-list must contain at least one bin/*.dll core runtime file",
        ));
    }
    Ok(stage)
}

/// Executes GStreamer element checks against a private Windows runtime.
///
/// The caller chooses the inspector binary, normally the SDK's
/// `bin/gst-inspect-1.0.exe`. The command inherits no GStreamer plugin path;
/// only the staged runtime plugin directory is visible.
pub(crate) fn verify_private_elements(
    inspector: &Path,
    runtime_root: &Path,
    registry_path: &Path,
    required_elements: &[String],
) -> Result<()> {
    let runtime_root = canonical_directory(runtime_root, "Windows GStreamer runtime root")?;
    let bin = runtime_root.join("bin");
    let plugins = runtime_root.join("lib").join("gstreamer-1.0");
    let scanner = runtime_root
        .join("libexec")
        .join("gstreamer-1.0")
        .join("gst-plugin-scanner.exe");
    for path in [&bin, &plugins] {
        if !path.is_dir() {
            return Err(XtaskError::msg(format!(
                "required private Windows GStreamer directory is missing: {}",
                path.display()
            )));
        }
    }
    if !scanner.is_file() {
        return Err(XtaskError::msg(format!(
            "required private Windows GStreamer plugin scanner is missing: {}",
            scanner.display()
        )));
    }
    if !inspector.is_file() {
        return Err(XtaskError::msg(format!(
            "GStreamer inspector is missing: {}",
            inspector.display()
        )));
    }

    let registry_parent = registry_path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve registry parent for {}",
            registry_path.display()
        ))
    })?;
    fs::create_dir_all(registry_parent).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create GStreamer registry directory {}: {err}",
            registry_parent.display()
        ))
    })?;

    for element in required_elements {
        if element.trim().is_empty() {
            return Err(XtaskError::msg(
                "required Windows GStreamer element must not be empty",
            ));
        }
        let mut command =
            private_inspect_command(inspector, &bin, &plugins, &scanner, registry_path);
        command.arg(element);
        let status = super::command_status_with_timeout(
            &mut command,
            &format!("private Windows GStreamer inspector for `{element}`"),
        )?;
        if !status.success() {
            return Err(XtaskError::msg(format!(
                "private Windows GStreamer runtime does not provide required element `{element}`"
            )));
        }
    }
    Ok(())
}

fn private_inspect_command(
    inspector: &Path,
    bin: &Path,
    plugins: &Path,
    scanner: &Path,
    registry_path: &Path,
) -> Command {
    let mut command = Command::new(inspector);
    let path = env::var_os("PATH").unwrap_or_default();
    let private_path = format!("{};{}", bin.display(), path.to_string_lossy());
    command
        .env("PATH", private_path)
        .env("GST_PLUGIN_SYSTEM_PATH", plugins)
        .env("GST_PLUGIN_SYSTEM_PATH_1_0", plugins)
        .env("GST_PLUGIN_SCANNER", scanner)
        .env("GST_PLUGIN_SCANNER_1_0", scanner)
        .env("GST_REGISTRY_1_0", registry_path)
        .env_remove("GST_PLUGIN_PATH")
        .env_remove("GST_PLUGIN_PATH_1_0");
    command
}

fn is_bin_dll(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(name)) if name.eq_ignore_ascii_case("bin"))
        && components.next().is_some()
        && components.next().is_none()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve {label} {}: {err}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(XtaskError::msg(format!(
            "{label} {} is not a directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn checked_relative_path<'a>(path: &'a Path, label: &str) -> Result<&'a Path> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(XtaskError::msg(format!(
            "{label} {} must be a non-empty relative path",
            path.display()
        )));
    }
    Ok(path)
}

fn canonical_regular_file(root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
    let candidate = root.join(relative);
    let canonical = fs::canonicalize(&candidate).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve {label} {}: {err}",
            candidate.display()
        ))
    })?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(XtaskError::msg(format!(
            "{label} {} escapes the runtime root or is not a regular file",
            candidate.display()
        )));
    }
    Ok(canonical)
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve destination parent for {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create Windows runtime destination directory {}: {err}",
            parent.display()
        ))
    })?;
    fs::copy(source, destination).map_err(|err| {
        XtaskError::msg(format!(
            "failed to stage Windows runtime file {} to {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::stage_private_runtime;
    use crate::error::Result;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Result<Self> {
            let id = NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "xtask-gstreamer-windows-{timestamp}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stages_bin_dlls_beside_the_app_and_plugins_as_resources() -> Result<()> {
        let root = TestDir::new()?;
        let runtime = root.0.join("runtime");
        let staging = root.0.join("bundle");
        fs::create_dir_all(runtime.join("bin"))?;
        fs::create_dir_all(runtime.join("lib/gstreamer-1.0"))?;
        fs::create_dir_all(runtime.join("libexec/gstreamer-1.0"))?;
        fs::create_dir_all(&staging)?;
        fs::write(runtime.join("bin/gstreamer-1.0-0.dll"), "core")?;
        fs::write(
            runtime.join("lib/gstreamer-1.0/libgstplayback.dll"),
            "plugin",
        )?;
        fs::write(
            runtime.join("libexec/gstreamer-1.0/gst-plugin-scanner.exe"),
            "scanner",
        )?;

        let stage = stage_private_runtime(
            &runtime,
            &[
                PathBuf::from("bin/gstreamer-1.0-0.dll"),
                PathBuf::from("lib/gstreamer-1.0/libgstplayback.dll"),
                PathBuf::from("libexec/gstreamer-1.0/gst-plugin-scanner.exe"),
            ],
            &staging,
        )?;

        assert!(staging.join("gstreamer-1.0-0.dll").is_file());
        assert!(
            staging
                .join("gstreamer/lib/gstreamer-1.0/libgstplayback.dll")
                .is_file()
        );
        assert!(
            stage
                .resources_map
                .values()
                .any(|target| target == "gstreamer/lib/gstreamer-1.0/libgstplayback.dll")
        );
        assert_eq!(stage.app_root_dlls.len(), 1);
        Ok(())
    }

    #[test]
    fn rejects_a_runtime_path_that_escapes_the_root() -> Result<()> {
        let root = TestDir::new()?;
        let runtime = root.0.join("runtime");
        let staging = root.0.join("bundle");
        fs::create_dir_all(runtime.join("bin"))?;
        fs::create_dir_all(&staging)?;
        fs::write(runtime.join("bin/gstreamer-1.0-0.dll"), "core")?;

        let error = stage_private_runtime(&runtime, &[PathBuf::from("../outside.dll")], &staging)
            .expect_err("escaping path must fail");
        assert!(error.to_string().contains("relative path"));
        Ok(())
    }
}
