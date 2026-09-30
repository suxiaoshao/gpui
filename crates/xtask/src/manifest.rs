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
