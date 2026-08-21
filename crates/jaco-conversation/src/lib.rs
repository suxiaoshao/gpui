use jaco_core::{Conversation, ConversationId, ConversationSummary};
use jaco_db::{ConversationTimelineRecords, FreshRepository};

pub type Result<T> = std::result::Result<T, ConversationError>;

#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error(transparent)]
    Database(#[from] jaco_db::DbError),
}

pub struct ConversationService<'a> {
    repository: &'a FreshRepository,
}

impl<'a> ConversationService<'a> {
    pub fn new(repository: &'a FreshRepository) -> Self {
        Self { repository }
    }

    pub fn load_catalog(&self) -> Result<Vec<ConversationSummary>> {
        self.repository
            .list_sidebar_conversations()
            .map_err(Into::into)
    }

    pub fn load(&self, id: &ConversationId) -> Result<Option<Conversation>> {
        self.repository
            .conversation_timeline_records(id)
            .map(|records| records.map(conversation_from_records))
            .map_err(Into::into)
    }

    pub fn search_catalog(&self, query: &str, limit: usize) -> Result<Vec<ConversationSummary>> {
        self.repository
            .search_sidebar_conversations(query, limit)
            .map_err(Into::into)
    }

    pub fn load_scratch_catalog(&self, query: &str) -> Result<Vec<ConversationSummary>> {
        self.repository
            .list_no_project_conversations(query)
            .map_err(Into::into)
    }

    pub fn set_pinned(&self, id: &ConversationId, pinned: bool) -> Result<ConversationSummary> {
        self.repository
            .set_conversation_pinned(id, pinned)
            .map_err(Into::into)
    }

    pub fn delete(&self, id: &ConversationId) -> Result<ConversationSummary> {
        self.repository
            .soft_delete_conversation(id)
            .map_err(Into::into)
    }
}

fn conversation_from_records(records: ConversationTimelineRecords) -> Conversation {
    Conversation {
        summary: records.conversation,
        project: records.project,
        entries: records.items,
        attachments: records.attachments,
        runs: records.runs,
        provider_steps: records.provider_steps,
        tool_invocations: records.tool_invocations,
        agent_message_request_usages: records.agent_message_request_usages,
        latest_context_request_usage: records.latest_context_request_usage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        AgentMessageRequestUsage, ConversationContextRequestUsage, ConversationMetadata,
        ConversationSettingsSnapshot, ProjectKind, ProjectMetadata, ProviderUsageSnapshot,
        ToolApprovalMode, ToolApprovalPolicy, ToolPolicySnapshot,
    };
    use jaco_db::{FreshStore, NewConversation, NewProject};

    #[test]
    fn empty_store_has_ready_empty_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3")).unwrap();
        let repository = store.repository();
        let service = ConversationService::new(&repository);

        assert_eq!(service.load_catalog().unwrap(), Vec::new());
    }

    #[test]
    fn conversation_from_records_moves_request_usage_without_reassociation() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3")).unwrap();
        let repository = store.repository();
        let project = repository
            .insert_project(NewProject {
                path: "/tmp/jaco-conversation-usage".to_string(),
                display_name: "Conversation Usage".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: Some("/tmp".to_string()),
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Usage".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: None,
                    model_id: None,
                    model_capabilities: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::OnRequest,
                        enabled_sources: Vec::new(),
                        max_steps: 8,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
            })
            .unwrap();
        let mut records = repository
            .conversation_timeline_records(&conversation.id)
            .unwrap()
            .unwrap();
        let completed_at = records.conversation.updated_at;
        let usages = vec![
            AgentMessageRequestUsage {
                conversation_entry_id: "entry-reported".to_string(),
                agent_run_id: "run-reported".to_string(),
                provider_step_id: "step-reported".to_string(),
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                provider_kind: "openai".to_string(),
                completed_at,
                usage: Some(ProviderUsageSnapshot {
                    input_tokens: 10,
                    output_tokens: 2,
                    cached_input_tokens: 3,
                    cache_write_input_tokens: 0,
                    reasoning_tokens: 0,
                    total_tokens: 12,
                    metadata: None,
                }),
            },
            AgentMessageRequestUsage {
                conversation_entry_id: "entry-missing".to_string(),
                agent_run_id: "run-missing".to_string(),
                provider_step_id: "step-missing".to_string(),
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                provider_kind: "openai".to_string(),
                completed_at,
                usage: None,
            },
        ];
        records.agent_message_request_usages = usages.clone();

        let hydrated = conversation_from_records(records);

        assert_eq!(hydrated.agent_message_request_usages, usages);
    }

    #[test]
    fn load_hydrates_latest_context_request_usage_without_reselection() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3")).unwrap();
        let repository = store.repository();
        let project = repository
            .insert_project(NewProject {
                path: "/tmp/jaco-conversation-context".to_string(),
                display_name: "Conversation Context".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: Some("/tmp".to_string()),
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Context".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: None,
                    model_id: None,
                    model_capabilities: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::OnRequest,
                        enabled_sources: Vec::new(),
                        max_steps: 8,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
            })
            .unwrap();
        let mut records = repository
            .conversation_timeline_records(&conversation.id)
            .unwrap()
            .unwrap();
        let completed_at = records.conversation.updated_at;
        let request_usage = ConversationContextRequestUsage {
            agent_run_id: "run-latest".to_string(),
            provider_step_id: "step-latest".to_string(),
            provider_step_seq: 3,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            provider_step_completed_at: completed_at,
            agent_run_completed_at: completed_at,
            usage: Some(ProviderUsageSnapshot {
                input_tokens: 10,
                output_tokens: 2,
                cached_input_tokens: 3,
                cache_write_input_tokens: 0,
                reasoning_tokens: 0,
                total_tokens: 12,
                metadata: None,
            }),
        };
        records.latest_context_request_usage = Some(request_usage.clone());

        let hydrated = conversation_from_records(records);

        assert_eq!(hydrated.latest_context_request_usage, Some(request_usage));
    }

    #[test]
    fn load_preserves_latest_context_request_with_missing_usage() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3")).unwrap();
        let repository = store.repository();
        let project = repository
            .insert_project(NewProject {
                path: "/tmp/jaco-conversation-context-missing".to_string(),
                display_name: "Conversation Context Missing".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: Some("/tmp".to_string()),
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Missing Context Usage".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: None,
                    model_id: None,
                    model_capabilities: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::OnRequest,
                        enabled_sources: Vec::new(),
                        max_steps: 8,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
            })
            .unwrap();
        let mut records = repository
            .conversation_timeline_records(&conversation.id)
            .unwrap()
            .unwrap();
        let completed_at = records.conversation.updated_at;
        let request_usage = ConversationContextRequestUsage {
            agent_run_id: "run-missing".to_string(),
            provider_step_id: "step-missing".to_string(),
            provider_step_seq: 1,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            provider_step_completed_at: completed_at,
            agent_run_completed_at: completed_at,
            usage: None,
        };
        records.latest_context_request_usage = Some(request_usage.clone());

        let hydrated = conversation_from_records(records);

        assert_eq!(hydrated.latest_context_request_usage, Some(request_usage));
    }
}
