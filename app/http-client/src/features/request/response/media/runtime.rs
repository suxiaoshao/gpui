//! App-local GStreamer bootstrap for response media previews.
//!
//! Development builds deliberately leave the system SDK untouched. A bundled
//! packaged executable instead discovers only the runtime staged beside
//! that executable, isolates GStreamer plugin discovery from system plugins,
//! and puts its registry in the app's private writable cache before the first
//! `gst::init` call.

use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use gstreamer::{self as gst};

const CACHE_DIRECTORY: &str = "top.sushao.http-client/gstreamer-1.0";
const PLUGIN_SYSTEM_PATH_LEGACY: &str = "GST_PLUGIN_SYSTEM_PATH";
const PLUGIN_SYSTEM_PATH: &str = "GST_PLUGIN_SYSTEM_PATH_1_0";
const PLUGIN_SCANNER: &str = "GST_PLUGIN_SCANNER";
const PLUGIN_SCANNER_1_0: &str = "GST_PLUGIN_SCANNER_1_0";
const REGISTRY_PATH: &str = "GST_REGISTRY_1_0";
const EXTRA_PLUGIN_PATH: &str = "GST_PLUGIN_PATH";
const EXTRA_PLUGIN_PATH_1_0: &str = "GST_PLUGIN_PATH_1_0";

static INITIALIZED: OnceLock<Result<(), RuntimeBootstrapProblem>> = OnceLock::new();

/// Stable, redacted reasons that the private media runtime cannot start.
///
/// Paths, URL values and native loader diagnostics deliberately remain outside
/// this value: callers map every variant to viewer-local
/// `MediaProblemKind::RuntimeUnavailable`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeBootstrapProblem {
    ExecutablePath,
    BundledRuntimeMissing,
    PluginDirectoryMissing,
    PluginScannerMissing,
    RuntimeLibraryDirectoryMissing,
    CacheDirectory,
    Environment,
    Initialize,
}

impl fmt::Display for RuntimeBootstrapProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExecutablePath => "media runtime executable location is unavailable",
            Self::BundledRuntimeMissing => "bundled media runtime is unavailable",
            Self::PluginDirectoryMissing => "bundled media plugins are unavailable",
            Self::PluginScannerMissing => "bundled media plugin scanner is unavailable",
            Self::RuntimeLibraryDirectoryMissing => {
                "bundled media runtime library directory is unavailable"
            }
            Self::CacheDirectory => "media runtime cache directory is unavailable",
            Self::Environment => "media runtime environment could not be configured",
            Self::Initialize => "media runtime could not initialize",
        })
    }
}

impl Error for RuntimeBootstrapProblem {}

/// Configures and initializes GStreamer exactly once for this process.
///
/// Both audio and video must enter here before their respective backend calls:
/// the video fork still invokes `gst::init` internally, but by then the private
/// runtime environment is already frozen.
pub(crate) fn initialize_runtime() -> Result<(), RuntimeBootstrapProblem> {
    INITIALIZED.get_or_init(initialize_once).to_owned()
}

fn initialize_once() -> Result<(), RuntimeBootstrapProblem> {
    let executable = env::current_exe().map_err(|_| RuntimeBootstrapProblem::ExecutablePath)?;
    let platform = current_platform();
    let runtime = match runtime_root_for(&executable, platform)? {
        Some(runtime_root) => Some(private_runtime(
            runtime_root,
            &private_cache_root()?,
            platform,
        )?),
        None => None,
    };

    if let Some(runtime) = &runtime {
        // Unit tests may deliberately configure an isolated GStreamer
        // environment. Respect that harness rather than mutating process-wide
        // variables underneath it.
        if cfg!(test) && explicit_test_environment() {
            tracing::debug!(
                runtime = "test-environment",
                "using explicit GStreamer test environment"
            );
        } else {
            configure_private_runtime(runtime)?;
            tracing::debug!(
                runtime = "bundled",
                "configured app-local GStreamer runtime"
            );
        }
    } else {
        tracing::debug!(runtime = "system-sdk", "using development GStreamer SDK");
    }

    gst::init().map_err(|_| RuntimeBootstrapProblem::Initialize)?;
    if let Some(runtime) = &runtime {
        // Environment variables select the private scanner/registry before
        // initialization. An explicit scan makes the application-owned plugin
        // directory part of this process registry without consulting PATH or
        // a second system plugin directory.
        let _ = gst::Registry::get().scan_path(&runtime.plugin_directory);
    }
    Ok(())
}

#[allow(dead_code)] // Cross-platform bundle layouts are verified in unit tests on every host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimePlatform {
    MacOs,
    Windows,
    Linux,
}

const fn current_platform() -> RuntimePlatform {
    #[cfg(target_os = "macos")]
    {
        RuntimePlatform::MacOs
    }
    #[cfg(target_os = "windows")]
    {
        RuntimePlatform::Windows
    }
    #[cfg(target_os = "linux")]
    {
        RuntimePlatform::Linux
    }
}

struct PrivateRuntime {
    plugin_directory: PathBuf,
    scanner: PathBuf,
    registry: PathBuf,
    windows_bin: Option<PathBuf>,
}

#[cfg(test)]
fn discover_runtime(
    executable: &Path,
    cache_root: &Path,
    platform: RuntimePlatform,
) -> Result<Option<PrivateRuntime>, RuntimeBootstrapProblem> {
    let Some(runtime_root) = runtime_root_for(executable, platform)? else {
        return Ok(None);
    };
    private_runtime(runtime_root, cache_root, platform).map(Some)
}

fn private_runtime(
    runtime_root: PathBuf,
    cache_root: &Path,
    platform: RuntimePlatform,
) -> Result<PrivateRuntime, RuntimeBootstrapProblem> {
    let plugin_directory = runtime_root.join("lib/gstreamer-1.0");
    if !plugin_directory.is_dir() {
        return Err(RuntimeBootstrapProblem::PluginDirectoryMissing);
    }
    let scanner = plugin_scanner_for(&runtime_root, platform)
        .filter(|path| path.is_file())
        .ok_or(RuntimeBootstrapProblem::PluginScannerMissing)?;
    let windows_bin = if platform == RuntimePlatform::Windows {
        let bin = runtime_root.join("bin");
        if !bin.is_dir() {
            return Err(RuntimeBootstrapProblem::RuntimeLibraryDirectoryMissing);
        }
        Some(bin)
    } else {
        None
    };

    Ok(PrivateRuntime {
        plugin_directory,
        scanner,
        registry: cache_root.join("registry.bin"),
        windows_bin,
    })
}

fn runtime_root_for(
    executable: &Path,
    platform: RuntimePlatform,
) -> Result<Option<PathBuf>, RuntimeBootstrapProblem> {
    match platform {
        RuntimePlatform::Linux => {
            let Some(bin) = executable.parent() else {
                return Err(RuntimeBootstrapProblem::ExecutablePath);
            };
            let Some(usr) = bin.parent() else {
                return Ok(None);
            };
            if bin.file_name().is_none_or(|name| name != "bin")
                || usr.file_name().is_none_or(|name| name != "usr")
            {
                return Ok(None);
            }
            let root = linux_runtime_root();
            if !root.is_dir() {
                return Err(RuntimeBootstrapProblem::BundledRuntimeMissing);
            }
            Ok(Some(root))
        }
        RuntimePlatform::MacOs => {
            let Some(macos) = executable.parent() else {
                return Err(RuntimeBootstrapProblem::ExecutablePath);
            };
            let Some(contents) = macos.parent() else {
                return Ok(None);
            };
            if macos.file_name().is_none_or(|name| name != "MacOS")
                || contents.file_name().is_none_or(|name| name != "Contents")
            {
                return Ok(None);
            }
            let root = contents.join("Frameworks/GStreamer.framework/Versions/1.0");
            if !root.is_dir() {
                return Err(RuntimeBootstrapProblem::BundledRuntimeMissing);
            }
            Ok(Some(root))
        }
        RuntimePlatform::Windows => {
            let Some(parent) = executable.parent() else {
                return Err(RuntimeBootstrapProblem::ExecutablePath);
            };
            let root = parent.join("gstreamer");
            if root.is_dir() {
                Ok(Some(root))
            } else if cfg!(debug_assertions) {
                Ok(None)
            } else {
                Err(RuntimeBootstrapProblem::BundledRuntimeMissing)
            }
        }
    }
}

fn linux_runtime_root() -> PathBuf {
    PathBuf::from("/usr/lib/HTTP Client/gstreamer")
}

fn plugin_scanner_for(root: &Path, platform: RuntimePlatform) -> Option<PathBuf> {
    let name = match platform {
        RuntimePlatform::Windows => "gst-plugin-scanner.exe",
        RuntimePlatform::MacOs | RuntimePlatform::Linux => "gst-plugin-scanner",
    };
    [
        root.join("libexec/gstreamer-1.0").join(name),
        root.join("libexec").join(name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

fn private_cache_root() -> Result<PathBuf, RuntimeBootstrapProblem> {
    let root = dirs_next::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join(CACHE_DIRECTORY);
    fs::create_dir_all(&root).map_err(|_| RuntimeBootstrapProblem::CacheDirectory)?;
    Ok(root)
}

fn explicit_test_environment() -> bool {
    [
        PLUGIN_SYSTEM_PATH_LEGACY,
        PLUGIN_SYSTEM_PATH,
        PLUGIN_SCANNER,
        PLUGIN_SCANNER_1_0,
        REGISTRY_PATH,
    ]
    .into_iter()
    .any(|name| env::var_os(name).is_some())
}

fn configure_private_runtime(runtime: &PrivateRuntime) -> Result<(), RuntimeBootstrapProblem> {
    set_environment(
        PLUGIN_SYSTEM_PATH_LEGACY,
        runtime.plugin_directory.as_os_str(),
    )?;
    set_environment(PLUGIN_SYSTEM_PATH, runtime.plugin_directory.as_os_str())?;
    set_environment(PLUGIN_SCANNER, runtime.scanner.as_os_str())?;
    set_environment(PLUGIN_SCANNER_1_0, runtime.scanner.as_os_str())?;
    set_environment(REGISTRY_PATH, runtime.registry.as_os_str())?;
    unset_environment(EXTRA_PLUGIN_PATH);
    unset_environment(EXTRA_PLUGIN_PATH_1_0);
    if let Some(bin) = &runtime.windows_bin {
        let mut path = OsString::new();
        path.push(bin);
        if let Some(previous) = env::var_os("PATH") {
            path.push(if cfg!(windows) { ";" } else { ":" });
            path.push(previous);
        }
        set_environment("PATH", &path)?;
    }
    Ok(())
}

fn set_environment(
    name: &str,
    value: impl AsRef<std::ffi::OsStr>,
) -> Result<(), RuntimeBootstrapProblem> {
    // This is the process's one-time GStreamer bootstrap gate. It runs before
    // this app initializes GStreamer or spawns any media backend; no app code
    // reads or writes these GStreamer variables afterwards.
    unsafe { gst::glib::setenv(name, value, true) }
        .map_err(|_| RuntimeBootstrapProblem::Environment)
}

fn unset_environment(name: &str) {
    // See `set_environment`: this one-time gate owns the GStreamer variables
    // before the first native initialization.
    unsafe { gst::glib::unsetenv(name) };
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn macos_bundle_layout_requires_the_private_framework_runtime() {
        let temp = TempDir::new().unwrap();
        let executable = temp
            .path()
            .join("HTTP Client.app/Contents/MacOS/http-client");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "test").unwrap();
        let cache = temp.path().join("cache");

        assert!(matches!(
            discover_runtime(&executable, &cache, RuntimePlatform::MacOs),
            Err(RuntimeBootstrapProblem::BundledRuntimeMissing)
        ));

        let root = temp
            .path()
            .join("HTTP Client.app/Contents/Frameworks/GStreamer.framework/Versions/1.0");
        write_runtime_layout(&root, RuntimePlatform::MacOs);
        let runtime = discover_runtime(&executable, &cache, RuntimePlatform::MacOs)
            .unwrap()
            .expect("macOS app bundle has a private runtime");
        assert_eq!(runtime.plugin_directory, root.join("lib/gstreamer-1.0"));
        assert_eq!(runtime.registry, cache.join("registry.bin"));
        assert!(runtime.windows_bin.is_none());
    }

    #[test]
    fn windows_bundle_layout_uses_sibling_runtime_and_bin_directory() {
        let temp = TempDir::new().unwrap();
        let executable = temp.path().join("http-client.exe");
        fs::write(&executable, "test").unwrap();
        let cache = temp.path().join("cache");
        assert!(
            discover_runtime(&executable, &cache, RuntimePlatform::Windows)
                .unwrap()
                .is_none()
        );

        let root = temp.path().join("gstreamer");
        write_runtime_layout(&root, RuntimePlatform::Windows);
        let runtime = discover_runtime(&executable, &cache, RuntimePlatform::Windows)
            .unwrap()
            .expect("Windows staging has a sibling runtime");
        assert_eq!(runtime.windows_bin, Some(root.join("bin")));
        assert_eq!(
            runtime.scanner,
            root.join("libexec/gstreamer-1.0/gst-plugin-scanner.exe")
        );
    }

    #[test]
    fn linux_runtime_root_is_the_fixed_system_install_location() {
        let temp = TempDir::new().unwrap();
        let executable = temp.path().join("target/debug/http-client");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "test").unwrap();

        assert_eq!(
            linux_runtime_root(),
            PathBuf::from("/usr/lib/HTTP Client/gstreamer")
        );
        assert!(
            runtime_root_for(&executable, RuntimePlatform::Linux)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn non_bundle_development_executables_keep_system_sdk() {
        let temp = TempDir::new().unwrap();
        let executable = temp.path().join("target/debug/http-client");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "test").unwrap();
        let cache = temp.path().join("cache");

        assert!(
            discover_runtime(&executable, &cache, RuntimePlatform::Linux)
                .unwrap()
                .is_none()
        );
        assert!(
            discover_runtime(&executable, &cache, RuntimePlatform::MacOs)
                .unwrap()
                .is_none()
        );
    }

    fn write_runtime_layout(root: &Path, platform: RuntimePlatform) {
        fs::create_dir_all(root.join("lib/gstreamer-1.0")).unwrap();
        if platform == RuntimePlatform::Windows {
            fs::create_dir_all(root.join("bin")).unwrap();
        }
        let scanner = plugin_scanner_for_test(root, platform);
        fs::create_dir_all(scanner.parent().unwrap()).unwrap();
        fs::write(scanner, "test").unwrap();
    }

    fn plugin_scanner_for_test(root: &Path, platform: RuntimePlatform) -> PathBuf {
        let name = if platform == RuntimePlatform::Windows {
            "gst-plugin-scanner.exe"
        } else {
            "gst-plugin-scanner"
        };
        root.join("libexec/gstreamer-1.0").join(name)
    }
}
