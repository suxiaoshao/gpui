//! Linux private GStreamer runtime staging and ELF loader verification.
//!
//! The caller supplies a release-produced private prefix and the manifest
//! allow-list. This module never copies a host package by discovery.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::error::{Result, XtaskError};

/// Linux target supported by the private runtime producer.
pub(crate) const SUPPORTED_TARGET: &str = "x86_64-unknown-linux-gnu";
pub(crate) const PRODUCER_IMAGE: &str = "ubuntu:22.04";
pub(crate) const MINIMUM_GLIBC: &str = "2.35";

/// The final Debian resource location created by tauri-bundler for the HTTP
/// Client product name. The whitespace is intentional and must remain part of
/// the ELF RUNPATH literal.
const APP_RUNPATH: &str = "$ORIGIN/../lib/HTTP Client/gstreamer/lib";
const LIB_RUNPATH: &str = "$ORIGIN";
const PLUGIN_RUNPATH: &str = "$ORIGIN/..";
const SCANNER_RUNPATH: &str = "$ORIGIN/../../lib";
const BIN_RUNPATH: &str = "$ORIGIN/../lib";

/// Result of staging a private Linux runtime into a tauri resource tree.
#[derive(Debug, Default)]
pub(crate) struct LinuxRuntimeStage {
    pub(crate) resources_map: BTreeMap<String, String>,
    pub(crate) runtime_root: PathBuf,
}

/// Stages an allow-listed Cerbero prefix and writes the required relative ELF
/// RUNPATH values. `main_binary` must already be a copy in the platform bundle
/// staging directory, before tauri-bundler packages it.
pub(crate) fn stage_private_runtime(
    private_prefix: &Path,
    allowed_files: &[PathBuf],
    bundle_staging: &Path,
    main_binary: &Path,
) -> Result<LinuxRuntimeStage> {
    let patchelf = env::var_os("GPUI_PATCHELF")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("patchelf"));
    stage_private_runtime_with_patchelf(
        private_prefix,
        allowed_files,
        bundle_staging,
        main_binary,
        &patchelf,
    )
}

/// Same as [`stage_private_runtime`], with an explicit patchelf command for
/// hermetic tests and release runners.
pub(crate) fn stage_private_runtime_with_patchelf(
    private_prefix: &Path,
    allowed_files: &[PathBuf],
    bundle_staging: &Path,
    main_binary: &Path,
    patchelf: &Path,
) -> Result<LinuxRuntimeStage> {
    let private_prefix = canonical_directory(private_prefix, "Linux GStreamer private prefix")?;
    if !bundle_staging.is_dir() {
        return Err(XtaskError::msg(format!(
            "Linux bundle staging directory {} does not exist",
            bundle_staging.display()
        )));
    }
    if !main_binary.is_file() || !main_binary.starts_with(bundle_staging) {
        return Err(XtaskError::msg(format!(
            "Linux main binary {} must be a file inside bundle staging {}",
            main_binary.display(),
            bundle_staging.display()
        )));
    }

    let runtime_root = bundle_staging.join("gstreamer");
    if runtime_root.exists() {
        fs::remove_dir_all(&runtime_root).map_err(|err| {
            XtaskError::msg(format!(
                "failed to clean previous Linux GStreamer staging directory {}: {err}",
                runtime_root.display()
            ))
        })?;
    }
    fs::create_dir_all(&runtime_root).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create Linux GStreamer staging directory {}: {err}",
            runtime_root.display()
        ))
    })?;

    let mut resources_map = BTreeMap::new();
    let mut copied_sources = BTreeSet::new();
    for relative in allowed_files {
        let relative = checked_relative_path(relative, "Linux runtime file")?;
        if !copied_sources.insert(relative.to_path_buf()) {
            return Err(XtaskError::msg(format!(
                "Linux GStreamer allow-list repeats {}",
                relative.display()
            )));
        }
        let source = canonical_regular_file(&private_prefix, relative, "Linux runtime file")?;
        let destination = runtime_root.join(relative);
        copy_file(&source, &destination)?;
        resources_map.insert(
            destination.to_string_lossy().into_owned(),
            format!(
                "gstreamer/{}",
                relative.to_string_lossy().replace('\\', "/")
            ),
        );
    }

    if resources_map.is_empty() {
        return Err(XtaskError::msg(
            "Linux GStreamer allow-list must contain private runtime files",
        ));
    }
    patch_runpath(patchelf, main_binary, APP_RUNPATH)?;
    patch_staged_runtime_runpaths(patchelf, &runtime_root)?;
    Ok(LinuxRuntimeStage {
        resources_map,
        runtime_root,
    })
}

/// Runs `gst-inspect` using only a staged private runtime. This is intended
/// for an unpacked package smoke test, not as a replacement for UI testing.
pub(crate) fn verify_private_elements(
    inspector: &Path,
    runtime_root: &Path,
    registry_path: &Path,
    required_elements: &[String],
) -> Result<()> {
    let runtime_root = canonical_directory(runtime_root, "Linux GStreamer runtime root")?;
    let lib = runtime_root.join("lib");
    let plugins = lib.join("gstreamer-1.0");
    let scanner = runtime_root
        .join("libexec")
        .join("gstreamer-1.0")
        .join("gst-plugin-scanner");
    for path in [&lib, &plugins] {
        if !path.is_dir() {
            return Err(XtaskError::msg(format!(
                "required private Linux GStreamer directory is missing: {}",
                path.display()
            )));
        }
    }
    if !scanner.is_file() {
        return Err(XtaskError::msg(format!(
            "required private Linux GStreamer plugin scanner is missing: {}",
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
                "required Linux GStreamer element must not be empty",
            ));
        }
        let mut command =
            private_inspect_command(inspector, &lib, &plugins, &scanner, registry_path);
        command.arg(element);
        let status = super::command_status_with_timeout(
            &mut command,
            &format!("private Linux GStreamer inspector for `{element}`"),
        )?;
        if !status.success() {
            return Err(XtaskError::msg(format!(
                "private Linux GStreamer runtime does not provide required element `{element}`"
            )));
        }
    }
    Ok(())
}

/// Rejects unpacked-package dependency output that resolves GStreamer from the
/// host or leaves a dependency unresolved. Invoke it after `dpkg-deb -x` with
/// the package's final `runtime_root`, not with a development SDK prefix.
pub(crate) fn verify_no_host_gstreamer_dependencies(
    lddtree: &Path,
    binaries: &[PathBuf],
    runtime_root: &Path,
) -> Result<()> {
    if !lddtree.is_file() {
        return Err(XtaskError::msg(format!(
            "lddtree is missing: {}",
            lddtree.display()
        )));
    }
    let runtime_root = canonical_directory(runtime_root, "Linux GStreamer runtime root")?;
    for binary in binaries {
        if !binary.is_file() {
            return Err(XtaskError::msg(format!(
                "ELF dependency check input is missing: {}",
                binary.display()
            )));
        }
        let output = Command::new(lddtree)
            .arg("-l")
            .arg(binary)
            .output()
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to execute lddtree {}: {err}",
                    lddtree.display()
                ))
            })?;
        if !output.status.success() {
            return Err(XtaskError::msg(format!(
                "lddtree failed for {}",
                binary.display()
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("not found") {
            return Err(XtaskError::msg(format!(
                "unresolved ELF dependency while checking {}: {stdout}",
                binary.display()
            )));
        }
        for line in stdout.lines() {
            let candidate = Path::new(line.trim());
            let text = candidate.to_string_lossy();
            if text.contains("gstreamer") && !candidate.starts_with(&runtime_root) {
                return Err(XtaskError::msg(format!(
                    "{} resolves host GStreamer dependency outside {}: {}",
                    binary.display(),
                    runtime_root.display(),
                    text
                )));
            }
        }
    }
    Ok(())
}

fn private_inspect_command(
    inspector: &Path,
    lib: &Path,
    plugins: &Path,
    scanner: &Path,
    registry_path: &Path,
) -> Command {
    let mut command = Command::new(inspector);
    command
        .env("LD_LIBRARY_PATH", lib)
        .env("GST_PLUGIN_SYSTEM_PATH", plugins)
        .env("GST_PLUGIN_SYSTEM_PATH_1_0", plugins)
        .env("GST_PLUGIN_SCANNER", scanner)
        .env("GST_PLUGIN_SCANNER_1_0", scanner)
        .env("GST_REGISTRY_1_0", registry_path)
        .env_remove("GST_PLUGIN_PATH")
        .env_remove("GST_PLUGIN_PATH_1_0");
    command
}

fn patch_staged_runtime_runpaths(patchelf: &Path, runtime_root: &Path) -> Result<()> {
    for path in walk_regular_files(runtime_root)? {
        if !is_elf(&path)? {
            continue;
        }
        let relative = path.strip_prefix(runtime_root).map_err(|err| {
            XtaskError::msg(format!(
                "failed to resolve staged runtime path {} relative to {}: {err}",
                path.display(),
                runtime_root.display()
            ))
        })?;
        let runpath = runtime_runpath(relative).ok_or_else(|| {
            XtaskError::msg(format!(
                "staged ELF {} is outside the supported private GStreamer layout",
                relative.display()
            ))
        })?;
        patch_runpath(patchelf, &path, runpath)?;
    }
    Ok(())
}

fn runtime_runpath(relative: &Path) -> Option<&'static str> {
    let components: Vec<_> = relative.components().collect();
    match components.as_slice() {
        [Component::Normal(first), ..] if *first == "lib" => {
            if matches!(components.get(1), Some(Component::Normal(second)) if *second == "gstreamer-1.0")
            {
                Some(PLUGIN_RUNPATH)
            } else {
                Some(LIB_RUNPATH)
            }
        }
        [
            Component::Normal(first),
            Component::Normal(second),
            Component::Normal(_),
        ] if *first == "libexec" && *second == "gstreamer-1.0" => Some(SCANNER_RUNPATH),
        [Component::Normal(first), ..] if *first == "bin" => Some(BIN_RUNPATH),
        _ => None,
    }
}

fn patch_runpath(patchelf: &Path, path: &Path, runpath: &str) -> Result<()> {
    let status = Command::new(patchelf)
        .args(["--set-rpath", runpath])
        .arg(path)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute patchelf {} for {}: {err}",
                patchelf.display(),
                path.display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "patchelf failed while setting RUNPATH on {}",
            path.display()
        )));
    }
    Ok(())
}

fn walk_regular_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to walk staged Linux runtime {}: {err}",
                root.display()
            ))
        })?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

fn is_elf(path: &Path) -> Result<bool> {
    use std::io::Read;

    let mut file = fs::File::open(path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to open staged file {}: {err}",
            path.display()
        ))
    })?;
    let mut magic = [0_u8; 4];
    let read = file.read(&mut magic).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read staged file {}: {err}",
            path.display()
        ))
    })?;
    Ok(read == magic.len() && magic == [0x7f, b'E', b'L', b'F'])
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
            "failed to create Linux runtime destination directory {}: {err}",
            parent.display()
        ))
    })?;
    fs::copy(source, destination).map_err(|err| {
        XtaskError::msg(format!(
            "failed to stage Linux runtime file {} to {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MINIMUM_GLIBC, PRODUCER_IMAGE, SUPPORTED_TARGET, stage_private_runtime_with_patchelf,
    };
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
                "xtask-gstreamer-linux-{timestamp}-{}-{id}",
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

    #[cfg(unix)]
    fn fake_patchelf(root: &std::path::Path) -> Result<PathBuf> {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("patchelf");
        fs::write(&path, "#!/usr/bin/env sh\nexit 0\n")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    #[cfg(unix)]
    #[test]
    fn stages_private_layout_and_patches_elfs() -> Result<()> {
        let root = TestDir::new()?;
        let prefix = root.0.join("prefix");
        let staging = root.0.join("bundle");
        let main = staging.join("http-client");
        fs::create_dir_all(prefix.join("lib/gstreamer-1.0"))?;
        fs::create_dir_all(prefix.join("libexec/gstreamer-1.0"))?;
        fs::create_dir_all(&staging)?;
        fs::write(&main, [0x7f, b'E', b'L', b'F'])?;
        fs::write(
            prefix.join("lib/libgstreamer-1.0.so.0"),
            [0x7f, b'E', b'L', b'F'],
        )?;
        fs::write(
            prefix.join("lib/gstreamer-1.0/libgstplayback.so"),
            [0x7f, b'E', b'L', b'F'],
        )?;
        fs::write(
            prefix.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
            [0x7f, b'E', b'L', b'F'],
        )?;

        let stage = stage_private_runtime_with_patchelf(
            &prefix,
            &[
                PathBuf::from("lib/libgstreamer-1.0.so.0"),
                PathBuf::from("lib/gstreamer-1.0/libgstplayback.so"),
                PathBuf::from("libexec/gstreamer-1.0/gst-plugin-scanner"),
            ],
            &staging,
            &main,
            &fake_patchelf(&root.0)?,
        )?;

        assert_eq!(SUPPORTED_TARGET, "x86_64-unknown-linux-gnu");
        assert_eq!(PRODUCER_IMAGE, "ubuntu:22.04");
        assert_eq!(MINIMUM_GLIBC, "2.35");
        assert!(
            stage
                .runtime_root
                .join("lib/libgstreamer-1.0.so.0")
                .is_file()
        );
        assert!(
            stage
                .resources_map
                .values()
                .any(|target| target == "gstreamer/lib/gstreamer-1.0/libgstplayback.so")
        );
        Ok(())
    }

    #[test]
    fn rejects_an_empty_allow_list() -> Result<()> {
        let root = TestDir::new()?;
        let prefix = root.0.join("prefix");
        let staging = root.0.join("bundle");
        let main = staging.join("http-client");
        fs::create_dir_all(&prefix)?;
        fs::create_dir_all(&staging)?;
        fs::write(&main, [0x7f, b'E', b'L', b'F'])?;
        let error = stage_private_runtime_with_patchelf(
            &prefix,
            &[],
            &staging,
            &main,
            std::path::Path::new("unused-patchelf"),
        )
        .expect_err("empty allow-list must fail before patchelf execution");
        assert!(error.to_string().contains("allow-list"));
        Ok(())
    }
}
