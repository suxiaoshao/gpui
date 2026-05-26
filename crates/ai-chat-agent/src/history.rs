use crate::{AgentRuntimeError, Result};
use ai_chat_core::*;
use ai_chat_db::ConversationItemRecord;
use rig_core::{
    OneOrMany,
    completion::{AssistantContent, Message as RigMessage},
    message::{ToolCall, ToolFunction, ToolResult, ToolResultContent, UserContent},
};

pub(crate) struct PromptHistory {
    pub(crate) prompt: RigMessage,
    pub(crate) history: Vec<RigMessage>,
    pub(crate) input_item_ids: Vec<ConversationItemId>,
}

pub(crate) fn build_prompt_history(
    items: &[ConversationItemRecord],
    user_item_id: &str,
) -> Result<PromptHistory> {
    let user_index = items
        .iter()
        .position(|item| item.id == user_item_id)
        .ok_or_else(|| {
            AgentRuntimeError::Invariant(format!("user item {user_item_id} is missing"))
        })?;
    let prompt = conversation_item_to_rig_message(&items[user_index])?.ok_or_else(|| {
        AgentRuntimeError::Invariant(format!("user item {user_item_id} cannot be used as prompt"))
    })?;
    let history = items[..user_index]
        .iter()
        .filter_map(|item| conversation_item_to_rig_message(item).transpose())
        .collect::<Result<Vec<_>>>()?;
    let input_item_ids = items[..=user_index]
        .iter()
        .map(|item| item.id.clone())
        .collect();
    Ok(PromptHistory {
        prompt,
        history,
        input_item_ids,
    })
}

pub(crate) fn conversation_item_to_rig_message(
    item: &ConversationItemRecord,
) -> Result<Option<RigMessage>> {
    Ok(match &item.payload {
        ConversationItemPayload::Message { role, content } => {
            let text = content_text(content);
            match role {
                TranscriptRole::System => Some(RigMessage::system(text)),
                TranscriptRole::Developer => Some(RigMessage::system(format!(
                    "Developer instruction:\n{text}"
                ))),
                TranscriptRole::User => Some(RigMessage::user(text)),
                TranscriptRole::Assistant => Some(RigMessage::assistant(text)),
                TranscriptRole::Tool => Some(RigMessage::user(text)),
            }
        }
        ConversationItemPayload::SkillActivation(skill) => Some(RigMessage::system(format!(
            "Loaded skill `{}` from {}:\n{}",
            skill.name,
            skill.skill_file_path,
            content_text(&skill.content)
        ))),
        ConversationItemPayload::Reasoning { text, summary } => {
            let reasoning = summary.as_ref().map_or_else(
                || rig_core::message::Reasoning::new(text),
                |summary| rig_core::message::Reasoning::summaries(vec![summary.clone()]),
            );
            Some(RigMessage::Assistant {
                id: item.provider_item_id.clone(),
                content: OneOrMany::one(AssistantContent::Reasoning(reasoning)),
            })
        }
        ConversationItemPayload::ToolCall(call) => Some(RigMessage::Assistant {
            id: item.provider_item_id.clone(),
            content: OneOrMany::one(AssistantContent::ToolCall(ToolCall::new(
                call.call_id.clone(),
                ToolFunction::new(call.runtime_tool_name.clone(), call.arguments.value.clone()),
            ))),
        }),
        ConversationItemPayload::ToolResult(result) => Some(RigMessage::User {
            content: OneOrMany::one(UserContent::ToolResult(ToolResult {
                id: result.call_id.clone(),
                call_id: None,
                content: OneOrMany::one(ToolResultContent::text(content_text(&result.content))),
            })),
        }),
        ConversationItemPayload::Error(error) => Some(RigMessage::system(format!(
            "Previous run error [{}]: {}",
            error.code, error.message
        ))),
        ConversationItemPayload::ApprovalDecision(item) => Some(RigMessage::system(format!(
            "Tool approval decision: approved={} reason={}",
            item.decision.approved,
            item.decision.reason.clone().unwrap_or_default()
        ))),
        ConversationItemPayload::ApprovalRequest(_) | ConversationItemPayload::Status(_) => None,
    })
}

pub(crate) fn content_text(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}
