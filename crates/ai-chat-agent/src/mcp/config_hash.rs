use ai_chat_core::McpRuntimeConfigSnapshot;
use sha2::{Digest, Sha256};

use crate::Result;

pub fn mcp_config_hash(snapshot: &McpRuntimeConfigSnapshot) -> Result<String> {
    let bytes = serde_json::to_vec(snapshot)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}
