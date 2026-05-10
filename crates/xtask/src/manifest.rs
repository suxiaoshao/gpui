use std::fs;
use std::path::Path;

use crate::error::{Result, XtaskError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    package: ManifestPackage,
    #[serde(default)]
    bin: Vec<ManifestBin>,
}

#[derive(Deserialize)]
struct ManifestPackage {
    name: String,
}

#[derive(Deserialize)]
struct ManifestBin {
    name: Option<String>,
    path: Option<String>,
}

pub fn get_main_binary_name(manifest_path: &Path) -> Result<String> {
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let manifest: Manifest = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse {}: {err}",
            manifest_path.display()
        ))
    })?;

    for bin in manifest.bin {
        if bin.path.as_deref() == Some("src/main.rs")
            && let Some(name) = bin.name
        {
            return Ok(name);
        }
    }

    Ok(manifest.package.name)
}

#[cfg(test)]
mod tests {
    use super::get_main_binary_name;
    use crate::error::Result;
    use std::fs;
    use std::path::PathBuf;
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
                "xtask-manifest-{suffix}-{}-{id}",
                std::process::id(),
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

    #[test]
    fn main_binary_name_falls_back_to_package_name() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = temp_dir.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "http-client"
version = "0.1.0"
"#,
        )?;

        assert_eq!(get_main_binary_name(&manifest_path)?, "http-client");
        Ok(())
    }

    #[test]
    fn main_binary_name_prefers_main_bin() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let manifest_path = temp_dir.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "workspace-package"
version = "0.1.0"

[[bin]]
name = "helper"
path = "src/bin/helper.rs"

[[bin]]
name = "main-app"
path = "src/main.rs"
"#,
        )?;

        assert_eq!(get_main_binary_name(&manifest_path)?, "main-app");
        Ok(())
    }
}
