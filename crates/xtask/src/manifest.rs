use std::fs;
use std::path::Path;

use crate::error::{Result, XtaskError};

pub fn get_main_binary_name(manifest_path: &Path) -> Result<String> {
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
    })?;
    let value: toml::Value = toml::from_str(&content).map_err(|err| {
        XtaskError::msg(format!(
            "failed to parse {}: {err}",
            manifest_path.display()
        ))
    })?;

    let bins = value
        .get("bin")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for bin in bins {
        if let Some(path) = bin.get("path").and_then(toml::Value::as_str)
            && path == "src/main.rs"
            && let Some(name) = bin.get("name").and_then(toml::Value::as_str)
        {
            return Ok(name.to_string());
        }
    }

    value
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            XtaskError::msg(format!(
                "failed to resolve binary name from {}",
                manifest_path.display()
            ))
        })
}
