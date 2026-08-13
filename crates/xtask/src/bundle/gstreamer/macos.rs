//! macOS staging and verification for a private GStreamer framework.
//!
//! The source is an already-expanded official `GStreamer.framework`. Staging
//! preserves its framework symlinks and relative loader layout; the caller is
//! responsible for obtaining and checksum-verifying that source archive.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::error::{Result, XtaskError};

const FRAMEWORK_NAME: &str = "GStreamer.framework";
const RUNTIME_RELATIVE_PATH: &str = "Versions/1.0";
const RUNTIME_RPATH: &str = "@executable_path/../Frameworks/GStreamer.framework/Versions/1.0/lib";

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

struct MachoIndex {
    aliases: BTreeMap<String, BTreeSet<PathBuf>>,
    canonical_files: BTreeSet<PathBuf>,
}

/// Stages a complete GStreamer framework into an application bundle.
///
/// The framework is copied without dereferencing symlinks, thinned to arm64 on
/// Apple Silicon, and then verified independently of Homebrew or a system
/// GStreamer installation. This intentionally does not sign the app: callers
/// must call [`crate::bundle::macos::finalize_ad_hoc_codesign`] after every
/// bundle mutation, including icon injection.
pub(crate) fn stage_private_runtime(
    source_framework: &Path,
    app_bundle: &Path,
    required_elements: &[String],
    required_plugins: &[String],
) -> Result<()> {
    let source_framework = fs::canonicalize(source_framework).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve GStreamer framework source {}: {err}",
            source_framework.display()
        ))
    })?;
    if !source_framework.is_dir() {
        return Err(XtaskError::msg(format!(
            "GStreamer framework source {} is not a directory",
            source_framework.display()
        )));
    }

    let destination = framework_destination(app_bundle);
    if source_framework == destination {
        return Err(XtaskError::msg(
            "GStreamer framework source must not be the bundle destination",
        ));
    }
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|err| {
            XtaskError::msg(format!(
                "failed to remove previous staged GStreamer framework {}: {err}",
                destination.display()
            ))
        })?;
    }
    copy_tree_preserving_symlinks(&source_framework, &destination)?;
    remove_unused_framework_umbrella(&destination)?;

    let main_binary = find_main_binary(app_bundle)?;
    rewrite_main_binary_homebrew_dependencies(&main_binary, &destination)?;
    prune_unreachable_macho_files(&destination, &main_binary, required_plugins)?;

    #[cfg(target_arch = "aarch64")]
    thin_tree_to_arm64(&destination)?;

    configure_main_binary_rpath(&main_binary)?;
    verify_bundle_closure(app_bundle, &main_binary)?;
    smoke_required_elements(&destination, required_elements)
}

fn remove_unused_framework_umbrella(framework: &Path) -> Result<()> {
    for relative in [
        Path::new(".gitignore"),
        Path::new("GStreamer"),
        Path::new("Versions/1.0/GStreamer"),
        Path::new("Versions/1.0/lib/GStreamer"),
    ] {
        let path = framework.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                return Err(XtaskError::msg(format!(
                    "unexpected directory at unused GStreamer framework umbrella path {}",
                    path.display()
                )));
            }
            Ok(_) => fs::remove_file(&path).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to remove unused GStreamer framework umbrella {}: {err}",
                    path.display()
                ))
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(XtaskError::msg(format!(
                    "failed to inspect GStreamer framework umbrella {}: {err}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn prune_unreachable_macho_files(
    framework: &Path,
    main_binary: &Path,
    required_plugins: &[String],
) -> Result<()> {
    if required_plugins.is_empty() {
        return Err(XtaskError::msg(
            "GStreamer private runtime requires at least one plugin root",
        ));
    }

    let runtime = runtime_root(framework);
    let mut roots = vec![
        runtime.join("bin/gst-inspect-1.0"),
        runtime.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
    ];
    for plugin in required_plugins {
        let plugin = plugin.trim();
        if plugin.is_empty()
            || !plugin
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(XtaskError::msg(format!(
                "invalid GStreamer plugin root `{plugin}`"
            )));
        }
        roots.push(
            runtime
                .join("lib/gstreamer-1.0")
                .join(format!("libgst{plugin}.dylib")),
        );
    }

    let macho_files = macho_files_under(framework)?;
    let macho_index = macho_aliases(framework, &macho_files)?;
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(fs::canonicalize(main_binary).map_err(|err| {
        XtaskError::msg(format!(
            "failed to resolve HTTP Client executable {}: {err}",
            main_binary.display()
        ))
    })?);
    for root in roots {
        let canonical = fs::canonicalize(&root).map_err(|err| {
            XtaskError::msg(format!(
                "required GStreamer runtime root {} is missing: {err}",
                root.display()
            ))
        })?;
        if !macho_index.canonical_files.contains(&canonical) {
            return Err(XtaskError::msg(format!(
                "required GStreamer runtime root {} is not Mach-O",
                root.display()
            )));
        }
        queue.push_back(canonical);
    }

    while let Some(owner) = queue.pop_front() {
        if owner.starts_with(framework) && !reachable.insert(owner.clone()) {
            continue;
        }
        for dependency in dependencies(&owner)? {
            if dependency.starts_with("/usr/lib/") || dependency.starts_with("/System/Library/") {
                continue;
            }
            let candidate =
                resolve_reachable_dependency(&owner, &dependency, framework, &macho_index.aliases)?;
            if candidate.starts_with(framework) && !reachable.contains(&candidate) {
                queue.push_back(candidate);
            }
        }
    }

    for path in macho_files {
        let canonical = fs::canonicalize(&path).map_err(|err| {
            XtaskError::msg(format!(
                "failed to resolve staged Mach-O {}: {err}",
                path.display()
            ))
        })?;
        if !reachable.contains(&canonical) {
            fs::remove_file(&path).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to remove unreachable staged Mach-O {}: {err}",
                    path.display()
                ))
            })?;
        }
    }
    remove_dangling_symlinks(framework)
}

fn macho_aliases(framework: &Path, macho_files: &[PathBuf]) -> Result<MachoIndex> {
    let canonical_macho_files = macho_files
        .iter()
        .map(|path| {
            fs::canonicalize(path).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to resolve staged Mach-O {}: {err}",
                    path.display()
                ))
            })
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let mut aliases = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for entry in WalkDir::new(framework).follow_links(false) {
        let entry =
            entry.map_err(|err| XtaskError::msg(format!("failed to walk {framework:?}: {err}")))?;
        if !(entry.file_type().is_file() || entry.file_type().is_symlink()) {
            continue;
        }
        let Ok(canonical) = fs::canonicalize(entry.path()) else {
            continue;
        };
        if !canonical_macho_files.contains(&canonical) {
            continue;
        }
        aliases
            .entry(entry.file_name().to_string_lossy().into_owned())
            .or_default()
            .insert(canonical);
    }
    Ok(MachoIndex {
        aliases,
        canonical_files: canonical_macho_files,
    })
}

fn resolve_reachable_dependency(
    owner: &Path,
    dependency: &str,
    framework: &Path,
    aliases: &BTreeMap<String, BTreeSet<PathBuf>>,
) -> Result<PathBuf> {
    if let Some(relative) = dependency.strip_prefix("@loader_path/") {
        let candidate = owner
            .parent()
            .ok_or_else(|| {
                XtaskError::msg(format!(
                    "failed to resolve Mach-O owner directory for {}",
                    owner.display()
                ))
            })?
            .join(relative);
        if let Ok(canonical) = fs::canonicalize(candidate)
            && canonical.starts_with(framework)
        {
            return Ok(canonical);
        }
    }

    if dependency.starts_with("@rpath/")
        || dependency.starts_with("@executable_path/")
        || dependency.starts_with('/')
    {
        let file_name = Path::new(dependency)
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                XtaskError::msg(format!(
                    "invalid Mach-O dependency `{dependency}` in {}",
                    owner.display()
                ))
            })?;
        if let Some(candidates) = aliases.get(file_name) {
            if candidates.len() == 1 {
                return Ok(candidates.iter().next().expect("one candidate").clone());
            }
            return Err(XtaskError::msg(format!(
                "Mach-O dependency `{dependency}` in {} matches multiple staged libraries",
                owner.display()
            )));
        }
    }

    Err(XtaskError::msg(format!(
        "reachable Mach-O {} has an unresolved non-system dependency {dependency}",
        owner.display()
    )))
}

fn remove_dangling_symlinks(root: &Path) -> Result<()> {
    let mut symlinks = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_symlink())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();
    symlinks.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in symlinks {
        match fs::canonicalize(&path) {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                fs::remove_file(&path).map_err(|err| {
                    XtaskError::msg(format!(
                        "failed to remove dangling staged symlink {}: {err}",
                        path.display()
                    ))
                })?;
            }
            Err(err) => {
                return Err(XtaskError::msg(format!(
                    "failed to resolve staged symlink {}: {err}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn framework_destination(app_bundle: &Path) -> PathBuf {
    app_bundle.join("Contents/Frameworks").join(FRAMEWORK_NAME)
}

fn runtime_root(framework: &Path) -> PathBuf {
    framework.join(RUNTIME_RELATIVE_PATH)
}

fn find_main_binary(app_bundle: &Path) -> Result<PathBuf> {
    let executable_dir = app_bundle.join("Contents/MacOS");
    let mut candidates = fs::read_dir(&executable_dir)
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to read app executable directory {}: {err}",
                executable_dir.display()
            ))
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();

    match candidates.as_slice() {
        [main_binary] => Ok(main_binary.clone()),
        [] => Err(XtaskError::msg(format!(
            "application bundle {} has no executable under {}",
            app_bundle.display(),
            executable_dir.display()
        ))),
        _ => Err(XtaskError::msg(format!(
            "application bundle {} has multiple executables under {}; pass a bundle with one main binary",
            app_bundle.display(),
            executable_dir.display()
        ))),
    }
}

fn copy_tree_preserving_symlinks(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|err| XtaskError::msg(format!("failed to inspect {}: {err}", source.display())))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target = fs::read_link(source).map_err(|err| {
            XtaskError::msg(format!(
                "failed to read symlink {}: {err}",
                source.display()
            ))
        })?;
        let parent = destination.parent().ok_or_else(|| {
            XtaskError::msg(format!(
                "failed to resolve destination parent for {}",
                destination.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            XtaskError::msg(format!("failed to create {}: {err}", parent.display()))
        })?;
        symlink(&target, destination).map_err(|err| {
            XtaskError::msg(format!(
                "failed to create symlink {} -> {}: {err}",
                destination.display(),
                target.display()
            ))
        })?;
        return Ok(());
    }

    if file_type.is_dir() {
        fs::create_dir_all(destination).map_err(|err| {
            XtaskError::msg(format!("failed to create {}: {err}", destination.display()))
        })?;
        fs::set_permissions(destination, metadata.permissions()).map_err(|err| {
            XtaskError::msg(format!(
                "failed to copy permissions to {}: {err}",
                destination.display()
            ))
        })?;
        for entry in fs::read_dir(source)
            .map_err(|err| XtaskError::msg(format!("failed to read {}: {err}", source.display())))?
        {
            let entry = entry.map_err(|err| {
                XtaskError::msg(format!(
                    "failed to read entry under {}: {err}",
                    source.display()
                ))
            })?;
            copy_tree_preserving_symlinks(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }

    if !file_type.is_file() {
        return Err(XtaskError::msg(format!(
            "unsupported GStreamer source file type at {}",
            source.display()
        )));
    }

    let parent = destination.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve destination parent for {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|err| XtaskError::msg(format!("failed to create {}: {err}", parent.display())))?;
    fs::copy(source, destination).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy GStreamer file {} to {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;
    fs::set_permissions(destination, metadata.permissions()).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy permissions to {}: {err}",
            destination.display()
        ))
    })?;
    Ok(())
}

#[cfg(target_arch = "aarch64")]
fn thin_tree_to_arm64(root: &Path) -> Result<()> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!("failed to walk staged GStreamer runtime: {err}"))
        })?;
        if !entry.file_type().is_file() || !is_macho(entry.path())? {
            continue;
        }
        thin_to_arm64(entry.path())?;
    }
    Ok(())
}

#[cfg(target_arch = "aarch64")]
fn thin_to_arm64(path: &Path) -> Result<()> {
    let output = Command::new("lipo")
        .args([OsStr::new("-archs")])
        .arg(path)
        .output()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to inspect architectures for {}: {err}",
                path.display()
            ))
        })?;
    if !output.status.success() {
        return Err(XtaskError::msg(format!(
            "lipo -archs failed for staged GStreamer file {}",
            path.display()
        )));
    }
    let architectures = parse_architectures(&String::from_utf8_lossy(&output.stdout));
    if architectures == BTreeSet::from(["arm64".to_string()]) {
        return Ok(());
    }
    if !architectures.contains("arm64") {
        return Err(XtaskError::msg(format!(
            "staged GStreamer file {} has no arm64 slice",
            path.display()
        )));
    }

    let temporary = temporary_peer_path(path, "thin");
    let status = Command::new("lipo")
        .args([OsStr::new("-thin"), OsStr::new("arm64")])
        .arg(path)
        .args([OsStr::new("-output"), temporary.as_os_str()])
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute lipo while thinning {} to arm64: {err}",
                path.display()
            ))
        })?;
    if !status.success() {
        let _ = fs::remove_file(&temporary);
        return Err(XtaskError::msg(format!(
            "lipo failed while thinning staged GStreamer file {} to arm64",
            path.display()
        )));
    }
    fs::rename(&temporary, path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to replace {} with its arm64 slice: {err}",
            path.display()
        ))
    })
}

fn parse_architectures(output: &str) -> BTreeSet<String> {
    output.split_whitespace().map(str::to_string).collect()
}

fn temporary_peer_path(path: &Path, suffix: &str) -> PathBuf {
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("runtime");
    path.with_file_name(format!(".{file_name}.{suffix}-{}-{id}", std::process::id()))
        .with_extension(nanos.to_string())
}

fn configure_main_binary_rpath(main_binary: &Path) -> Result<()> {
    for rpath in rpaths(main_binary)? {
        if is_disallowed_rpath(&rpath) {
            run_install_name_tool(main_binary, "-delete_rpath", &rpath)?;
        }
    }

    if !rpaths(main_binary)?
        .iter()
        .any(|rpath| rpath == RUNTIME_RPATH)
    {
        run_install_name_tool(main_binary, "-add_rpath", RUNTIME_RPATH)?;
    }
    Ok(())
}

fn rewrite_main_binary_homebrew_dependencies(main_binary: &Path, framework: &Path) -> Result<()> {
    let runtime_library_directory = runtime_root(framework).join("lib");
    let mut runtime_library_names = BTreeSet::new();
    for entry in fs::read_dir(&runtime_library_directory).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read staged GStreamer libraries {}: {err}",
            runtime_library_directory.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to read a staged GStreamer library under {}: {err}",
                runtime_library_directory.display()
            ))
        })?;
        if entry.path().is_file() {
            runtime_library_names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }

    for dependency in dependencies(main_binary)? {
        let Some(replacement) = homebrew_runtime_replacement(&dependency, &runtime_library_names)?
        else {
            continue;
        };
        run_install_name_change(main_binary, &dependency, &replacement)?;
    }
    Ok(())
}

fn homebrew_runtime_replacement(
    dependency: &str,
    runtime_library_names: &BTreeSet<String>,
) -> Result<Option<String>> {
    if !(dependency.starts_with("/opt/homebrew/") || dependency.starts_with("/usr/local/")) {
        return Ok(None);
    }
    let Some(file_name) = Path::new(dependency).file_name().and_then(OsStr::to_str) else {
        return Ok(None);
    };
    if !file_name.ends_with(".dylib") {
        return Ok(None);
    }
    if !runtime_library_names.contains(file_name) {
        return Err(XtaskError::msg(format!(
            "Homebrew native dependency {dependency} has no compatible counterpart in the staged private runtime"
        )));
    }
    Ok(Some(format!("@rpath/{file_name}")))
}

fn run_install_name_tool(main_binary: &Path, operation: &str, value: &str) -> Result<()> {
    let status = Command::new("install_name_tool")
        .args([OsStr::new(operation), OsStr::new(value)])
        .arg(main_binary)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute install_name_tool for {}: {err}",
                main_binary.display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "install_name_tool {operation} {value} failed for {}",
            main_binary.display()
        )));
    }
    Ok(())
}

fn run_install_name_change(main_binary: &Path, old: &str, new: &str) -> Result<()> {
    let status = Command::new("install_name_tool")
        .args([OsStr::new("-change"), OsStr::new(old), OsStr::new(new)])
        .arg(main_binary)
        .status()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to rewrite native dependency for {}: {err}",
                main_binary.display()
            ))
        })?;
    if !status.success() {
        return Err(XtaskError::msg(format!(
            "install_name_tool -change failed for {}",
            main_binary.display()
        )));
    }
    Ok(())
}

fn verify_bundle_closure(app_bundle: &Path, main_binary: &Path) -> Result<()> {
    let framework = framework_destination(app_bundle);
    let runtime = runtime_root(&framework);
    for required in [
        runtime.join("lib/libgstreamer-1.0.0.dylib"),
        runtime.join("lib/gstreamer-1.0"),
        runtime.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
        runtime.join("bin/gst-inspect-1.0"),
    ] {
        if !required.exists() {
            return Err(XtaskError::msg(format!(
                "staged GStreamer runtime is missing {}",
                required.display()
            )));
        }
    }

    let main_rpaths = rpaths(main_binary)?;
    if !main_rpaths.iter().any(|rpath| rpath == RUNTIME_RPATH) {
        return Err(XtaskError::msg(format!(
            "main executable {} does not contain required GStreamer rpath {RUNTIME_RPATH}",
            main_binary.display()
        )));
    }
    if main_rpaths.iter().any(|rpath| is_disallowed_rpath(rpath)) {
        return Err(XtaskError::msg(format!(
            "main executable {} retains a disallowed external runtime rpath",
            main_binary.display()
        )));
    }

    let bundle_files = files_under(&app_bundle.join("Contents"))?;
    for path in macho_files_under(&app_bundle.join("Contents"))? {
        for dependency in dependencies(&path)? {
            if is_disallowed_dependency(&dependency) {
                return Err(XtaskError::msg(format!(
                    "bundle Mach-O {} depends on external library {dependency}",
                    path.display()
                )));
            }
            verify_dependency_is_resolvable(&path, &dependency, &bundle_files)?;
        }
    }
    Ok(())
}

fn files_under(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry =
            entry.map_err(|err| XtaskError::msg(format!("failed to walk {root:?}: {err}")))?;
        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            paths.push(entry.path().to_path_buf());
        }
    }
    Ok(paths)
}

fn verify_dependency_is_resolvable(
    owner: &Path,
    dependency: &str,
    bundle_files: &[PathBuf],
) -> Result<()> {
    if dependency.starts_with("/usr/lib/") || dependency.starts_with("/System/Library/") {
        return Ok(());
    }
    if let Some(relative) = dependency.strip_prefix("@loader_path/") {
        let candidate = owner
            .parent()
            .ok_or_else(|| {
                XtaskError::msg(format!(
                    "failed to resolve Mach-O owner directory for {}",
                    owner.display()
                ))
            })?
            .join(relative);
        if candidate.exists() {
            return Ok(());
        }
    } else if dependency.starts_with("@rpath/") || dependency.starts_with("@executable_path/") {
        let file_name = Path::new(dependency).file_name();
        if file_name.is_some_and(|file_name| {
            bundle_files
                .iter()
                .any(|candidate| candidate.file_name() == Some(file_name))
        }) {
            return Ok(());
        }
    }

    Err(XtaskError::msg(format!(
        "bundle Mach-O {} has an unresolved non-system dependency {dependency}",
        owner.display()
    )))
}

fn smoke_required_elements(framework: &Path, required_elements: &[String]) -> Result<()> {
    if required_elements.is_empty() {
        return Err(XtaskError::msg(
            "GStreamer private runtime requires at least one smoke-test element",
        ));
    }
    let runtime = runtime_root(framework);
    let inspect = runtime.join("bin/gst-inspect-1.0");
    let plugin_directory = runtime.join("lib/gstreamer-1.0");
    let scanner = runtime.join("libexec/gstreamer-1.0/gst-plugin-scanner");
    let registry = TemporaryRegistry::new()?;

    for element in required_elements {
        let element = element.trim();
        if element.is_empty() {
            return Err(XtaskError::msg(
                "GStreamer private runtime has an empty required smoke-test element",
            ));
        }
        let mut command = Command::new(&inspect);
        command
            .args([OsStr::new("--exists"), OsStr::new(element)])
            .env("GST_PLUGIN_SYSTEM_PATH", &plugin_directory)
            .env("GST_PLUGIN_SYSTEM_PATH_1_0", &plugin_directory)
            .env_remove("GST_PLUGIN_PATH")
            .env_remove("GST_PLUGIN_PATH_1_0")
            .env("GST_PLUGIN_SCANNER", &scanner)
            .env("GST_PLUGIN_SCANNER_1_0", &scanner)
            .env("GST_REGISTRY_1_0", registry.path());
        let status = super::command_status_with_timeout(
            &mut command,
            &format!("bundled gst-inspect for `{element}`"),
        )?;
        if !status.success() {
            return Err(XtaskError::msg(format!(
                "bundled gst-inspect could not find required element `{element}`"
            )));
        }
    }
    Ok(())
}

/// Verifies a prepared private framework without consulting a host GStreamer
/// installation. Used by release artifact smoke jobs after unpacking the app.
pub(crate) fn verify_private_elements(
    framework: &Path,
    required_elements: &[String],
) -> Result<()> {
    smoke_required_elements(framework, required_elements)
}

struct TemporaryRegistry {
    directory: PathBuf,
}

impl TemporaryRegistry {
    fn new() -> Result<Self> {
        let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| XtaskError::msg(format!("failed to read system time: {err}")))?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "xtask-gstreamer-registry-{nanos}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).map_err(|err| {
            XtaskError::msg(format!(
                "failed to create GStreamer smoke registry directory {}: {err}",
                directory.display()
            ))
        })?;
        Ok(Self { directory })
    }

    fn path(&self) -> PathBuf {
        self.directory.join("registry.bin")
    }
}

impl Drop for TemporaryRegistry {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn macho_files_under(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry =
            entry.map_err(|err| XtaskError::msg(format!("failed to walk {root:?}: {err}")))?;
        if entry.file_type().is_file() && is_macho(entry.path())? {
            paths.push(entry.path().to_path_buf());
        }
    }
    Ok(paths)
}

fn is_macho(path: &Path) -> Result<bool> {
    let mut bytes = [0_u8; 4];
    let mut file = fs::File::open(path)
        .map_err(|err| XtaskError::msg(format!("failed to open {}: {err}", path.display())))?;
    let Ok(()) = file.read_exact(&mut bytes) else {
        return Ok(false);
    };
    Ok(matches!(
        bytes,
        [0xfe, 0xed, 0xfa, 0xce]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
            | [0xca, 0xfe, 0xba, 0xbf]
            | [0xbf, 0xba, 0xfe, 0xca]
    ))
}

fn rpaths(path: &Path) -> Result<Vec<String>> {
    let output = otool(path, "-l")?;
    Ok(parse_rpaths(&output))
}

fn dependencies(path: &Path) -> Result<Vec<String>> {
    let output = otool(path, "-L")?;
    Ok(parse_dependencies(&output))
}

fn otool(path: &Path, argument: &str) -> Result<String> {
    let output = Command::new("otool")
        .arg(argument)
        .arg(path)
        .output()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to execute otool for {}: {err}",
                path.display()
            ))
        })?;
    if !output.status.success() {
        return Err(XtaskError::msg(format!(
            "otool {argument} failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| XtaskError::msg(format!("otool returned invalid UTF-8: {err}")))
}

fn parse_rpaths(output: &str) -> Vec<String> {
    let lines = output.lines().collect::<Vec<_>>();
    let mut rpaths = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "cmd LC_RPATH" {
            continue;
        }
        let Some(path_line) = lines[index + 1..]
            .iter()
            .take_while(|line| !line.trim_start().starts_with("cmd "))
            .find_map(|line| line.trim().strip_prefix("path "))
        else {
            continue;
        };
        if let Some(path) = path_line.strip_suffix(" (offset 12)") {
            rpaths.insert(path.to_string());
        }
    }
    rpaths.into_iter().collect()
}

fn parse_dependencies(output: &str) -> Vec<String> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| line.trim().split_once(" ("))
        .map(|(dependency, _)| dependency.to_string())
        .collect()
}

fn is_disallowed_rpath(rpath: &str) -> bool {
    Path::new(rpath).is_absolute()
        && !rpath.starts_with("/usr/lib/")
        && !rpath.starts_with("/System/Library/")
}

fn is_disallowed_dependency(dependency: &str) -> bool {
    dependency.starts_with("/opt/homebrew/")
        || dependency.starts_with("/usr/local/")
        || dependency.starts_with("/Library/Frameworks/GStreamer.framework")
        || (dependency.starts_with('/') && dependency.contains("GStreamer.framework"))
}

#[cfg(test)]
mod tests {
    use super::{
        RUNTIME_RPATH, framework_destination, homebrew_runtime_replacement,
        is_disallowed_dependency, is_disallowed_rpath, parse_architectures, parse_dependencies,
        parse_rpaths, verify_dependency_is_resolvable,
    };
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn framework_destination_preserves_the_framework_layout() {
        assert_eq!(
            framework_destination(Path::new("HTTP Client.app")),
            Path::new("HTTP Client.app/Contents/Frameworks/GStreamer.framework")
        );
    }

    #[test]
    fn parser_reads_rpaths_from_otool_load_commands() {
        let rpaths = parse_rpaths(
            "Load command 12\n          cmd LC_RPATH\n      cmdsize 64\n         path /opt/homebrew/lib (offset 12)\nLoad command 13\n          cmd LC_RPATH\n      cmdsize 96\n         path @executable_path/../Frameworks/GStreamer.framework/Versions/1.0/lib (offset 12)\n",
        );
        assert_eq!(
            rpaths,
            vec!["/opt/homebrew/lib".to_string(), RUNTIME_RPATH.to_string()]
        );
    }

    #[test]
    fn parser_reads_otool_dependencies() {
        assert_eq!(
            parse_dependencies(
                "Example:\n\t@rpath/libgstreamer-1.0.0.dylib (compatibility version 1.0.0, current version 1.0.0)\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\n",
            ),
            vec![
                "@rpath/libgstreamer-1.0.0.dylib".to_string(),
                "/usr/lib/libSystem.B.dylib".to_string(),
            ]
        );
    }

    #[test]
    fn parser_distinguishes_thin_and_universal_macho_files() {
        assert_eq!(
            parse_architectures("arm64\n"),
            BTreeSet::from(["arm64".to_string()])
        );
        assert_eq!(
            parse_architectures("x86_64 arm64\n"),
            BTreeSet::from(["arm64".to_string(), "x86_64".to_string()])
        );
    }

    #[test]
    fn rejects_external_gstreamer_and_homebrew_paths() {
        assert!(is_disallowed_rpath("/opt/homebrew/opt/gstreamer/lib"));
        assert!(is_disallowed_rpath(
            "/Library/Frameworks/GStreamer.framework/Versions/1.0/lib"
        ));
        assert!(is_disallowed_rpath(
            "/workspace/target/gstreamer-sdk/macos/GStreamer.framework/Versions/1.0/lib"
        ));
        assert!(!is_disallowed_rpath("@loader_path/../Frameworks"));
        assert!(!is_disallowed_rpath("/System/Library/Frameworks"));
        assert!(is_disallowed_dependency(
            "/usr/local/lib/libgstreamer-1.0.0.dylib"
        ));
        assert!(is_disallowed_dependency(
            "/vendor/GStreamer.framework/Versions/1.0/GStreamer"
        ));
        assert!(!is_disallowed_dependency("@rpath/libgstreamer-1.0.0.dylib"));
    }

    #[test]
    fn maps_homebrew_gstreamer_dependencies_to_the_private_runtime() {
        let libraries = BTreeSet::from([
            "libgstreamer-1.0.0.dylib".to_string(),
            "libgstvideo-1.0.0.dylib".to_string(),
        ]);
        assert_eq!(
            homebrew_runtime_replacement(
                "/opt/homebrew/opt/gstreamer/lib/libgstreamer-1.0.0.dylib",
                &libraries,
            )
            .expect("matching runtime library should be rewritten"),
            Some("@rpath/libgstreamer-1.0.0.dylib".to_string())
        );
        assert_eq!(
            homebrew_runtime_replacement(
                "/usr/local/opt/gstreamer/lib/libgstvideo-1.0.0.dylib",
                &libraries,
            )
            .expect("Intel Homebrew path should be rewritten"),
            Some("@rpath/libgstvideo-1.0.0.dylib".to_string())
        );
    }

    #[test]
    fn refuses_a_homebrew_dependency_missing_from_the_private_runtime() {
        let error = homebrew_runtime_replacement(
            "/opt/homebrew/opt/gstreamer/lib/libgstapp-1.0.0.dylib",
            &BTreeSet::new(),
        )
        .expect_err("a missing private counterpart must fail closed");
        assert!(error.to_string().contains("no compatible counterpart"));
    }

    #[test]
    fn maps_transitive_homebrew_runtime_libraries_when_the_bundle_contains_them() {
        let libraries = BTreeSet::from([
            "libgstreamer-1.0.0.dylib".to_string(),
            "libglib-2.0.0.dylib".to_string(),
        ]);
        assert_eq!(
            homebrew_runtime_replacement(
                "/opt/homebrew/opt/glib/lib/libglib-2.0.0.dylib",
                &libraries,
            )
            .expect("bundled GLib should be rewritten"),
            Some("@rpath/libglib-2.0.0.dylib".to_string())
        );
    }

    #[test]
    fn leaves_system_and_already_portable_dependencies_unchanged() {
        let libraries = BTreeSet::from(["libgstreamer-1.0.0.dylib".to_string()]);
        for dependency in [
            "/usr/lib/libSystem.B.dylib",
            "@rpath/libgstreamer-1.0.0.dylib",
        ] {
            assert_eq!(
                homebrew_runtime_replacement(dependency, &libraries)
                    .expect("unrelated dependency should be ignored"),
                None
            );
        }
    }

    #[test]
    fn dependency_resolution_requires_a_bundled_rpath_target() {
        let owner = Path::new("HTTP Client.app/Contents/MacOS/http-client");
        let files = vec![Path::new(
            "HTTP Client.app/Contents/Frameworks/GStreamer.framework/Versions/1.0/lib/libgstreamer-1.0.0.dylib",
        )
        .to_path_buf()];
        verify_dependency_is_resolvable(owner, "@rpath/libgstreamer-1.0.0.dylib", &files)
            .expect("bundled dependency should resolve");
        let error = verify_dependency_is_resolvable(owner, "@rpath/libcrypto.3.dylib", &files)
            .expect_err("missing bundled dependency should fail");
        assert!(error.to_string().contains("unresolved"));
    }
}
