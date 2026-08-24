use crate::{
    AgentRunFinalEntry, DATABASE_FILE, FinishAgentRun, FreshStore, NewAgentRun, NewAttachment,
    NewConversation, NewConversationEntry, NewConversationWithUserItem, NewProject, NewPrompt,
    NewProvider, NewProviderModel, NewProviderStep, NewShortcut, NewToolInvocation,
    NewToolInvocationApproval, NewUsageEvent, ToolInvocationApprovalOutcome, UpdatePrompt,
    UpdateProvider, UpdateProviderStepStatus, UpdateShortcut, UpdateToolInvocationStatus,
};
use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Integer, Text},
};
use jaco_core::*;
use serde_json::json;
use std::{collections::HashSet, fs};
use tempfile::tempdir;

mod agent;
mod analytics;
mod attachments;
mod bootstrap;
mod catalog;
mod legacy;
mod projects;
mod schema;

fn count(conn: &mut SqliteConnection, table: &str) -> i64 {
    let sql = format!(
        "SELECT COUNT(*) AS value FROM sqlite_master WHERE type IN ('table', 'view') AND name = '{table}'"
    );
    let exists = sql_query(sql).load::<CountRow>(conn).unwrap()[0].value;
    if exists == 0 {
        return 0;
    }
    let sql = format!("SELECT COUNT(*) AS value FROM {table}");
    sql_query(sql).load::<CountRow>(conn).unwrap()[0].value
}

fn busy_timeout(conn: &mut SqliteConnection) -> i32 {
    sql_query("PRAGMA busy_timeout")
        .load::<BusyTimeoutRow>(conn)
        .unwrap()[0]
        .timeout
}

fn table_sql(conn: &mut SqliteConnection, table: &str) -> String {
    sql_query("SELECT sql AS value FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind::<Text, _>(table)
        .load::<TextRow>(conn)
        .unwrap()[0]
        .value
        .clone()
}

#[derive(diesel::QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(diesel::QueryableByName)]
struct BusyTimeoutRow {
    #[diesel(sql_type = Integer)]
    timeout: i32,
}

#[derive(diesel::QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

fn project(suffix: &str) -> NewProject {
    NewProject {
        path: format!("/tmp/jaco-{suffix}"),
        display_name: format!("Project {suffix}"),
        kind: ProjectKind::Normal,
        pinned: false,
        removed: false,
        metadata: project_metadata(),
    }
}

fn scratch_project(suffix: &str) -> NewProject {
    NewProject {
        path: format!("/tmp/jaco-{suffix}"),
        display_name: format!("Scratch {suffix}"),
        kind: ProjectKind::Scratch,
        pinned: false,
        removed: false,
        metadata: ProjectMetadata {
            scratch_reason: Some("no-project".to_string()),
            git_root: None,
            last_active_conversation_id: None,
        },
    }
}

fn project_metadata() -> ProjectMetadata {
    ProjectMetadata {
        scratch_reason: None,
        git_root: Some("/tmp".to_string()),
        last_active_conversation_id: None,
    }
}

fn conversation(project: &crate::ProjectRecord) -> NewConversation {
    NewConversation {
        project_id: project.id.clone(),
        title: "Conversation".to_string(),
        pinned: false,
        prompt_id: None,
        default_provider_id: None,
        default_model_id: None,
        metadata: conversation_metadata(),
        settings_snapshot: conversation_settings(),
    }
}

fn conversation_metadata() -> ConversationMetadata {
    ConversationMetadata {
        summary: Some("summary".to_string()),
        tags: vec!["tag".to_string()],
    }
}

fn conversation_settings() -> ConversationSettingsSnapshot {
    ConversationSettingsSnapshot {
        prompt: Some(prompt_content()),
        provider_id: Some("provider".to_string()),
        model_id: Some("model".to_string()),
        model_capabilities: Some(model_capabilities()),
        tool_policy: tool_policy(),
    }
}

fn message_item(conversation_id: &str, text: &str) -> NewConversationEntry {
    message_item_with_role(conversation_id, TranscriptRole::User, text)
}

fn message_item_with_role(
    conversation_id: &str,
    role: TranscriptRole,
    text: &str,
) -> NewConversationEntry {
    NewConversationEntry {
        conversation_id: conversation_id.to_string(),
        status: ConversationEntryStatus::Completed,
        agent_run_id: None,
        provider_step_id: None,
        tool_invocation_id: None,
        provider_item_id: None,
        payload: ConversationEntryPayload::Message {
            role,
            content: vec![ContentPart::Text {
                text: text.to_string(),
            }],
        },
    }
}

fn provider() -> NewProvider {
    NewProvider {
        kind: "openai".to_string(),
        display_name: "OpenAI".to_string(),
        enabled: true,
        settings: provider_settings(),
        secret_refs: provider_secret_refs(),
    }
}

fn provider_settings() -> ProviderSettingsPayload {
    ProviderSettingsPayload {
        provider_kind: "openai".to_string(),
        fields: vec![ProviderSettingFieldValue {
            key: "base_url".to_string(),
            value: ProviderSettingValue::String {
                value: "https://api.openai.com/v1".to_string(),
            },
        }],
    }
}

fn provider_secret_refs() -> ProviderSecretRefs {
    ProviderSecretRefs {
        refs: vec![ProviderSecretRef {
            key: "api_key".to_string(),
            storage: "keychain".to_string(),
            ref_id: "openai-api-key".to_string(),
        }],
    }
}

fn provider_model(provider_id: &str, model_id: &str, display_name: &str) -> NewProviderModel {
    NewProviderModel {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        display_name: Some(display_name.to_string()),
        enabled: true,
        capabilities: model_capabilities(),
        metadata: provider_model_metadata(display_name),
        pricing: None,
    }
}

fn model_pricing(
    model_id: &str,
    input: u64,
    output: u64,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
) -> ProviderModelPricingSnapshot {
    ProviderModelPricingSnapshot::new(
        "openai",
        model_id,
        official_provider_pricing_route(&provider_settings()).unwrap(),
        time::OffsetDateTime::UNIX_EPOCH,
        ProviderTokenPriceSnapshot::new(
            UsdNanoPerMillionTokens::new(input),
            UsdNanoPerMillionTokens::new(output),
            cache_read.map(UsdNanoPerMillionTokens::new),
            cache_write.map(UsdNanoPerMillionTokens::new),
        ),
        Vec::new(),
    )
    .unwrap()
}

fn provider_model_metadata(display_name: &str) -> ProviderModelMetadata {
    ProviderModelMetadata {
        display_name: Some(display_name.to_string()),
        family: Some("gpt".to_string()),
        raw: Some(provider_raw(json!({ "owned_by": "openai" }))),
    }
}

fn prompt() -> NewPrompt {
    NewPrompt {
        name: "Default".to_string(),
        content: prompt_content(),
        enabled: true,
        sort_order: 10,
    }
}

fn prompt_content() -> PromptContent {
    PromptContent {
        text: "You are useful.".to_string(),
    }
}

fn attachment_metadata() -> AttachmentMetadata {
    AttachmentMetadata {
        source: AttachmentSource::LocalFile {
            path: "/tmp/notes.txt".to_string(),
        },
        width: None,
        height: None,
        duration_ms: None,
        preview_attachment_id: None,
    }
}

fn agent_run_input(_trigger_entry_id: &str, provider_id: &str, model_id: &str) -> AgentRunInput {
    AgentRunInput {
        prompt_snapshot: Some(prompt_content()),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        settings_snapshot: run_settings(provider_id, model_id),
        runtime_snapshot: AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: "0.22.0".to_string(),
            skill_catalog_hash: Some("skills".to_string()),
            tool_name_strategy: ToolNameStrategy::Namespaced,
        },
        max_steps: 8,
    }
}

fn provider_step_request(
    provider_id: &str,
    model_id: &str,
    input_item_id: &str,
) -> ProviderStepRequestSnapshot {
    ProviderStepRequestSnapshot {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        input_item_ids: vec![input_item_id.to_string()],
        snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
        transport: ProviderTransportSnapshot::ProviderDefault,
        context_mode: ProviderRequestContextSnapshot::FullHistory,
        previous_response_id: None,
        request_body: provider_raw(json!({ "messages": [] })),
    }
}

fn provider_step_response() -> ProviderStepResponseSnapshot {
    ProviderStepResponseSnapshot {
        provider_run_id: Some("resp_1".to_string()),
        output_item_ids: vec!["item_1".to_string()],
        response_body: Some(provider_raw(json!({ "id": "resp_1" }))),
        provider_outputs: Vec::new(),
    }
}

fn provider_run_state(provider_id: &str) -> ProviderRunStateSnapshot {
    ProviderRunStateSnapshot {
        provider_id: provider_id.to_string(),
        provider_run_id: Some("resp_1".to_string()),
        output_item_ids: vec!["item_1".to_string()],
    }
}

fn tool_input() -> ToolInvocationInput {
    ToolInvocationInput {
        source: ToolSource::Mcp {
            server_id: "filesystem".to_string(),
        },
        namespace: Some("filesystem".to_string()),
        tool_name: "read_file".to_string(),
        runtime_tool_name: "filesystem__read_file".to_string(),
        call_id: "call_1".to_string(),
        arguments: ToolArguments {
            value: json!({ "path": "/tmp/notes.txt" }),
        },
        approval_policy: ToolApprovalPolicy::OnRequest,
        execution_policy: ToolExecutionPolicy::Foreground,
    }
}

fn tool_output() -> ToolInvocationOutput {
    ToolInvocationOutput {
        content: vec![ContentPart::Text {
            text: "file body".to_string(),
        }],
        structured_output: Some(StructuredOutput {
            value: json!({ "bytes": 9 }),
        }),
        raw_output: Some(provider_raw(json!({ "stdout": "file body" }))),
        is_error: false,
    }
}

fn approval_request() -> ApprovalRequestPayload {
    ApprovalRequestPayload {
        reason: "Read a local file".to_string(),
        tool_source: ToolSource::Mcp {
            server_id: "filesystem".to_string(),
        },
        tool_name: "read_file".to_string(),
        arguments_preview: "{\"path\":\"/tmp/notes.txt\"}".to_string(),
        access_requests: Vec::new(),
    }
}

fn approval_decision() -> ApprovalDecisionPayload {
    ApprovalDecisionPayload {
        approved: true,
        decided_by: "user".to_string(),
        reason: Some("ok".to_string()),
    }
}

fn approved_tool_invocation_approval() -> ToolInvocationApproval {
    let now = time::OffsetDateTime::now_utc();
    ToolInvocationApproval {
        status: ApprovalStatus::Approved,
        request: approval_request(),
        decision: Some(approval_decision()),
        requested_at: now,
        decided_at: Some(now),
        expires_at: None,
    }
}

fn usage_snapshot() -> ProviderUsageSnapshot {
    ProviderUsageSnapshot {
        input_tokens: 10,
        output_tokens: 20,
        cached_input_tokens: 2,
        cache_write_input_tokens: 3,
        reasoning_tokens: 4,
        total_tokens: 39,
        metadata: Some(provider_raw(json!({ "source": "test" }))),
    }
}

fn run_settings(provider_id: &str, model_id: &str) -> RunSettingsSnapshot {
    RunSettingsSnapshot {
        prompt: Some(prompt_content()),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        model_capabilities: model_capabilities(),
        provider_settings: provider_settings(),
        reasoning_selection: None,
        tool_policy: tool_policy(),
    }
}

fn tool_policy() -> ToolPolicySnapshot {
    ToolPolicySnapshot {
        approval_policy: ToolApprovalPolicy::OnRequest,
        enabled_sources: vec![
            ToolSource::Local,
            ToolSource::Mcp {
                server_id: "filesystem".to_string(),
            },
        ],
        max_steps: 8,
        approval_mode: ToolApprovalMode::RequestApproval,
        permission_scope: None,
    }
}

fn model_capabilities() -> ModelCapabilitiesSnapshot {
    ModelCapabilitiesSnapshot {
        context_window: None,
        text_input: true,
        text_output: true,
        streaming: true,
        image_input: Some(ImageInputCapabilitySnapshot {
            max_images: Some(4),
        }),
        file_input: Some(FileInputCapabilitySnapshot { max_files: Some(8) }),
        audio_input: false,
        image_generation: false,
        tool_calling: Some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        }),
        hosted_web_search: true,
        remote_mcp: false,
        reasoning: Some(ReasoningCapabilitySnapshot {
            default_effort: "medium".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string()],
            summaries: true,
            control: None,
            source: Default::default(),
        }),
        structured_output: true,
        stateful_response_continuation: true,
        extension: ProviderCapabilityExtensionSnapshot::OpenAi {
            responses_api: true,
            raw: Some(provider_raw(json!({ "family": "gpt" }))),
        },
    }
}

fn run_error() -> RunErrorPayload {
    RunErrorPayload {
        code: "provider_error".to_string(),
        message: "temporary".to_string(),
        retryable: true,
        provider: Some("openai".to_string()),
        raw: Some(provider_raw(json!({ "status": 500 }))),
    }
}

fn provider_raw(value: serde_json::Value) -> ProviderRawPayload {
    ProviderRawPayload {
        provider_kind: "openai".to_string(),
        value,
    }
}
