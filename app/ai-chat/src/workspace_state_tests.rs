use super::{
    ConversationDraft, LatestModelPreset, PersistedTab, PersistedWorkspaceState, WorkspaceState,
};
use crate::database::Mode;

fn sample_request_template() -> serde_json::Value {
    serde_json::json!({
        "model": "gpt-4o",
        "stream": true
    })
}

fn sample_draft() -> ConversationDraft {
    ConversationDraft {
        text: "hello".to_string(),
        provider_name: "OpenAI".to_string(),
        model_id: "gpt-4o".to_string(),
        mode: Mode::Single,
        selected_template_id: Some(42),
        request_template: sample_request_template(),
    }
}

#[test]
fn conversation_draft_to_latest_model_preset_keeps_model_fields_only() {
    let preset = sample_draft().to_latest_model_preset().unwrap();

    assert_eq!(preset.provider_name, "OpenAI");
    assert_eq!(preset.model_id, "gpt-4o");
    assert_eq!(preset.request_template, sample_request_template());
}

#[test]
fn conversation_draft_to_latest_model_preset_returns_none_without_model() {
    let mut draft = sample_draft();
    draft.model_id.clear();

    assert_eq!(draft.to_latest_model_preset(), None);
}

#[test]
fn latest_model_preset_seeds_blank_contextual_draft() {
    let preset = LatestModelPreset {
        provider_name: "OpenAI".to_string(),
        model_id: "gpt-4o".to_string(),
        request_template: sample_request_template(),
    };

    let draft = ConversationDraft::from_latest_model_preset(&preset);

    assert_eq!(draft.text, "");
    assert_eq!(draft.provider_name, "OpenAI");
    assert_eq!(draft.model_id, "gpt-4o");
    assert_eq!(draft.mode, Mode::Contextual);
    assert_eq!(draft.selected_template_id, None);
    assert_eq!(draft.request_template, sample_request_template());
}

#[test]
fn resolve_initial_conversation_draft_prefers_explicit_and_persisted_drafts() {
    let explicit = sample_draft();
    let persisted = ConversationDraft {
        text: "persisted".to_string(),
        ..sample_draft()
    };
    let latest = LatestModelPreset {
        provider_name: "OpenAI".to_string(),
        model_id: "gpt-5".to_string(),
        request_template: serde_json::json!({ "model": "gpt-5" }),
    };

    let state = WorkspaceState {
        persisted: PersistedWorkspaceState {
            tabs: vec![PersistedTab::Conversation {
                id: 1,
                draft: Some(persisted.clone()),
            }],
            latest_model_preset: Some(latest),
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };

    assert_eq!(
        state.resolve_initial_conversation_draft(1, Some(explicit.clone())),
        Some(explicit)
    );
    assert_eq!(
        state.resolve_initial_conversation_draft(1, None),
        Some(persisted)
    );
}

#[test]
fn resolve_initial_conversation_draft_falls_back_to_latest_preset() {
    let latest = LatestModelPreset {
        provider_name: "OpenAI".to_string(),
        model_id: "gpt-5".to_string(),
        request_template: serde_json::json!({ "model": "gpt-5", "stream": false }),
    };

    let state = WorkspaceState {
        persisted: PersistedWorkspaceState {
            latest_model_preset: Some(latest.clone()),
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };

    assert_eq!(
        state.resolve_initial_conversation_draft(9, None),
        Some(ConversationDraft::from_latest_model_preset(&latest))
    );
}

#[test]
fn persisted_workspace_state_defaults_latest_model_preset_when_missing() {
    let state: PersistedWorkspaceState = toml::from_str(
        r#"
version = 1
sidebar_width = 300.0
"#,
    )
    .unwrap();

    assert_eq!(state.latest_model_preset, None);
}

#[test]
fn persisted_workspace_state_roundtrips_latest_model_preset_request_template() {
    let state = PersistedWorkspaceState {
        latest_model_preset: Some(LatestModelPreset {
            provider_name: "OpenAI".to_string(),
            model_id: "gpt-4o".to_string(),
            request_template: sample_request_template(),
        }),
        ..PersistedWorkspaceState::default()
    };

    let text = toml::to_string_pretty(&state).unwrap();
    let parsed: PersistedWorkspaceState = toml::from_str(&text).unwrap();

    assert_eq!(parsed, state);
}

#[test]
fn sync_conversation_chat_form_state_keeps_latest_preset_when_draft_is_temporarily_empty() {
    let latest = LatestModelPreset {
        provider_name: "OpenAI".to_string(),
        model_id: "gpt-5".to_string(),
        request_template: serde_json::json!({ "model": "gpt-5", "stream": false }),
    };

    let mut state = WorkspaceState {
        persisted: PersistedWorkspaceState {
            latest_model_preset: Some(latest.clone()),
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };

    let changed = state.upsert_latest_model_preset(latest.clone());

    assert!(!changed);
    assert_eq!(state.persisted.latest_model_preset, Some(latest));
}
