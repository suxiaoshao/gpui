use std::path::PathBuf;

use gpui::App;
use jaco_agent::{AgentRunRequest, SkillActivationRequest};
use jaco_core::{
    AgentEngineKind, AgentRunTriggerKind, AgentRuntimeSnapshot, ContentPart, ConversationId,
    ConversationItemPayload, ConversationItemStatus, ConversationMetadata,
    ConversationSettingsSnapshot, ProjectId, PromptContent, PromptId, ReasoningSelectionSnapshot,
    RunSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy, ToolNameStrategy,
    ToolPermissionScopeSnapshot, ToolPolicySnapshot, ToolSource, TranscriptRole, new_id,
};
use jaco_db::{
    ConversationItemRecord, ConversationRecord, ConversationTimelineRecords,
    ConversationWithUserItemRecord, FreshRepository, NewConversation, NewConversationItem,
    NewConversationWithUserItem, ProjectRecord,
};

use crate::{
    database,
    errors::JacoResult,
    foundation::I18n,
    state::{
        attachments::{
            ComposerAttachment, cleanup_stored_attachment_files, prepare_message_attachments,
        },
        projects,
        providers::ProviderModelChoice,
    },
};

const DEFAULT_MAX_STEPS: u32 = 32;
const TITLE_MAX_CHARS: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CreateConversationRequest {
    pub(crate) project_id: Option<ProjectId>,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) attachments: Vec<ComposerAttachment>,
    pub(crate) title_seed: String,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
    pub(crate) prompt_id: Option<PromptId>,
    pub(crate) prompt_snapshot: Option<PromptContent>,
    pub(crate) trigger_kind: AgentRunTriggerKind,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SendConversationMessageRequest {
    pub(crate) conversation_id: ConversationId,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) attachments: Vec<ComposerAttachment>,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
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
) -> JacoResult<CreatedConversation> {
    let repository = database::repository(cx);
    let provider = repository
        .get_provider(&request.provider_model.provider_id)?
        .ok_or_else(|| {
            jaco_db::DbError::Invariant(format!(
                "provider {} is missing",
                request.provider_model.provider_id
            ))
        })?;
    let mut tool_policy = default_tool_policy();
    tool_policy.approval_mode = request.approval_mode;
    let settings_snapshot = conversation_settings_snapshot(
        &request.provider_model,
        request.prompt_snapshot.clone(),
        tool_policy.clone(),
    );
    let conversation_id = new_id();
    let prepared_attachments =
        prepare_message_attachments(&conversation_id, &new_id(), &request.attachments, cx)?;
    let attachment_cleanup_paths = prepared_attachments.stored_paths.clone();
    let project = match project_for_new_conversation(request.project_id.as_ref(), cx) {
        Ok(project) => project,
        Err(err) => {
            cleanup_stored_attachment_files(&attachment_cleanup_paths);
            return Err(err);
        }
    };
    let conversation = NewConversation {
        project_id: project.id.clone(),
        title: conversation_title(&request.title_seed, cx.global::<I18n>()),
        pinned: false,
        prompt_id: request.prompt_id.clone(),
        default_provider_id: Some(request.provider_model.provider_id.clone()),
        default_model_id: Some(request.provider_model.model_id.clone()),
        metadata: empty_conversation_metadata(),
        settings_snapshot,
    };
    let user_item = new_user_message_item(conversation_id.clone(), request.content_parts.clone());
    let record = match repository.insert_conversation_with_user_item_with_id_and_attachments(
        conversation_id,
        NewConversationWithUserItem {
            conversation,
            user_item,
        },
        prepared_attachments.new_attachments,
    ) {
        Ok(record) => record,
        Err(err) => {
            cleanup_stored_attachment_files(&attachment_cleanup_paths);
            return Err(err.into());
        }
    };
    update_last_active_conversation(&project, &record.conversation.id, cx)?;
    let run_request = build_run_request(RunRequestContext {
        conversation_id: &record.conversation.id,
        user_item_id: &record.user_item.id,
        project: &project,
        provider_settings: &provider.settings,
        provider_model: request.provider_model,
        reasoning_selection: request.reasoning_selection,
        skill_requests: request.skill_requests,
        prompt_snapshot: request.prompt_snapshot,
        trigger_kind: request.trigger_kind,
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
) -> JacoResult<SentConversationMessage> {
    let repository = database::repository(cx);
    let conversation = repository
        .get_conversation(&request.conversation_id)?
        .ok_or_else(|| {
            jaco_db::DbError::Invariant(format!(
                "conversation {} is missing",
                request.conversation_id
            ))
        })?;
    let project = repository
        .get_project(&conversation.project_id)?
        .ok_or_else(|| {
            jaco_db::DbError::Invariant(format!("project {} is missing", conversation.project_id))
        })?;
    let provider = repository
        .get_provider(&request.provider_model.provider_id)?
        .ok_or_else(|| {
            jaco_db::DbError::Invariant(format!(
                "provider {} is missing",
                request.provider_model.provider_id
            ))
        })?;
    let prompt_snapshot = follow_up_prompt_snapshot(&conversation, &repository)?;
    let prepared_attachments =
        prepare_message_attachments(&conversation.id, &new_id(), &request.attachments, cx)?;
    let attachment_cleanup_paths = prepared_attachments.stored_paths.clone();
    let item = match repository.append_conversation_item_with_attachments(
        new_user_message_item(conversation.id.clone(), request.content_parts.clone()),
        prepared_attachments.new_attachments,
    ) {
        Ok(item) => item,
        Err(err) => {
            cleanup_stored_attachment_files(&attachment_cleanup_paths);
            return Err(err.into());
        }
    };
    update_last_active_conversation(&project, &conversation.id, cx)?;
    let run_request = build_run_request(RunRequestContext {
        conversation_id: &conversation.id,
        user_item_id: &item.id,
        project: &project,
        provider_settings: &provider.settings,
        provider_model: request.provider_model,
        reasoning_selection: request.reasoning_selection,
        skill_requests: request.skill_requests,
        prompt_snapshot,
        trigger_kind: AgentRunTriggerKind::User,
        tool_policy: {
            let mut tool_policy = default_tool_policy();
            tool_policy.approval_mode = request.approval_mode;
            tool_policy
        },
    });

    Ok(SentConversationMessage { item, run_request })
}

fn follow_up_prompt_snapshot(
    conversation: &ConversationRecord,
    repository: &FreshRepository,
) -> jaco_db::Result<Option<PromptContent>> {
    if let Some(prompt) = conversation.settings_snapshot.prompt.clone() {
        return Ok(Some(prompt));
    }
    let Some(prompt_id) = conversation.prompt_id.as_ref() else {
        return Ok(None);
    };
    Ok(repository
        .get_prompt(prompt_id)?
        .filter(|prompt| prompt.enabled)
        .map(|prompt| prompt.content))
}

pub(crate) fn load_conversation(
    conversation_id: &ConversationId,
    cx: &App,
) -> jaco_db::Result<Option<ConversationLoadSnapshot>> {
    database::repository(cx).conversation_timeline_records(conversation_id)
}

fn project_for_new_conversation(
    project_id: Option<&ProjectId>,
    cx: &mut App,
) -> JacoResult<ProjectRecord> {
    if let Some(project_id) = project_id {
        return database::repository(cx)
            .get_project(project_id)?
            .ok_or_else(|| {
                jaco_db::DbError::Invariant(format!("project {project_id} is missing")).into()
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
    provider_settings: &'a jaco_core::ProviderSettingsPayload,
    provider_model: ProviderModelChoice,
    reasoning_selection: Option<ReasoningSelectionSnapshot>,
    skill_requests: Vec<SkillActivationRequest>,
    prompt_snapshot: Option<PromptContent>,
    trigger_kind: AgentRunTriggerKind,
    tool_policy: ToolPolicySnapshot,
}

fn build_run_request(input: RunRequestContext<'_>) -> AgentRunRequest {
    let mut tool_policy = input.tool_policy;
    tool_policy.permission_scope = Some(ToolPermissionScopeSnapshot {
        project_roots: vec![input.project.path.clone()],
        external_read_requires_approval: false,
        external_write_requires_approval: true,
    });
    let mut request = AgentRunRequest::new(
        input.conversation_id.clone(),
        input.user_item_id.to_string(),
        input.provider_model.provider_id.clone(),
        input.provider_model.model_id.clone(),
        RunSettingsSnapshot {
            prompt: input.prompt_snapshot.clone(),
            provider_id: input.provider_model.provider_id.clone(),
            model_id: input.provider_model.model_id.clone(),
            model_capabilities: input.provider_model.capabilities.clone(),
            provider_settings: input.provider_settings.clone(),
            reasoning_selection: input.reasoning_selection,
            tool_policy,
        },
        AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            skill_catalog_hash: None,
            tool_name_strategy: ToolNameStrategy::Namespaced,
        },
    );
    request.trigger_kind = input.trigger_kind;
    request.prompt_snapshot = input.prompt_snapshot;
    request.skill_requests = input.skill_requests;
    request.project_root = Some(PathBuf::from(&input.project.path));
    request
}

fn conversation_settings_snapshot(
    provider_model: &ProviderModelChoice,
    prompt: Option<PromptContent>,
    tool_policy: ToolPolicySnapshot,
) -> ConversationSettingsSnapshot {
    ConversationSettingsSnapshot {
        prompt,
        provider_id: Some(provider_model.provider_id.clone()),
        model_id: Some(provider_model.model_id.clone()),
        model_capabilities: Some(provider_model.capabilities.clone()),
        tool_policy,
    }
}

pub(crate) fn default_tool_policy() -> ToolPolicySnapshot {
    ToolPolicySnapshot {
        approval_policy: ToolApprovalPolicy::OnRequest,
        enabled_sources: vec![ToolSource::Local],
        max_steps: DEFAULT_MAX_STEPS,
        approval_mode: ToolApprovalMode::RequestApproval,
        permission_scope: None,
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
) -> jaco_db::Result<()> {
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
    use super::*;
    use crate::{
        database::{self, FreshStoreGlobal},
        foundation::I18n,
        state::{
            JacoConfig,
            attachments::{ComposerAttachmentKind, ComposerAttachmentSource},
        },
    };
    use gpui::TestAppContext;
    use jaco_core::{
        ModelCapabilitiesSnapshot, ProjectKind, ProjectMetadata, ProviderSecretRefs,
        ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
        conservative_model_capabilities,
    };
    use jaco_db::{NewConversation, NewProject, NewPrompt, NewProvider, ProjectRecord};
    use tempfile::{TempDir, tempdir};

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

    #[gpui::test]
    fn send_message_does_not_persist_user_item_when_attachment_copy_fails(cx: &mut TestAppContext) {
        let dir = init_conversations_test(cx);
        let (conversation_id, provider_model, initial_item_count) = cx.update(|cx| {
            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            let provider_model = provider_model_choice(&provider.id);
            let conversation_id = insert_conversation(&repository, &provider_model);
            let initial_item_count = repository
                .conversation_items(&conversation_id)
                .unwrap()
                .len();
            (conversation_id, provider_model, initial_item_count)
        });
        let missing_path = dir.path().join("missing-attachment.txt");

        let result = cx.update(|cx| {
            send_conversation_message(
                SendConversationMessageRequest {
                    conversation_id: conversation_id.clone(),
                    content_parts: vec![ContentPart::Text {
                        text: "send with missing attachment".to_string(),
                    }],
                    attachments: vec![ComposerAttachment {
                        local_id: 1,
                        kind: ComposerAttachmentKind::File,
                        source: ComposerAttachmentSource::LocalFile { path: missing_path },
                        name: "missing-attachment.txt".to_string(),
                        mime_type: Some("text/plain".to_string()),
                        size_bytes: Some(12),
                        width: None,
                        height: None,
                    }],
                    skill_requests: Vec::new(),
                    provider_model,
                    reasoning_selection: None,
                    approval_mode: ToolApprovalMode::RequestApproval,
                },
                cx,
            )
        });

        assert!(result.is_err());
        cx.update(|cx| {
            let repository = database::repository(cx);
            assert_eq!(
                repository
                    .conversation_items(&conversation_id)
                    .unwrap()
                    .len(),
                initial_item_count
            );
            assert!(
                repository
                    .conversation_attachments(&conversation_id)
                    .unwrap()
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn create_conversation_does_not_persist_conversation_when_attachment_copy_fails(
        cx: &mut TestAppContext,
    ) {
        let dir = init_conversations_test(cx);
        let provider_model = cx.update(|cx| {
            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            provider_model_choice(&provider.id)
        });
        let missing_path = dir.path().join("missing-new-conversation.txt");

        let result = cx.update(|cx| {
            create_conversation(
                CreateConversationRequest {
                    project_id: None,
                    content_parts: vec![ContentPart::Text {
                        text: "new conversation with missing attachment".to_string(),
                    }],
                    attachments: vec![ComposerAttachment {
                        local_id: 1,
                        kind: ComposerAttachmentKind::File,
                        source: ComposerAttachmentSource::LocalFile { path: missing_path },
                        name: "missing-new-conversation.txt".to_string(),
                        mime_type: Some("text/plain".to_string()),
                        size_bytes: Some(12),
                        width: None,
                        height: None,
                    }],
                    title_seed: "new conversation with missing attachment".to_string(),
                    skill_requests: Vec::new(),
                    provider_model,
                    reasoning_selection: None,
                    approval_mode: ToolApprovalMode::RequestApproval,
                    prompt_id: None,
                    prompt_snapshot: None,
                    trigger_kind: AgentRunTriggerKind::User,
                },
                cx,
            )
        });

        assert!(result.is_err());
        cx.update(|cx| {
            assert!(
                database::repository(cx)
                    .list_sidebar_conversations()
                    .unwrap()
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn send_message_reuses_conversation_prompt_snapshot(cx: &mut TestAppContext) {
        let _dir = init_conversations_test(cx);
        let (conversation_id, provider_model, expected_prompt) = cx.update(|cx| {
            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            let provider_model = provider_model_choice(&provider.id);
            let prompt = repository
                .insert_prompt(NewPrompt {
                    name: "Shortcut Prompt".to_string(),
                    content: PromptContent {
                        text: "current prompt text".to_string(),
                    },
                    enabled: true,
                    sort_order: 10,
                })
                .unwrap();
            let expected_prompt = PromptContent {
                text: "snapshot prompt text".to_string(),
            };
            let conversation_id = insert_conversation_with_prompt(
                &repository,
                &provider_model,
                Some(prompt.id),
                Some(expected_prompt.clone()),
            );
            (conversation_id, provider_model, expected_prompt)
        });

        let sent = cx
            .update(|cx| {
                send_conversation_message(
                    SendConversationMessageRequest {
                        conversation_id,
                        content_parts: vec![ContentPart::Text {
                            text: "follow up".to_string(),
                        }],
                        attachments: Vec::new(),
                        skill_requests: Vec::new(),
                        provider_model,
                        reasoning_selection: None,
                        approval_mode: ToolApprovalMode::RequestApproval,
                    },
                    cx,
                )
            })
            .unwrap();

        assert_eq!(
            sent.run_request.prompt_snapshot,
            Some(expected_prompt.clone())
        );
        assert_eq!(
            sent.run_request.settings_snapshot.prompt,
            Some(expected_prompt)
        );
    }

    #[gpui::test]
    fn send_message_falls_back_to_prompt_id_when_snapshot_is_missing(cx: &mut TestAppContext) {
        let _dir = init_conversations_test(cx);
        let (conversation_id, provider_model, expected_prompt) = cx.update(|cx| {
            let repository = database::repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            let provider_model = provider_model_choice(&provider.id);
            let prompt = repository
                .insert_prompt(NewPrompt {
                    name: "Fallback Prompt".to_string(),
                    content: PromptContent {
                        text: "fallback prompt text".to_string(),
                    },
                    enabled: true,
                    sort_order: 10,
                })
                .unwrap();
            let expected_prompt = prompt.content.clone();
            let conversation_id = insert_conversation_with_prompt(
                &repository,
                &provider_model,
                Some(prompt.id),
                None,
            );
            (conversation_id, provider_model, expected_prompt)
        });

        let sent = cx
            .update(|cx| {
                send_conversation_message(
                    SendConversationMessageRequest {
                        conversation_id,
                        content_parts: vec![ContentPart::Text {
                            text: "follow up".to_string(),
                        }],
                        attachments: Vec::new(),
                        skill_requests: Vec::new(),
                        provider_model,
                        reasoning_selection: None,
                        approval_mode: ToolApprovalMode::RequestApproval,
                    },
                    cx,
                )
            })
            .unwrap();

        assert_eq!(
            sent.run_request.prompt_snapshot,
            Some(expected_prompt.clone())
        );
        assert_eq!(
            sent.run_request.settings_snapshot.prompt,
            Some(expected_prompt)
        );
    }

    fn init_conversations_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            let mut config =
                JacoConfig::load_from_path_for_test(&dir.path().join("config.toml")).unwrap();
            config.storage.data_dir = Some(dir.path().join("data"));
            crate::state::config::install_for_test(cx, config).unwrap();
            crate::foundation::i18n::init(cx);
        });
        dir
    }

    fn insert_conversation(
        repository: &jaco_db::FreshRepository,
        provider_model: &ProviderModelChoice,
    ) -> ConversationId {
        insert_conversation_with_prompt(repository, provider_model, None, None)
    }

    fn insert_conversation_with_prompt(
        repository: &jaco_db::FreshRepository,
        provider_model: &ProviderModelChoice,
        prompt_id: Option<PromptId>,
        prompt_snapshot: Option<PromptContent>,
    ) -> ConversationId {
        let project = insert_project(repository);
        repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Conversation Test".to_string(),
                pinned: false,
                prompt_id,
                default_provider_id: Some(provider_model.provider_id.clone()),
                default_model_id: Some(provider_model.model_id.clone()),
                metadata: empty_conversation_metadata(),
                settings_snapshot: conversation_settings_snapshot(
                    provider_model,
                    prompt_snapshot,
                    default_tool_policy(),
                ),
            })
            .unwrap()
            .id
    }

    fn insert_project(repository: &jaco_db::FreshRepository) -> ProjectRecord {
        repository
            .insert_project(NewProject {
                path: format!("/tmp/jaco-conversation-test-{}", new_id()),
                display_name: "Conversation Test".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
            })
            .unwrap()
    }

    fn provider_for_test() -> NewProvider {
        NewProvider {
            kind: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: vec![ProviderSettingFieldValue {
                    key: "base_url".to_string(),
                    value: ProviderSettingValue::String {
                        value: "https://api.openai.com/v1".to_string(),
                    },
                }],
            },
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
        }
    }

    fn provider_model_choice(provider_id: &str) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider_id.to_string(),
            provider_kind: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
            model_id: "gpt-5".to_string(),
            model_display_name: None,
            capabilities: model_capabilities(),
        }
    }

    fn model_capabilities() -> ModelCapabilitiesSnapshot {
        let mut capabilities = conservative_model_capabilities("openai");
        capabilities.file_input =
            Some(jaco_core::FileInputCapabilitySnapshot { max_files: Some(4) });
        capabilities
    }
}
