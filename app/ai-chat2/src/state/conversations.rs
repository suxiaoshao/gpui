use std::path::PathBuf;

use ai_chat_agent::{AgentRunRequest, SkillActivationRequest};
use ai_chat_core::{
    AgentEngineKind, AgentRuntimeSnapshot, ContentPart, ConversationId, ConversationItemPayload,
    ConversationItemStatus, ConversationMetadata, ConversationSettingsSnapshot, ProjectId,
    ReasoningSelectionSnapshot, RunSettingsSnapshot, ToolApprovalPolicy, ToolNameStrategy,
    ToolPolicySnapshot, TranscriptRole,
};
use ai_chat_db::{
    ConversationItemRecord, ConversationTimelineRecords, ConversationWithUserItemRecord,
    NewConversation, NewConversationItem, NewConversationWithUserItem, ProjectRecord,
};
use gpui::App;

use crate::{
    database,
    errors::AiChat2Result,
    foundation::I18n,
    state::{projects, providers::ProviderModelChoice},
};

const DEFAULT_MAX_STEPS: u32 = 32;
const TITLE_MAX_CHARS: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CreateConversationRequest {
    pub(crate) project_id: Option<ProjectId>,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) title_seed: String,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SendConversationMessageRequest {
    pub(crate) conversation_id: ConversationId,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

pub(crate) struct CreatedConversation {
    pub(crate) record: ConversationWithUserItemRecord,
    pub(crate) run_request: AgentRunRequest,
}

pub(crate) struct SentConversationMessage {
    pub(crate) item: ConversationItemRecord,
    pub(crate) run_request: AgentRunRequest,
}

pub(crate) type ConversationLoadSnapshot = ConversationTimelineRecords;

pub(crate) fn create_conversation(
    request: CreateConversationRequest,
    cx: &mut App,
) -> AiChat2Result<CreatedConversation> {
    let project = project_for_new_conversation(request.project_id.as_ref(), cx)?;
    let repository = database::repository(cx);
    let provider = repository
        .get_provider(&request.provider_model.provider_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "provider {} is missing",
                request.provider_model.provider_id
            ))
        })?;
    let tool_policy = default_tool_policy();
    let settings_snapshot =
        conversation_settings_snapshot(&request.provider_model, tool_policy.clone());
    let conversation = NewConversation {
        project_id: project.id.clone(),
        title: conversation_title(&request.title_seed, cx.global::<I18n>()),
        pinned: false,
        prompt_id: None,
        default_provider_id: Some(request.provider_model.provider_id.clone()),
        default_model_id: Some(request.provider_model.model_id.clone()),
        metadata: empty_conversation_metadata(),
        settings_snapshot,
    };
    let user_item = new_user_message_item(String::new(), request.content_parts.clone());
    let record = repository.insert_conversation_with_user_item(NewConversationWithUserItem {
        conversation,
        user_item,
    })?;
    update_last_active_conversation(&project, &record.conversation.id, cx)?;
    let run_request = build_run_request(RunRequestContext {
        conversation_id: &record.conversation.id,
        user_item_id: &record.user_item.id,
        project: &project,
        provider_settings: &provider.settings,
        provider_model: request.provider_model,
        reasoning_selection: request.reasoning_selection,
        skill_requests: request.skill_requests,
        tool_policy,
    });

    Ok(CreatedConversation {
        record,
        run_request,
    })
}

pub(crate) fn send_conversation_message(
    request: SendConversationMessageRequest,
    cx: &mut App,
) -> AiChat2Result<SentConversationMessage> {
    let repository = database::repository(cx);
    let conversation = repository
        .get_conversation(&request.conversation_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "conversation {} is missing",
                request.conversation_id
            ))
        })?;
    let project = repository
        .get_project(&conversation.project_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "project {} is missing",
                conversation.project_id
            ))
        })?;
    let provider = repository
        .get_provider(&request.provider_model.provider_id)?
        .ok_or_else(|| {
            ai_chat_db::DbError::Invariant(format!(
                "provider {} is missing",
                request.provider_model.provider_id
            ))
        })?;
    let item = repository.append_conversation_item(new_user_message_item(
        conversation.id.clone(),
        request.content_parts,
    ))?;
    update_last_active_conversation(&project, &conversation.id, cx)?;
    let run_request = build_run_request(RunRequestContext {
        conversation_id: &conversation.id,
        user_item_id: &item.id,
        project: &project,
        provider_settings: &provider.settings,
        provider_model: request.provider_model,
        reasoning_selection: request.reasoning_selection,
        skill_requests: request.skill_requests,
        tool_policy: default_tool_policy(),
    });

    Ok(SentConversationMessage { item, run_request })
}

pub(crate) fn load_conversation(
    conversation_id: &ConversationId,
    cx: &App,
) -> ai_chat_db::Result<Option<ConversationLoadSnapshot>> {
    database::repository(cx).conversation_timeline_records(conversation_id)
}

fn project_for_new_conversation(
    project_id: Option<&ProjectId>,
    cx: &mut App,
) -> AiChat2Result<ProjectRecord> {
    if let Some(project_id) = project_id {
        return database::repository(cx)
            .get_project(project_id)?
            .ok_or_else(|| {
                ai_chat_db::DbError::Invariant(format!("project {project_id} is missing")).into()
            });
    }

    projects::create_anonymous_scratch_project(cx)
}

fn new_user_message_item(
    conversation_id: ConversationId,
    content: Vec<ContentPart>,
) -> NewConversationItem {
    NewConversationItem {
        conversation_id,
        status: ConversationItemStatus::Completed,
        agent_run_id: None,
        provider_step_id: None,
        tool_invocation_id: None,
        provider_item_id: None,
        payload: ConversationItemPayload::Message {
            role: TranscriptRole::User,
            content,
        },
    }
}

struct RunRequestContext<'a> {
    conversation_id: &'a ConversationId,
    user_item_id: &'a str,
    project: &'a ProjectRecord,
    provider_settings: &'a ai_chat_core::ProviderSettingsPayload,
    provider_model: ProviderModelChoice,
    reasoning_selection: Option<ReasoningSelectionSnapshot>,
    skill_requests: Vec<SkillActivationRequest>,
    tool_policy: ToolPolicySnapshot,
}

fn build_run_request(input: RunRequestContext<'_>) -> AgentRunRequest {
    let mut request = AgentRunRequest::new(
        input.conversation_id.clone(),
        input.user_item_id.to_string(),
        input.provider_model.provider_id.clone(),
        input.provider_model.model_id.clone(),
        RunSettingsSnapshot {
            prompt: None,
            provider_id: input.provider_model.provider_id.clone(),
            model_id: input.provider_model.model_id.clone(),
            model_capabilities: input.provider_model.capabilities.clone(),
            provider_settings: input.provider_settings.clone(),
            reasoning_selection: input.reasoning_selection,
            tool_policy: input.tool_policy,
        },
        AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            skill_catalog_hash: None,
            mcp_config_hash: None,
            tool_name_strategy: ToolNameStrategy::Namespaced,
        },
    );
    request.skill_requests = input.skill_requests;
    request.project_root = Some(PathBuf::from(&input.project.path));
    request
}

fn conversation_settings_snapshot(
    provider_model: &ProviderModelChoice,
    tool_policy: ToolPolicySnapshot,
) -> ConversationSettingsSnapshot {
    ConversationSettingsSnapshot {
        prompt: None,
        provider_id: Some(provider_model.provider_id.clone()),
        model_id: Some(provider_model.model_id.clone()),
        model_capabilities: Some(provider_model.capabilities.clone()),
        tool_policy,
    }
}

fn default_tool_policy() -> ToolPolicySnapshot {
    ToolPolicySnapshot {
        approval_policy: ToolApprovalPolicy::OnRequest,
        enabled_sources: Vec::new(),
        max_steps: DEFAULT_MAX_STEPS,
    }
}

fn empty_conversation_metadata() -> ConversationMetadata {
    ConversationMetadata {
        summary: None,
        tags: Vec::new(),
    }
}

fn update_last_active_conversation(
    project: &ProjectRecord,
    conversation_id: &ConversationId,
    cx: &App,
) -> ai_chat_db::Result<()> {
    let mut metadata = project.metadata.clone();
    metadata.last_active_conversation_id = Some(conversation_id.clone());
    database::repository(cx).update_project_metadata(&project.id, metadata)?;
    Ok(())
}

fn conversation_title(seed: &str, i18n: &I18n) -> String {
    let title = seed.lines().next().unwrap_or_default().trim();
    if title.is_empty() {
        return i18n.t("conversation-default-title");
    }
    let mut truncated = title.chars().take(TITLE_MAX_CHARS).collect::<String>();
    if title.chars().count() > TITLE_MAX_CHARS {
        truncated.push_str("...");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::conversation_title;
    use crate::foundation::I18n;

    #[test]
    fn conversation_title_uses_first_non_empty_line() {
        let i18n = I18n::english_for_test();

        assert_eq!(conversation_title("hello\nsecond", &i18n), "hello");
    }

    #[test]
    fn conversation_title_falls_back_for_empty_seed() {
        let i18n = I18n::english_for_test();

        assert_eq!(conversation_title("  ", &i18n), "New conversation");
    }
}
