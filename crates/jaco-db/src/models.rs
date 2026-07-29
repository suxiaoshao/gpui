use crate::{Result, error::DbError, records::*, schema::*};
use diesel::prelude::*;
use jaco_core::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use time::OffsetDateTime;

#[path = "models/agent.rs"]
mod agent_models;
#[path = "models/conversations.rs"]
mod conversations_models;
#[path = "models/projects.rs"]
mod projects_models;
#[path = "models/prompts.rs"]
mod prompts_models;
#[path = "models/providers.rs"]
mod providers_models;
#[path = "models/schema.rs"]
mod schema_models;
#[path = "models/shortcuts.rs"]
mod shortcuts_models;

pub(crate) use agent_models::*;
pub(crate) use conversations_models::*;
pub(crate) use projects_models::*;
pub(crate) use prompts_models::*;
pub(crate) use providers_models::*;
pub(crate) use schema_models::*;
pub(crate) use shortcuts_models::*;

pub(crate) fn db_label<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        Value::String(value) => Ok(value),
        _ => Err(DbError::Invariant(
            "database enum labels must serialize to strings".to_string(),
        )),
    }
}

pub(crate) fn db_label_parse<T: DeserializeOwned>(value: String) -> Result<T> {
    Ok(serde_json::from_value(Value::String(value))?)
}

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub(crate) fn to_json_opt<T: Serialize>(value: &Option<T>) -> Result<Option<Value>> {
    value.as_ref().map(to_json).transpose()
}

pub(crate) fn from_json<T: DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

pub(crate) fn from_json_opt<T: DeserializeOwned>(value: Option<Value>) -> Result<Option<T>> {
    value.map(from_json).transpose()
}

pub(crate) fn tool_source_label(source: &ToolSource) -> String {
    match source {
        ToolSource::Local => "local".to_string(),
        ToolSource::Mcp { .. } => "mcp".to_string(),
        ToolSource::ProviderHosted { .. } => "provider_hosted".to_string(),
    }
}

pub(crate) fn tool_source_server_id(source: &ToolSource) -> Option<String> {
    match source {
        ToolSource::Mcp { server_id } => Some(server_id.clone()),
        ToolSource::Local | ToolSource::ProviderHosted { .. } => None,
    }
}

pub(crate) fn u64_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| DbError::Invariant("usage token count exceeds i64".to_string()))
}
