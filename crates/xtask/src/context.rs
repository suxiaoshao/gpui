use std::path::{Path, PathBuf};

use crate::error::{Result, XtaskError};

pub fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| XtaskError::msg("failed to resolve workspace root"))
}

pub fn ai_chat_dir() -> Result<PathBuf> {
    Ok(workspace_root()?.join("app/ai-chat"))
}
