use std::{fmt, path::PathBuf};

use gpui::{App, Task};
use gpui_operation::refresh;
use jaco_agent::{AgentRunRequest, SkillActivationRequest};
use jaco_core::{
    AgentEngineKind, AgentRunTriggerKind, AgentRuntimeSnapshot, ContentPart,
    ConversationEntryPayload, ConversationEntryStatus, ConversationId, ConversationMetadata,
    ConversationSettingsSnapshot, ProjectId, PromptContent, PromptId, ReasoningSelectionSnapshot,
    RunSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy, ToolNameStrategy,
    ToolPermissionScopeSnapshot, ToolPolicySnapshot, ToolSource, TranscriptRole, new_id,
};
use jaco_db::{
    ConversationEntryRecord, ConversationIndexDelta, ConversationRecord,
    ConversationTimelineRecords, ConversationWithUserItemRecord, CreatedConversationTransaction,
    FreshRepository, NewConversation, NewConversationEntry, NewConversationTransaction,
    NewConversationWithUserItem, ProjectRecord, SendConversationTransaction,
};
use tokio::sync::oneshot;

use crate::{
    database,
    errors::JacoResult,
    foundation::I18n,
    state::{
        attachments::{
            ComposerAttachment, cleanup_stored_attachment_files, prepare_message_attachments_in,
        },
        config, conversation_index, projects,
        providers::ProviderModelChoice,
        session::CatalogMutation,
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
    pub(crate) item: ConversationEntryRecord,
    pub(crate) conversation: ConversationRecord,
    pub(crate) run_request: AgentRunRequest,
}

pub(crate) type ConversationLoadSnapshot = ConversationTimelineRecords;
pub(crate) type ConversationTimelineOperation =
    refresh::Operation<Option<ConversationLoadSnapshot>, ConversationTimelineProblem, Task<()>>;

#[derive(Debug)]
pub(crate) struct ConversationTimelineProblem(jaco_db::DbError);

impl fmt::Display for ConversationTimelineProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ConversationTimelineProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<jaco_db::DbError> for ConversationTimelineProblem {
    fn from(error: jaco_db::DbError) -> Self {
        Self(error)
    }
}

pub(crate) fn create_conversation(
    request: CreateConversationRequest,
    cx: &mut App,
) -> Task<JacoResult<CreatedConversation>> {
    if !conversation_index::is_ready(cx) {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation index is not ready".to_string(),
        )
        .into()));
    }
    let provider =
        match crate::state::providers::ready_provider(&request.provider_model.provider_id, cx) {
            Ok(provider) => provider,
            Err(error) => return Task::ready(Err(error.into())),
        };
    let (project_id, new_project, scratch_path) = match request.project_id.as_ref() {
        Some(project_id) => {
            let project = projects::catalog(cx).read(cx, |operation| match operation {
                projects::ProjectOperation::Ready(ready) => ready
                    .data()
                    .projects()
                    .iter()
                    .find(|project| &project.id == project_id)
                    .cloned()
                    .ok_or_else(|| {
                        jaco_db::DbError::Invariant(format!("project {project_id} is missing"))
                    }),
                _ => Err(jaco_db::DbError::Invariant(
                    "project resource is not ready".to_string(),
                )),
            });
            match project {
                Ok(project) => (project.id, None, None),
                Err(error) => return Task::ready(Err(error.into())),
            }
        }
        None => match projects::prepare_anonymous_scratch_project(cx) {
            Ok((id, path, project)) => (id.clone(), Some((id, project)), Some(path)),
            Err(error) => return Task::ready(Err(error)),
        },
    };
    let data_dir = match config::data_dir(cx) {
        Ok(path) => path,
        Err(error) => return Task::ready(Err(error)),
    };
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation create requires an exact Ready session".to_string(),
        )
        .into()));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error.into())),
    };
    let mut tool_policy = default_tool_policy();
    tool_policy.approval_mode = request.approval_mode;
    let settings_snapshot = conversation_settings_snapshot(
        &request.provider_model,
        request.prompt_snapshot.clone(),
        tool_policy.clone(),
    );
    let conversation_id = new_id();
    let entry_id = new_id();
    let conversation = NewConversation {
        project_id,
        title: conversation_title(&request.title_seed, cx.global::<I18n>()),
        pinned: false,
        prompt_id: request.prompt_id.clone(),
        default_provider_id: Some(request.provider_model.provider_id.clone()),
        default_model_id: Some(request.provider_model.model_id.clone()),
        metadata: empty_conversation_metadata(),
        settings_snapshot,
    };
    let user_item = new_user_message_item(conversation_id.clone(), request.content_parts.clone());
    let attachments = request.attachments.clone();
    let run_input = request;
    let (sender, receiver) = oneshot::channel();
    cx.spawn(async move |cx| {
        let result = executor
            .mutate_two(
                CatalogMutation::Project,
                CatalogMutation::Conversation,
                move |repository| {
                    if let Some(path) = scratch_path.as_ref() {
                        std::fs::create_dir_all(path)
                            .map_err(|error| jaco_db::DbError::Invariant(error.to_string()))?;
                    }
                    let prepared = match prepare_message_attachments_in(
                        data_dir,
                        &conversation_id,
                        &entry_id,
                        &attachments,
                    ) {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            if let Some(path) = scratch_path.as_ref() {
                                let _ = std::fs::remove_dir_all(path);
                            }
                            return Err(jaco_db::DbError::Invariant(error.to_string()));
                        }
                    };
                    let cleanup = prepared.stored_paths.clone();
                    let result =
                        repository.create_conversation_transaction(NewConversationTransaction {
                            new_project,
                            conversation_id,
                            conversation: NewConversationWithUserItem {
                                conversation,
                                user_item,
                            },
                            attachments: prepared.new_attachments,
                        });
                    if result.is_err() {
                        cleanup_stored_attachment_files(&cleanup);
                        if let Some(path) = scratch_path.as_ref() {
                            let _ = std::fs::remove_dir_all(path);
                        }
                    }
                    result
                },
            )
            .await
            .map(|transaction: CreatedConversationTransaction| {
                let run_request = build_run_request(RunRequestContext {
                    conversation_id: &transaction.record.conversation.id,
                    trigger_entry_id: &transaction.record.user_item.id,
                    project: &transaction.project,
                    provider_settings: &provider.settings,
                    provider_model: run_input.provider_model,
                    reasoning_selection: run_input.reasoning_selection,
                    skill_requests: run_input.skill_requests,
                    prompt_snapshot: run_input.prompt_snapshot,
                    trigger_kind: run_input.trigger_kind,
                    tool_policy,
                });
                (transaction, run_request)
            });
        if let Ok((transaction, _)) = &result {
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() == Some(&binding) {
                    projects::publish_project(transaction.project.clone(), cx);
                    conversation_index::publish(transaction.record.conversation.clone(), cx);
                }
            });
        }
        let result = result
            .map(|(transaction, run_request)| CreatedConversation {
                record: transaction.record,
                run_request,
            })
            .map_err(Into::into);
        let _ = sender.send(result);
    })
    .detach();
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(jaco_db::DbError::Invariant(
                "conversation create driver ended without a result".to_string(),
            )
            .into())
        })
    })
}

pub(crate) fn send_conversation_message(
    request: SendConversationMessageRequest,
    cx: &mut App,
) -> Task<JacoResult<SentConversationMessage>> {
    if !conversation_index::is_ready(cx) {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation index is not ready".to_string(),
        )
        .into()));
    }
    let provider =
        match crate::state::providers::ready_provider(&request.provider_model.provider_id, cx) {
            Ok(provider) => provider,
            Err(error) => return Task::ready(Err(error.into())),
        };
    let data_dir = match config::data_dir(cx) {
        Ok(path) => path,
        Err(error) => return Task::ready(Err(error)),
    };
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation send requires an exact Ready session".to_string(),
        )
        .into()));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error.into())),
    };
    let conversation_id = request.conversation_id.clone();
    let entry_id = new_id();
    let attachments = request.attachments.clone();
    let content_parts = request.content_parts.clone();
    let (sender, receiver) = oneshot::channel();
    cx.spawn(async move |cx| {
        let result = executor
            .mutate_two(
                CatalogMutation::Project,
                CatalogMutation::Conversation,
                move |repository| {
                    let conversation =
                        repository
                            .get_conversation(&conversation_id)?
                            .ok_or_else(|| {
                                jaco_db::DbError::Invariant(format!(
                                    "conversation {conversation_id} is missing"
                                ))
                            })?;
                    let prompt_snapshot = follow_up_prompt_snapshot(&conversation, repository)?;
                    let prepared = prepare_message_attachments_in(
                        data_dir,
                        &conversation_id,
                        &entry_id,
                        &attachments,
                    )
                    .map_err(|error| jaco_db::DbError::Invariant(error.to_string()))?;
                    let cleanup = prepared.stored_paths.clone();
                    let result = repository
                        .send_conversation_transaction(SendConversationTransaction {
                            entry: new_user_message_item(conversation_id, content_parts),
                            attachments: prepared.new_attachments,
                        })
                        .map(|transaction| (transaction, prompt_snapshot));
                    if result.is_err() {
                        cleanup_stored_attachment_files(&cleanup);
                    }
                    result
                },
            )
            .await
            .map(|(transaction, prompt_snapshot)| {
                let item = transaction.commit.value.clone();
                let run_request = build_run_request(RunRequestContext {
                    conversation_id: &transaction.commit.conversation.id,
                    trigger_entry_id: &item.id,
                    project: &transaction.project,
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
                let sent = SentConversationMessage {
                    item,
                    conversation: transaction.commit.conversation.clone(),
                    run_request,
                };
                (transaction, sent)
            });
        if let Ok((transaction, _)) = &result {
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() == Some(&binding) {
                    projects::publish_project(transaction.project.clone(), cx);
                    conversation_index::publish_committed(
                        transaction.commit.conversation.clone(),
                        transaction.commit.index_delta.clone(),
                        cx,
                    );
                }
            });
        }
        let _ = sender.send(
            result
                .map(|(_, sent)| sent)
                .map_err(crate::errors::JacoError::from),
        );
    })
    .detach();
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(jaco_db::DbError::Invariant(
                "conversation send driver ended without a result".to_string(),
            )
            .into())
        })
    })
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

pub(crate) fn set_conversation_pinned(
    conversation_id: ConversationId,
    pinned: bool,
    cx: &mut App,
) -> Task<jaco_db::Result<ConversationRecord>> {
    spawn_conversation_mutation(
        cx,
        move |repository| repository.set_conversation_pinned(&conversation_id, pinned),
        |conversation, cx| {
            conversation_index::publish_committed(
                conversation.clone(),
                ConversationIndexDelta::PresentationChanged {
                    id: conversation.id.clone(),
                    title: None,
                    pinned: Some(conversation.pinned),
                    status: None,
                    updated_at: conversation.updated_at,
                },
                cx,
            );
        },
    )
}

pub(crate) fn delete_conversation(
    conversation_id: ConversationId,
    cx: &mut App,
) -> Task<jaco_db::Result<ConversationRecord>> {
    let removed_id = conversation_id.clone();
    spawn_conversation_mutation(
        cx,
        move |repository| repository.soft_delete_conversation(&conversation_id),
        move |_conversation, cx| conversation_index::publish_removed(removed_id, cx),
    )
}

fn spawn_conversation_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    publish: impl FnOnce(&R, &mut App) + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static,
{
    if !conversation_index::is_ready(cx) {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation index is not ready".to_string(),
        )));
    }
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "conversation mutation requires an exact Ready session".to_string(),
        )));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    cx.spawn(async move |cx| {
        let result = executor
            .mutate(CatalogMutation::Conversation, command)
            .await;
        if let Ok(value) = &result {
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() == Some(&binding) {
                    publish(value, cx);
                }
            });
        }
        let _ = sender.send(result);
    })
    .detach();
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(jaco_db::DbError::Invariant(
                "conversation mutation driver ended without a result".to_string(),
            ))
        })
    })
}

fn new_user_message_item(
    conversation_id: ConversationId,
    content: Vec<ContentPart>,
) -> NewConversationEntry {
    NewConversationEntry {
        conversation_id,
        status: ConversationEntryStatus::Completed,
        agent_run_id: None,
        provider_step_id: None,
        tool_invocation_id: None,
        provider_item_id: None,
        payload: ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            content,
        },
    }
}

struct RunRequestContext<'a> {
    conversation_id: &'a ConversationId,
    trigger_entry_id: &'a str,
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
        input.trigger_entry_id.to_string(),
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
        database,
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
            let repository = test_repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            let provider_model = provider_model_choice(&provider.id);
            let conversation_id = insert_conversation(&repository, &provider_model);
            let initial_item_count = repository
                .conversation_entries(&conversation_id)
                .unwrap()
                .len();
            (conversation_id, provider_model, initial_item_count)
        });
        init_conversation_resources(cx);
        let missing_path = dir.path().join("missing-attachment.txt");

        let task = cx.update(|cx| {
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
        let result = cx.foreground_executor().block_test(task);

        assert!(result.is_err());
        cx.update(|cx| {
            let repository = test_repository(cx);
            assert_eq!(
                repository
                    .conversation_entries(&conversation_id)
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
            let repository = test_repository(cx);
            let provider = repository.insert_provider(provider_for_test()).unwrap();
            provider_model_choice(&provider.id)
        });
        init_conversation_resources(cx);
        let missing_path = dir.path().join("missing-new-conversation.txt");

        let task = cx.update(|cx| {
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
        let result = cx.foreground_executor().block_test(task);

        assert!(result.is_err());
        cx.update(|cx| {
            assert!(
                test_repository(cx)
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
            let repository = test_repository(cx);
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
        init_conversation_resources(cx);

        let task = cx.update(|cx| {
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
        });
        let sent = cx.foreground_executor().block_test(task).unwrap();

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
            let repository = test_repository(cx);
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
        init_conversation_resources(cx);

        let task = cx.update(|cx| {
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
        });
        let sent = cx.foreground_executor().block_test(task).unwrap();

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
            database::install_for_test(cx, dir.path());
            let mut config =
                JacoConfig::load_from_path_for_test(&dir.path().join("config.toml")).unwrap();
            config.storage.data_dir = Some(dir.path().join("data"));
            crate::state::config::install_for_test(cx, dir.path().join("config.toml"), config)
                .unwrap();
            crate::foundation::i18n::init(cx);
        });
        dir
    }

    fn init_conversation_resources(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::state::providers::init(cx);
            crate::state::projects::init(cx);
            crate::state::prompts::init(cx);
            crate::state::conversation_index::init(cx);
        });
        cx.run_until_parked();
    }

    fn test_repository(cx: &App) -> jaco_db::FreshRepository {
        database::with_ready_repository(cx, |repository| Ok(repository.clone())).unwrap()
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
