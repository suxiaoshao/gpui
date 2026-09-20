use std::{collections::BTreeSet, env, fmt::Write, fs, path::PathBuf};

fn main() {
    let directory = PathBuf::from(
        env::var_os("DEP_GPUI_KIT_DEFAULT_ICONS_ICONS_DIR")
            .expect("gpui-kit-assets must provide its icons-dir metadata"),
    );
    println!("cargo:rerun-if-env-changed=DEP_GPUI_KIT_DEFAULT_ICONS_ICONS_DIR");
    println!("cargo:rerun-if-changed={}", directory.display());
    println!("cargo:rerun-if-changed=build.rs");

    let paths: BTreeSet<_> = fs::read_dir(&directory)
        .expect("read upstream icons")
        .map(|entry| entry.expect("read upstream icon entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "svg"))
        .collect();
    assert!(!paths.is_empty(), "upstream icon catalog must not be empty");
    let mut names = BTreeSet::new();
    let mut code = String::from("#[allow(non_upper_case_globals)]\nimpl SvgIcon {\n");
    for path in paths {
        let slug = path.file_stem().unwrap().to_str().expect("UTF-8 icon name");
        let name: String = slug
            .split(['-', '_', '.'])
            .map(|part| {
                let mut chars = part.chars();
                let first = chars.next().expect("nonempty icon name segment");
                format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
            })
            .collect();
        assert!(names.insert(name.clone()), "duplicate icon name: {name}");
        assert!(name.starts_with(|c: char| c.is_ascii_alphabetic()));
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
        writeln!(code, "    /// Lucide `{slug}`.").unwrap();
        writeln!(
            code,
            "    pub const {name}: Self = Self::new(include_bytes!({:?}));",
            path.to_str().expect("UTF-8 icon path"),
        )
        .unwrap();
    }
    code.push_str("}\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(output.join("icons.rs"), code).expect("write icon constants");
}
