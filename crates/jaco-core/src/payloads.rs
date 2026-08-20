use crate::{
    AgentRunId, AttachmentId, ConversationEntryId, ConversationId, ProjectId, ProviderId,
    ProviderModelId, ProviderStepId, ToolInvocationId,
};

mod agent;
mod app;
mod capabilities;
mod conversation;
mod foundation;
mod resources;

pub use agent::*;
pub use app::*;
pub use capabilities::*;
pub use conversation::*;
pub use foundation::*;
pub use resources::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_are_uuid_v7_strings() {
        let id = crate::new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().nth(14), Some('7'));
    }

    #[test]
    fn legacy_project_metadata_roundtrips_without_sidebar_flags() {
        let metadata: ProjectMetadata = serde_json::from_value(json!({
            "scratchReason": null,
            "gitRoot": "/tmp/project",
            "lastActiveConversationId": null
        }))
        .unwrap();

        assert_eq!(metadata.scratch_reason, None);
        assert_eq!(metadata.git_root, Some("/tmp/project".to_string()));
        assert_eq!(metadata.last_active_conversation_id, None);
    }

    #[test]
    fn app_settings_payload_roundtrips_as_typed_settings() {
        let payload = AppSettingsPayload {
            language: AppLanguage::Chinese,
            theme: AppThemeSettings {
                mode: AppThemeMode::Dark,
                light_theme: Some("preset:Default Light".to_string()),
                dark_theme: Some("material-you:#3271AE".to_string()),
                custom_theme_colors: vec!["#3271AE".to_string()],
            },
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            http_proxy: Some("http://127.0.0.1:8080".to_string()),
            default_project_id: Some("project_1".to_string()),
        };

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["language"], "zh-CN");
        assert_eq!(value["theme"]["mode"], "dark");
        assert_eq!(value["temporaryHotkey"], "cmd+shift+j");
        assert_eq!(value["httpProxy"], "http://127.0.0.1:8080");
        assert_eq!(
            serde_json::from_value::<AppSettingsPayload>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn app_settings_payload_defaults_unknown_language_and_theme_mode_to_system() {
        let payload: AppSettingsPayload = serde_json::from_value(json!({
            "language": "fr-FR",
            "theme": {
                "mode": "auto"
            }
        }))
        .unwrap();

        assert_eq!(payload.language, AppLanguage::System);
        assert_eq!(payload.theme.mode, AppThemeMode::System);
        assert_eq!(payload.temporary_hotkey, None);
        assert_eq!(payload.http_proxy, None);
        assert_eq!(payload.default_project_id, None);
    }

    #[test]
    fn skill_activation_roundtrips_with_file_snapshot() {
        let payload = ConversationEntryPayload::SkillActivation(SkillActivationEntry {
            name: "rust".to_string(),
            source_kind: SkillSourceKind::Project,
            skill_file_path: "/repo/.agents/skills/rust/SKILL.md".to_string(),
            directory_path: "/repo/.agents/skills/rust".to_string(),
            content_sha256: "abc123".to_string(),
            content: vec![ContentPart::Text {
                text: "Use cargo test.".to_string(),
            }],
        });

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["type"], "skillActivation");
        assert_eq!(
            serde_json::from_value::<ConversationEntryPayload>(value).unwrap(),
            payload
        );
        assert_eq!(payload.kind(), ConversationEntryKind::SkillActivation);
    }

    #[test]
    fn typed_status_search_text_uses_stable_code() {
        let payload = ConversationEntryPayload::Status(ConversationStatusEntry {
            code: ConversationStatusCode::MaxStepsReached,
            message: Some("provider stopped after the guard".to_string()),
        });

        assert_eq!(
            payload.search_text(),
            "max_steps_reached\nprovider stopped after the guard"
        );
    }

    #[test]
    fn tool_runtime_name_roundtrips() {
        let payload = ToolInvocationInput {
            source: ToolSource::Mcp {
                server_id: "filesystem".to_string(),
            },
            namespace: Some("filesystem".to_string()),
            tool_name: "read_file".to_string(),
            runtime_tool_name: "filesystem__read_file".to_string(),
            call_id: "call-1".to_string(),
            arguments: ToolArguments {
                value: json!({ "path": "/tmp/a" }),
            },
            approval_policy: ToolApprovalPolicy::OnRequest,
            execution_policy: ToolExecutionPolicy::Foreground,
        };

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["runtimeToolName"], "filesystem__read_file");
        assert_eq!(
            serde_json::from_value::<ToolInvocationInput>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn tool_policy_defaults_approval_mode_for_old_json() {
        let payload: ToolPolicySnapshot = serde_json::from_value(json!({
            "approvalPolicy": "on_request",
            "enabledSources": [{ "type": "local" }],
            "maxSteps": 8
        }))
        .unwrap();

        assert_eq!(payload.approval_mode, ToolApprovalMode::RequestApproval);
        assert_eq!(payload.permission_scope, None);
    }

    #[test]
    fn tool_policy_roundtrips_approval_mode_and_scope() {
        let payload = ToolPolicySnapshot {
            approval_policy: ToolApprovalPolicy::OnRequest,
            enabled_sources: vec![ToolSource::Local],
            max_steps: 8,
            approval_mode: ToolApprovalMode::FullAccess,
            permission_scope: Some(ToolPermissionScopeSnapshot {
                project_roots: vec!["/repo".to_string()],
                external_read_requires_approval: false,
                external_write_requires_approval: true,
            }),
        };

        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["approvalMode"], "full_access");
        assert_eq!(
            serde_json::from_value::<ToolPolicySnapshot>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn approval_request_defaults_access_requests_for_old_json() {
        let payload: ApprovalRequestPayload = serde_json::from_value(json!({
            "reason": "Needs access",
            "toolSource": { "type": "local" },
            "toolName": "write_file",
            "argumentsPreview": "{\"path\":\"/tmp/out.txt\"}"
        }))
        .unwrap();

        assert!(payload.access_requests.is_empty());
    }

    #[test]
    fn tool_access_request_roundtrips() {
        for kind in [
            ToolAccessKind::Read,
            ToolAccessKind::Write,
            ToolAccessKind::Execute,
            ToolAccessKind::Network,
        ] {
            let payload = ToolAccessRequestPayload {
                kind,
                target: "/tmp/out.txt".to_string(),
                normalized_path: Some("/tmp/out.txt".to_string()),
                within_project: false,
                reason_key: Some("write_file".to_string()),
            };
            let value = serde_json::to_value(&payload).unwrap();
            assert_eq!(
                serde_json::from_value::<ToolAccessRequestPayload>(value).unwrap(),
                payload
            );
        }
    }

    #[test]
    fn rig_runtime_snapshot_roundtrips() {
        let snapshot = AgentRuntimeSnapshot {
            engine: AgentEngineKind::Rig,
            engine_version: "0.22.0".to_string(),
            skill_catalog_hash: Some("skills".to_string()),
            tool_name_strategy: ToolNameStrategy::Namespaced,
        };

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["engine"], "rig");
        assert!(value.get("mcpConfigHash").is_none());
        assert!(value.get("mcpConfigSnapshot").is_none());
        assert_eq!(
            serde_json::from_value::<AgentRuntimeSnapshot>(value).unwrap(),
            snapshot
        );
    }

    #[test]
    fn provider_step_snapshot_kind_roundtrips() {
        let snapshot = ProviderStepRequestSnapshot {
            provider_id: "openai".to_string(),
            model_id: "gpt-5.2".to_string(),
            input_item_ids: vec!["item-1".to_string()],
            snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
            transport: ProviderTransportSnapshot::ProviderDefault,
            context_mode: ProviderRequestContextSnapshot::FullHistory,
            previous_response_id: None,
            request_body: ProviderRawPayload {
                provider_kind: "openai".to_string(),
                value: json!({ "messages": [] }),
            },
        };

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["snapshotKind"], "rig_completion_request");
        assert_eq!(
            serde_json::from_value::<ProviderStepRequestSnapshot>(value).unwrap(),
            snapshot
        );
    }

    #[test]
    fn legacy_reasoning_capability_defaults_to_heuristic_source() {
        let payload = json!({
            "defaultEffort": "medium",
            "efforts": ["low", "medium", "high"],
            "summaries": true
        });

        let snapshot: ReasoningCapabilitySnapshot = serde_json::from_value(payload).unwrap();

        assert_eq!(snapshot.control, None);
        assert!(matches!(
            snapshot.source,
            CapabilitySourceSnapshot::Heuristic { .. }
        ));
    }

    #[test]
    fn run_settings_defaults_missing_reasoning_selection() {
        let payload = json!({
            "prompt": null,
            "providerId": "provider",
            "modelId": "model",
            "modelCapabilities": {
                "textInput": true,
                "textOutput": true,
                "streaming": true,
                "imageInput": null,
                "fileInput": null,
                "audioInput": false,
                "imageGeneration": false,
                "toolCalling": null,
                "hostedWebSearch": false,
                "remoteMcp": false,
                "reasoning": null,
                "structuredOutput": false,
                "statefulResponseContinuation": false,
                "extension": { "provider": "none" }
            },
            "providerSettings": {
                "providerKind": "openai",
                "fields": []
            },
            "toolPolicy": {
                "approvalPolicy": "never",
                "enabledSources": [],
                "maxSteps": 8
            }
        });

        let snapshot: RunSettingsSnapshot = serde_json::from_value(payload).unwrap();

        assert_eq!(snapshot.reasoning_selection, None);
        assert_eq!(snapshot.model_capabilities.context_window, None);
    }

    #[test]
    fn context_window_capability_roundtrips_discovered_and_manual_sources() {
        for source in [
            CapabilitySourceSnapshot::ApiDiscovered {
                provider: "gemini".to_string(),
                endpoint: "/v1beta/models".to_string(),
            },
            CapabilitySourceSnapshot::Manual {
                source: "model settings".to_string(),
            },
        ] {
            let mut model_capabilities = crate::conservative_model_capabilities("gemini");
            model_capabilities.context_window = Some(ContextWindowCapabilitySnapshot {
                tokens: std::num::NonZeroU64::new(128_000).unwrap(),
                source,
            });
            let snapshot = RunSettingsSnapshot {
                prompt: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                model_capabilities,
                provider_settings: ProviderSettingsPayload {
                    provider_kind: "gemini".to_string(),
                    fields: Vec::new(),
                },
                reasoning_selection: None,
                tool_policy: ToolPolicySnapshot {
                    approval_policy: ToolApprovalPolicy::Never,
                    enabled_sources: Vec::new(),
                    max_steps: 8,
                    approval_mode: ToolApprovalMode::RequestApproval,
                    permission_scope: None,
                },
            };

            let value = serde_json::to_value(&snapshot).unwrap();
            assert_eq!(
                value["modelCapabilities"]["contextWindow"]["tokens"],
                128_000
            );
            assert_eq!(
                serde_json::from_value::<RunSettingsSnapshot>(value).unwrap(),
                snapshot
            );
        }
    }

    #[test]
    fn context_window_capability_rejects_zero_tokens() {
        let mut snapshot = crate::conservative_model_capabilities("openai");
        snapshot.context_window = Some(ContextWindowCapabilitySnapshot {
            tokens: std::num::NonZeroU64::new(1).unwrap(),
            source: CapabilitySourceSnapshot::ApiDiscovered {
                provider: "openai".to_string(),
                endpoint: "rig model listing".to_string(),
            },
        });
        let mut value = serde_json::to_value(snapshot).unwrap();
        value["contextWindow"]["tokens"] = serde_json::json!(0);

        assert!(serde_json::from_value::<ModelCapabilitiesSnapshot>(value).is_err());
    }

    #[test]
    fn unknown_context_window_is_omitted_from_capability_json() {
        let snapshot = crate::conservative_model_capabilities("openai");

        let value = serde_json::to_value(&snapshot).unwrap();

        assert!(value.get("contextWindow").is_none());
    }
}
