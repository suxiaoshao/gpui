use super::*;
use std::{collections::BTreeSet, fs};

const KINDS: [Kind; 4] = [Kind::Extension, Kind::Skill, Kind::Prompt, Kind::Theme];

pub(crate) fn scan(root: PathBuf, agents: Option<PathBuf>) -> Result<Catalog, Error> {
    let settings = read_json(&root.join("settings.json"))?;
    if !settings.is_object() {
        return Err(Error("Pi settings must be an object".into()));
    }
    let mut result = Catalog {
        root: root.clone(),
        ..Default::default()
    };
    for entry in settings["packages"].as_array().into_iter().flatten() {
        let Some(source) = entry.as_str().or_else(|| entry["source"].as_str()) else {
            continue;
        };
        let Some(path) = package_path(&root, source, &settings) else {
            result
                .warnings
                .push(format!("Package location unavailable: {source}"));
            continue;
        };
        let manifest = match read_json(&path.join("package.json")) {
            Ok(manifest) => manifest,
            Err(error) => {
                result.warnings.push(error.to_string());
                Value::Null
            }
        };
        if !path.exists() {
            result
                .warnings
                .push(format!("Package not installed: {source}"));
        }
        result.packages.push(Package {
            source: source.into(),
            path: path.clone(),
            version: manifest["version"].as_str().map(str::to_owned),
        });
        for kind in KINDS {
            let entries = &manifest["pi"][kind.key()];
            let filtered = entry.get(kind.key()).is_some() || entry["autoload"] == false;
            let paths = if entries.is_array() && (!strings(entries).is_empty() || !filtered) {
                collect_entries(&path, &strings(entries), kind, &mut result.warnings)
            } else if manifest.get("pi").is_none() || filtered || entry.is_object() {
                collect(&path.join(kind.key()), kind, &mut result.warnings)
            } else {
                vec![]
            };
            for file in paths {
                let enabled = if entry["autoload"] == false {
                    delta_enabled(&file, &strings(&entry[kind.key()]), &path)
                } else if entry[kind.key()].is_array() {
                    let patterns = strings(&entry[kind.key()]);
                    !patterns.is_empty() && enabled(&file, &patterns, &path)
                } else {
                    true
                };
                add(
                    &mut result,
                    kind,
                    file,
                    path.clone(),
                    Some(source.into()),
                    enabled,
                    false,
                );
            }
        }
    }
    for kind in KINDS {
        let entries = strings(&settings[kind.key()]);
        let overrides: Vec<_> = entries.iter().filter(|s| is_override(s)).cloned().collect();
        let patterns: Vec<_> = entries.iter().filter(|s| is_pattern(s)).cloned().collect();
        let plain: Vec<_> = entries.iter().filter(|s| !is_pattern(s)).cloned().collect();
        let explicit = collect_entries(&root, &plain, kind, &mut result.warnings);
        for path in explicit {
            let editable =
                matches!(kind, Kind::Skill | Kind::Prompt) && owned(&path, &root.join(kind.key()));
            let active = enabled(&path, &patterns, &root);
            add(
                &mut result,
                kind,
                path,
                root.clone(),
                None,
                active,
                editable,
            );
        }
        let mut paths = collect(&root.join(kind.key()), kind, &mut result.warnings);
        if matches!(kind, Kind::Prompt | Kind::Theme) {
            paths.retain(|p| p.parent() == Some(root.join(kind.key()).as_path()));
        }
        for path in paths {
            let editable =
                matches!(kind, Kind::Skill | Kind::Prompt) && owned(&path, &root.join(kind.key()));
            let active = enabled(&path, &overrides, &root);
            add(
                &mut result,
                kind,
                path,
                root.clone(),
                None,
                active,
                editable,
            );
        }
    }
    if let Some(agents) = agents {
        let overrides: Vec<_> = strings(&settings["skills"])
            .into_iter()
            .filter(|s| is_override(s))
            .collect();
        for path in collect_mode(
            &agents.join("skills"),
            Kind::Skill,
            true,
            &mut result.warnings,
        ) {
            let active = enabled(&path, &overrides, &agents);
            let editable = owned(&path, &agents.join("skills"));
            add(
                &mut result,
                Kind::Skill,
                path,
                agents.clone(),
                None,
                active,
                editable,
            );
        }
    }
    result.resources.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(result)
}

fn add(
    catalog: &mut Catalog,
    kind: Kind,
    path: PathBuf,
    base: PathBuf,
    package: Option<String>,
    enabled: bool,
    editable: bool,
) {
    // Pi keeps the first discovered path (package paths precede local paths).
    let real = path.canonicalize().unwrap_or_else(|_| path.clone());
    if catalog
        .resources
        .iter()
        .any(|r| r.path.canonicalize().unwrap_or_else(|_| r.path.clone()) == real)
    {
        return;
    }
    let mut name = if path.file_name().is_some_and(|s| s == "SKILL.md") {
        path.parent().and_then(Path::file_name)
    } else {
        path.file_stem()
    }
    .unwrap_or_default()
    .to_string_lossy()
    .into_owned();
    let mut description = String::new();
    if matches!(kind, Kind::Skill | Kind::Prompt) {
        match fs::read_to_string(&path) {
            Ok(text) => {
                let normalized = text.replace("\r\n", "\n");
                if let Some(front) = normalized
                    .strip_prefix("---\n")
                    .and_then(|s| s.split_once("\n---").map(|(s, _)| s))
                {
                    match serde_yaml_ng::from_str::<Value>(front) {
                        Ok(front) => {
                            if let Some(value) = front["name"].as_str() {
                                name = value.into();
                            }
                            if let Some(value) = front["description"].as_str() {
                                description = value.into();
                            }
                        }
                        Err(error) => catalog
                            .warnings
                            .push(format!("{}: {error}", path.display())),
                    }
                } else if kind == Kind::Prompt {
                    description = text.lines().next().unwrap_or_default().into();
                }
            }
            Err(error) => catalog
                .warnings
                .push(format!("{}: {error}", path.display())),
        }
    }
    catalog.resources.push(Resource {
        kind,
        path,
        base,
        package,
        name,
        description,
        enabled,
        editable,
    });
}

fn owned(path: &Path, root: &Path) -> bool {
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(path), Ok(root)) => path.starts_with(root),
        _ => false,
    }
}
fn is_pattern(s: &str) -> bool {
    is_override(s) || s.contains(['*', '?'])
}
fn is_override(s: &str) -> bool {
    s.starts_with(['!', '+', '-'])
}
fn collect_entries(
    base: &Path,
    entries: &[String],
    kind: Kind,
    warnings: &mut Vec<String>,
) -> Vec<PathBuf> {
    let overrides: Vec<_> = entries.iter().filter(|s| is_override(s)).cloned().collect();
    let mut found = BTreeSet::new();
    for entry in entries.iter().filter(|s| !is_override(s)) {
        let path = resolve(base, entry);
        if entry.contains(['*', '?']) {
            if let Ok(pattern) = globset::GlobBuilder::new(&path.to_string_lossy())
                .literal_separator(true)
                .build()
            {
                let matcher = pattern.compile_matcher();
                let prefix: PathBuf = Path::new(entry)
                    .components()
                    .take_while(|p| {
                        !p.as_os_str()
                            .to_string_lossy()
                            .contains(['*', '?', '{', '['])
                    })
                    .collect();
                let walker = ignore::WalkBuilder::new(resolve(base, &prefix.to_string_lossy()))
                    .hidden(true)
                    .ignore(false)
                    .git_ignore(false)
                    .git_exclude(false)
                    .git_global(false)
                    .parents(false)
                    .build();
                for candidate in walker.flatten() {
                    let path = candidate.path();
                    if matcher.is_match(path)
                        && path
                            .strip_prefix(base)
                            .unwrap_or(path)
                            .components()
                            .all(|p| {
                                !p.as_os_str().to_string_lossy().starts_with('.')
                                    || p.as_os_str() == ".."
                            })
                    {
                        found.extend(collect(path, kind, warnings));
                    }
                }
            }
        } else {
            found.extend(collect(&path, kind, warnings));
        }
    }
    found
        .into_iter()
        .filter(|p| enabled(p, &overrides, base))
        .collect()
}
fn collect(path: &Path, kind: Kind, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    collect_mode(path, kind, false, warnings)
}
fn collect_mode(path: &Path, kind: Kind, agents: bool, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    visit(
        path,
        kind,
        path,
        agents,
        &mut BTreeSet::new(),
        &mut paths,
        warnings,
    );
    paths
}
fn visit(
    path: &Path,
    kind: Kind,
    scan_root: &Path,
    agents: bool,
    seen: &mut BTreeSet<PathBuf>,
    paths: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let root = path == scan_root;
    if !path.exists() || (!root && ignored(path, scan_root)) {
        return;
    }
    let real = match path.canonicalize() {
        Ok(real) => real,
        Err(e) => {
            warnings.push(format!("{}: {e}", path.display()));
            return;
        }
    };
    if !seen.insert(real) {
        return;
    }
    if path.is_file() {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if match kind {
            Kind::Extension => matches!(ext, "js" | "ts"),
            Kind::Skill => {
                ext == "md" && (root || path.file_name().is_some_and(|s| s == "SKILL.md"))
            }
            Kind::Prompt => ext == "md",
            Kind::Theme => ext == "json",
        } {
            paths.push(path.to_owned());
        }
        return;
    }
    if kind == Kind::Skill
        && path.join("SKILL.md").is_file()
        && !ignored(&path.join("SKILL.md"), scan_root)
    {
        paths.push(path.join("SKILL.md"));
        return;
    }
    if kind == Kind::Extension {
        if let Ok(manifest) = read_json(&path.join("package.json"))
            && manifest["pi"]["extensions"].is_array()
        {
            paths.extend(collect_entries(
                path,
                &strings(&manifest["pi"]["extensions"]),
                kind,
                warnings,
            ));
            return;
        }
        for name in ["index.ts", "index.js"] {
            if path.join(name).is_file() {
                paths.push(path.join(name));
                return;
            }
        }
        if !root {
            return;
        }
    }
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry)
                        if !entry.file_name().to_string_lossy().starts_with('.')
                            && entry.file_name() != "node_modules" =>
                    {
                        // Pi discovers root-level skill .md files as well as SKILL.md directories.
                        if kind == Kind::Skill
                            && (if agents { !root } else { root })
                            && !ignored(&entry.path(), scan_root)
                            && entry.path().is_file()
                            && entry.path().extension().is_some_and(|e| e == "md")
                        {
                            paths.push(entry.path());
                        } else {
                            visit(
                                &entry.path(),
                                kind,
                                scan_root,
                                agents,
                                seen,
                                paths,
                                warnings,
                            );
                        }
                    }
                    Ok(_) => {}
                    Err(e) => warnings.push(e.to_string()),
                }
            }
        }
        Err(e) => warnings.push(format!("{}: {e}", path.display())),
    }
}

fn ignored(path: &Path, root: &Path) -> bool {
    let mut dirs: Vec<_> = path
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .take_while(|p| p.starts_with(root))
        .collect();
    dirs.reverse();
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    for dir in dirs {
        for name in [".gitignore", ".ignore", ".fdignore"] {
            let file = dir.join(name);
            if file.is_file() {
                builder.add(file);
            }
        }
    }
    builder.build().is_ok_and(|matcher| {
        matcher
            .matched_path_or_any_parents(path, path.is_dir())
            .is_ignore()
    })
}

pub(super) fn exact(path: &Path, pattern: &str, base: &Path) -> bool {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    path == resolve(base, pattern)
        || (path.file_name().is_some_and(|s| s == "SKILL.md")
            && path.parent() == Some(resolve(base, pattern).as_path()))
}
fn matches(path: &Path, pattern: &str, base: &Path) -> bool {
    let Ok(pattern) = globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
    else {
        return false;
    };
    let pattern = pattern.compile_matcher();
    let test = |p: &Path| {
        [
            p.to_string_lossy(),
            p.strip_prefix(base).unwrap_or(p).to_string_lossy(),
            p.file_name().unwrap_or_default().to_string_lossy(),
        ]
        .iter()
        .any(|s| pattern.is_match(s.as_ref()))
    };
    test(path)
        || (path.file_name().is_some_and(|s| s == "SKILL.md") && path.parent().is_some_and(test))
}
fn enabled(path: &Path, patterns: &[String], base: &Path) -> bool {
    let includes: Vec<_> = patterns.iter().filter(|s| !is_override(s)).collect();
    let mut active = includes.is_empty() || includes.iter().any(|p| matches(path, p, base));
    if patterns
        .iter()
        .any(|p| p.strip_prefix('!').is_some_and(|p| matches(path, p, base)))
    {
        active = false;
    }
    if patterns
        .iter()
        .any(|p| p.strip_prefix('+').is_some_and(|p| exact(path, p, base)))
    {
        active = true;
    }
    if patterns
        .iter()
        .any(|p| p.strip_prefix('-').is_some_and(|p| exact(path, p, base)))
    {
        active = false;
    }
    active
}
fn delta_enabled(path: &Path, patterns: &[String], base: &Path) -> bool {
    let mut active = false;
    for p in patterns {
        let target = p.strip_prefix(['!', '+', '-']).unwrap_or(p);
        if if p.starts_with(['+', '-']) {
            exact(path, target, base)
        } else {
            matches(path, target, base)
        } {
            active = !p.starts_with(['!', '-']);
        }
    }
    active
}

fn package_path(root: &Path, source: &str, settings: &Value) -> Option<PathBuf> {
    if let Some(spec) = source.strip_prefix("npm:") {
        let name = spec
            .rsplit_once('@')
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, _)| name)
            .unwrap_or(spec);
        if name.contains("..") || name.starts_with('/') {
            return None;
        }
        let managed = root.join("npm/node_modules").join(name);
        if managed.exists() {
            return Some(managed);
        }
        let mut command = strings(&settings["npmCommand"]);
        if command.is_empty() {
            command.push("npm".into());
        }
        let manager = command
            .iter()
            .rposition(|p| p == "--")
            .and_then(|i| command.get(i + 1))
            .unwrap_or(&command[0]);
        let manager = Path::new(manager).file_stem()?.to_string_lossy();
        let run = |args: &[&str]| {
            std::process::Command::new(&command[0])
                .args(&command[1..])
                .args(args)
                .current_dir(root)
                .stdin(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success())
        };
        if manager == "pnpm"
            && let Some(output) = run(&["list", "-g", "--depth", "0", "--json"])
            && let Ok(value) = serde_json::from_slice::<Value>(&output.stdout)
        {
            for entry in value.as_array().into_iter().flatten() {
                if let Some(path) = entry["dependencies"][name]["path"].as_str() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        let args: &[&str] = if manager == "bun" {
            &["pm", "bin", "-g"]
        } else {
            &["root", "-g"]
        };
        if let Some(output) = run(args) {
            let dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            let dir = if manager == "bun" {
                dir.parent()?.join("install/global/node_modules")
            } else {
                dir
            };
            let path = dir.join(name);
            if path.exists() {
                return Some(path);
            }
        }
        return Some(managed);
    }
    let explicit = source.starts_with("git:");
    let git = source.strip_prefix("git:").unwrap_or(source);
    if !explicit && !git.contains("://") {
        return Some(resolve(root, source));
    }
    let git = if let Some(rest) = git.strip_prefix("git@") {
        rest.replacen(':', "/", 1)
    } else {
        git.to_owned()
    };
    let git = if let Some(rest) = git.strip_prefix("github:") {
        format!("https://github.com/{rest}")
    } else if let Some(rest) = git.strip_prefix("gitlab:") {
        format!("https://gitlab.com/{rest}")
    } else if let Some(rest) = git.strip_prefix("bitbucket:") {
        format!("https://bitbucket.org/{rest}")
    } else if git.contains("://") {
        git
    } else if git
        .split('/')
        .next()
        .is_some_and(|s| s.contains('.') || s == "localhost")
    {
        format!("https://{git}")
    } else {
        format!("https://github.com/{git}")
    };
    let parsed = url::Url::parse(&git).ok()?;
    let host = parsed.host_str()?;
    let path = parsed
        .path()
        .trim_start_matches('/')
        .split('@')
        .next()?
        .trim_end_matches(".git");
    if path.split('/').count() < 2
        || path
            .split('/')
            .any(|part| matches!(part, ".." | "." | "") || part.contains(['%', '\\']))
    {
        return None;
    }
    Some(root.join("git").join(host).join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_globs_discover_both_extensions_without_loading_code() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("pkg/extensions")).unwrap();
        fs::write(
            root.join("pkg/package.json"),
            json!({"pi":{"extensions":["./extensions/*.{ts,js}"]}}).to_string(),
        )
        .unwrap();
        for name in ["a.ts", "b.js", "ignored.txt"] {
            fs::write(
                root.join("pkg/extensions").join(name),
                "throw new Error('do not execute');",
            )
            .unwrap();
        }
        fs::write(
            root.join("settings.json"),
            json!({"packages":["pkg"]}).to_string(),
        )
        .unwrap();
        let catalog = scan(root.into(), None).unwrap();
        assert_eq!(catalog.resources.len(), 2);
        assert!(
            catalog
                .resources
                .iter()
                .all(|r| r.kind == Kind::Extension && r.enabled && !r.editable)
        );
    }
    #[test]
    fn discovery_keeps_disabled_external_skills_and_respects_personal_rules() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("agent");
        let agents = temp.path().join(".agents");
        for dir in [
            root.join("skills/ignored"),
            root.join("prompts/nested"),
            agents.join("skills/group"),
            root.join("extensions/tool"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        let external = temp.path().join("external.md");
        fs::write(
            &external,
            "---\nname: ext\ndescription: >\n  multiple\n  lines\n---\nbody",
        )
        .unwrap();
        fs::write(root.join("skills/ignored/SKILL.md"), "ignored").unwrap();
        fs::write(root.join("skills/.ignore"), "ignored/\n").unwrap();
        fs::write(root.join("prompts/direct.md"), "direct").unwrap();
        fs::write(root.join("prompts/nested/hidden.md"), "hidden").unwrap();
        fs::write(
            agents.join("skills/root.md"),
            "not discovered in agents mode",
        )
        .unwrap();
        fs::write(
            agents.join("skills/group/nested.md"),
            "discovered in agents mode",
        )
        .unwrap();
        fs::write(
            root.join("extensions/tool/index.ts"),
            "export default () => {};",
        )
        .unwrap();
        fs::write(
            root.join("extensions/tool/helper.ts"),
            "not another extension",
        )
        .unwrap();
        fs::write(
            root.join("settings.json"),
            json!({"skills":[external.to_string_lossy(),format!("-{}",external.display())]})
                .to_string(),
        )
        .unwrap();
        let catalog = scan(root, Some(agents)).unwrap();
        let resource = catalog.resources.iter().find(|r| r.name == "ext").unwrap();
        assert!(!resource.enabled);
        assert!(!resource.editable);
        assert_eq!(resource.description, "multiple lines");
        assert!(
            !catalog
                .resources
                .iter()
                .any(|r| r.path.to_string_lossy().contains("ignored")
                    || r.name == "root"
                    || r.name == "hidden"
                    || r.name == "helper")
        );
        assert!(catalog.resources.iter().any(|r| r.name == "nested"));
        assert_eq!(
            catalog
                .resources
                .iter()
                .filter(|r| r.kind == Kind::Extension)
                .count(),
            1
        );
    }
    #[test]
    fn git_sources_resolve_using_host_and_repository_without_ref() {
        let root = Path::new("/agent");
        for source in [
            "git:github.com/user/repo@main",
            "git:git@github.com:user/repo.git@main",
            "https://github.com/user/repo.git",
            "git:user/repo",
        ] {
            assert_eq!(
                package_path(root, source, &json!({})),
                Some(root.join("git/github.com/user/repo")),
                "{source}"
            );
        }
        assert_eq!(
            package_path(root, "./local", &json!({})),
            Some(root.join("./local"))
        );
    }
    #[test]
    fn toggling_disabled_package_preserves_siblings_and_unknown_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let pkg = root.join("pkg");
        fs::create_dir_all(pkg.join("skills/a")).unwrap();
        fs::create_dir_all(pkg.join("skills/b")).unwrap();
        fs::write(pkg.join("skills/a/SKILL.md"), "A").unwrap();
        fs::write(pkg.join("skills/b/SKILL.md"), "B").unwrap();
        fs::write(
            root.join("settings.json"),
            r#"{"unknown":42,"packages":[{"source":"pkg","skills":[],"extra":true}]}"#,
        )
        .unwrap();
        let catalog = scan(root.into(), None).unwrap();
        assert_eq!(catalog.resources.len(), 2);
        assert!(catalog.resources.iter().all(|r| !r.enabled));
        set_enabled(root, &catalog.resources[0], true).unwrap();
        let updated = scan(root.into(), None).unwrap();
        assert_eq!(updated.resources.iter().filter(|r| r.enabled).count(), 1);
        let value = read_json(&root.join("settings.json")).unwrap();
        assert_eq!(value["unknown"], 42);
        assert_eq!(value["packages"][0]["extra"], true);
    }
    #[test]
    fn exact_toggle_overrides_exclusion_and_preserves_other_skills() {
        let base = Path::new("/agent");
        let path = base.join("skills/a/SKILL.md");
        assert!(!enabled(&path, &["!skills/**".into()], base));
        assert!(enabled(
            &path,
            &["!skills/**".into(), "+skills/a".into()],
            base
        ));
        assert!(!enabled(
            &path,
            &["+skills/a".into(), "-skills/a/SKILL.md".into()],
            base
        ));
        assert!(!delta_enabled(&path, &[], base));
        assert!(delta_enabled(&path, &["+skills/a".into()], base));
    }
}
