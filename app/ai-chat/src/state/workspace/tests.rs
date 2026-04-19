use super::{
    ConversationDraft, PersistedWorkspaceState, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH,
    SIDEBAR_MIN_WIDTH, WindowPlacementKind, WorkspaceState, persistence::*,
};
use crate::database::Mode;
use gpui::{Bounds, WindowBounds, point, px, size};

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
    assert_eq!(state.main_window_bounds, None);
    assert_eq!(state.settings_window_bounds, None);
}

fn window_bounds(x: f32, y: f32, width: f32, height: f32) -> PersistedWindowBounds {
    PersistedWindowBounds {
        mode: PersistedWindowMode::Windowed,
        x,
        y,
        width,
        height,
        display_id: Some(1),
    }
}

fn display(
    id: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    is_primary: bool,
) -> WindowDisplaySnapshot {
    WindowDisplaySnapshot {
        id,
        bounds: Bounds::new(point(px(x), px(y)), size(px(width), px(height))),
        is_primary,
    }
}

#[test]
fn persisted_workspace_state_roundtrips_window_bounds_per_window_type() {
    let main_window_bounds = window_bounds(10., 20., 1200., 800.);
    let settings_window_bounds = PersistedWindowBounds {
        mode: PersistedWindowMode::Maximized,
        x: 50.,
        y: 60.,
        width: 960.,
        height: 720.,
        display_id: Some(2),
    };
    let state = PersistedWorkspaceState {
        main_window_bounds: Some(main_window_bounds),
        settings_window_bounds: Some(settings_window_bounds),
        ..PersistedWorkspaceState::default()
    };

    let text = toml::to_string_pretty(&state).unwrap();
    let parsed: PersistedWorkspaceState = toml::from_str(&text).unwrap();

    assert_eq!(parsed.main_window_bounds, Some(main_window_bounds));
    assert_eq!(parsed.settings_window_bounds, Some(settings_window_bounds));
}

#[test]
fn persisted_window_bounds_resolve_only_when_visible_on_current_display() {
    let displays = vec![display(1, 0., 0., 1440., 900., true)];
    let valid = window_bounds(10., 20., 1200., 800.);

    let resolved = resolve_persisted_window_bounds(Some(valid), &displays).unwrap();

    assert_eq!(resolved.display_id, 1);
    assert_eq!(
        resolved.window_bounds,
        WindowBounds::Windowed(Bounds::new(
            point(px(10.), px(20.)),
            size(px(1200.), px(800.)),
        ))
    );
    assert_eq!(
        resolve_persisted_window_bounds(Some(window_bounds(10., 20., 0., 800.)), &displays),
        None
    );
    assert_eq!(
        resolve_persisted_window_bounds(Some(window_bounds(2000., 20., 1200., 800.)), &displays),
        None
    );
    assert_eq!(
        resolve_persisted_window_bounds(
            Some(PersistedWindowBounds {
                display_id: Some(99),
                ..valid
            }),
            &displays,
        ),
        None
    );
}

#[test]
fn fallback_display_prefers_saved_display_then_primary() {
    let displays = vec![
        display(1, 0., 0., 1440., 900., true),
        display(2, 1440., 0., 1920., 1080., false),
    ];

    assert_eq!(
        fallback_display_id_for_persisted_window(
            Some(PersistedWindowBounds {
                display_id: Some(2),
                ..window_bounds(3000., 3000., 1200., 800.)
            }),
            &displays,
        ),
        Some(2)
    );
    assert_eq!(
        fallback_display_id_for_persisted_window(
            Some(PersistedWindowBounds {
                display_id: Some(99),
                ..window_bounds(3000., 3000., 1200., 800.)
            }),
            &displays,
        ),
        Some(1)
    );
}

#[test]
fn window_bounds_updates_are_separated_by_window_type() {
    let main = window_bounds(10., 20., 1200., 800.);
    let settings = window_bounds(30., 40., 960., 720.);
    let mut state = WorkspaceState {
        persisted: PersistedWorkspaceState::default(),
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };

    assert!(state.update_persisted_window_bounds(WindowPlacementKind::Main, main));
    assert!(!state.update_persisted_window_bounds(WindowPlacementKind::Main, main));
    assert!(state.update_persisted_window_bounds(WindowPlacementKind::Settings, settings));

    assert_eq!(state.persisted.main_window_bounds, Some(main));
    assert_eq!(state.persisted.settings_window_bounds, Some(settings));
}

#[test]
fn sidebar_width_clamps_persisted_values() {
    let below_min = WorkspaceState {
        persisted: PersistedWorkspaceState {
            sidebar_width: f32::from(SIDEBAR_MIN_WIDTH) - 1.,
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };
    let above_max = WorkspaceState {
        persisted: PersistedWorkspaceState {
            sidebar_width: f32::from(SIDEBAR_MAX_WIDTH) + 1.,
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };
    let normal = WorkspaceState {
        persisted: PersistedWorkspaceState {
            sidebar_width: f32::from(SIDEBAR_DEFAULT_WIDTH),
            ..PersistedWorkspaceState::default()
        },
        tabs: Vec::new(),
        active_tab: None,
        save_task: None,
    };

    assert_eq!(below_min.sidebar_width(), SIDEBAR_MIN_WIDTH);
    assert_eq!(above_max.sidebar_width(), SIDEBAR_MAX_WIDTH);
    assert_eq!(normal.sidebar_width(), SIDEBAR_DEFAULT_WIDTH);
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
