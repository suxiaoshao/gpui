use super::super::{Activity, CatalogMessage, ConversationState, Discovery, ModelChange};
use super::super::{
    Session,
    content::{BodyState, LoadStage},
};
use crate::state::pi;
use gpui_kit::{AppContext, Entity, TestAppContext};
use gpui_operation::Transition;
use pi_rpc::{Client, protocol::StreamingBehavior};
use std::{path::PathBuf, sync::OnceLock};

fn fixture() -> PathBuf {
    static FIXTURE: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("process.rs");
            std::fs::write(&source, include_str!("process.rs")).unwrap();
            let binary = dir.path().join(format!(
                "gupi-loading-fixture{}",
                std::env::consts::EXE_SUFFIX
            ));
            let output = std::process::Command::new(
                std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            )
            .arg("--edition=2024")
            .arg(source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            (dir, binary)
        })
        .1
        .clone()
}
fn begin(
    cx: &mut TestAppContext,
    flags: &[&str],
) -> (tempfile::TempDir, Entity<ConversationState>, String) {
    begin_with_prompt(cx, flags, None)
}
fn begin_with_prompt(
    cx: &mut TestAppContext,
    flags: &[&str],
    prompt: Option<&str>,
) -> (tempfile::TempDir, Entity<ConversationState>, String) {
    let (dir, owner, key) = prepare(cx, flags);
    owner.update(cx, |state, cx| {
        if let Some(prompt) = prompt {
            state.set_draft(&key, prompt.into(), cx);
            state.send(&key, StreamingBehavior::Steer, cx);
        } else {
            state.connect(&key, cx);
        }
    });
    (dir, owner, key)
}
fn prepare(
    cx: &mut TestAppContext,
    flags: &[&str],
) -> (tempfile::TempDir, Entity<ConversationState>, String) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_tokio::init(cx);
        pi::init(cx);
    });
    let dir = tempfile::tempdir().unwrap();
    for flag in flags {
        std::fs::write(dir.path().join(flag), "").unwrap();
    }
    let owner = cx.new(|cx| ConversationState::new(fixture(), cx));
    let key = owner.update(cx, |state, _| {
        state.discovery = Discovery {
            home: dir.path().into(),
            agent: dir.path().join("agent"),
            current: dir.path().into(),
            session_override: None,
        };
        state.insert_draft(Some(dir.path().into()));
        state.selected.clone().unwrap()
    });
    (dir, owner, key)
}

#[gpui_kit::test]
async fn new_conversation_model_options_and_catalog_have_independent_lifecycles(
    cx: &mut TestAppContext,
) {
    let (dir, owner, key) = prepare(cx, &[]);
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
            && state.sessions[&key].thinking_levels.data().is_some()
    })
    .await;
    owner.read_with(cx, |state, _| {
        assert_eq!(state.scan_serial, 0);
        assert_eq!(state.sessions[&key].body_state(), BodyState::New);
        assert!(state.infos().is_empty());
        assert!(!state.sessions[&key].core_read.running());
        assert_eq!(
            state.sessions[&key].model_identity(),
            Some(("fixture".into(), "alpha".into()))
        );
    });
    for command in ["get_entries", "get_session_stats", "get_fork_messages"] {
        assert_eq!(
            count(dir.path(), command),
            0,
            "model options must not read {command}"
        );
    }
    assert_eq!(
        count(dir.path(), "get_state"),
        1,
        "only the connection handshake reads state"
    );
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].models.running() && !state.sessions[&key].thinking_levels.running()
    })
    .await;
    assert_eq!(count(dir.path(), "get_available_models"), 2);
    assert_eq!(count(dir.path(), "get_entries"), 0);
    assert_eq!(owner.read_with(cx, |state, _| state.scan_serial), 0);
    owner.update(cx, |state, cx| state.scan(cx));
    cx.condition(&owner, |state, _| {
        state.catalog.data().is_some() && !state.catalog.running()
    })
    .await;
    assert_eq!(
        count(dir.path(), "get_available_models"),
        2,
        "catalog refresh must not reload models"
    );
    assert!(owner.read_with(cx, |state, _| state.infos().is_empty()));
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn sending_can_join_a_connection_started_only_for_model_options(cx: &mut TestAppContext) {
    let (dir, owner, key) = prepare(cx, &[]);
    owner.update(cx, |state, cx| {
        state.refresh_models(&key, cx);
        state.set_draft(&key, "send while model connection starts".into(), cx);
        state.send(&key, StreamingBehavior::Steer, cx);
    });
    cx.condition(&owner, |state, _| {
        state.sessions[&key].state.is_some() && state.sessions[&key].draft.is_empty()
    })
    .await;
    assert_eq!(count(dir.path(), "prompt"), 1);
    assert!(count(dir.path(), "get_entries") >= 1);
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn empty_history_has_a_full_loading_and_retry_lifecycle(cx: &mut TestAppContext) {
    let (dir, owner, key) = prepare(
        cx,
        &["hold-get_entries", "fail-get_entries", "empty-entries"],
    );
    let path = dir.path().join("existing.jsonl");
    std::fs::write(&path, serde_json::json!({"type":"session", "id":"fixture", "version":3, "timestamp":"2026-09-11T00:00:00Z", "cwd":dir.path()}).to_string()).unwrap();
    owner.update(cx, |state, cx| {
        let mut info = state.sessions[&key].info.clone();
        info.path = path;
        state
            .sessions
            .insert(key.clone(), Session::new(info, String::new()));
        assert_eq!(
            state.sessions[&key].body_state(),
            BodyState::Loading(LoadStage::Connecting)
        );
        state.connect(&key, cx);
        assert_eq!(
            state.sessions[&key].body_state(),
            BodyState::Loading(LoadStage::CheckingFile)
        );
    });
    cx.condition(&owner, |state, _| {
        state.sessions[&key].body_state() == BodyState::Loading(LoadStage::History)
    })
    .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    control(&client, "wait_for", "get_entries", 1).await;
    control(&client, "release", "get_entries", 0).await;
    cx.condition(&owner, |state, _| {
        matches!(state.sessions[&key].body_state(), BodyState::Failed(_))
    })
    .await;
    std::fs::remove_file(dir.path().join("fail-get_entries")).unwrap();
    owner.update(cx, |state, cx| state.refresh(&key, cx));
    cx.condition(&owner, |state, _| {
        state.sessions[&key].body_state() == BodyState::Ready
    })
    .await;
    let revision = owner.read_with(cx, |state, _| {
        assert!(state.sessions[&key].history().entries.is_empty());
        state.sessions[&key].history().revision
    });
    std::fs::write(dir.path().join("hold-get_entries"), "").unwrap();
    std::fs::write(dir.path().join("fail-get_entries"), "").unwrap();
    owner.update(cx, |state, cx| {
        state.refresh(&key, cx);
        assert_eq!(
            state.sessions[&key].body_state(),
            BodyState::Refreshing(LoadStage::History)
        );
    });
    control(&client, "wait_for", "get_entries", 3).await;
    control(&client, "release", "get_entries", 0).await;
    cx.condition(&owner, |state, _| {
        matches!(
            state.sessions[&key].body_state(),
            BodyState::RefreshFailed(_)
        )
    })
    .await;
    assert_eq!(
        owner.read_with(cx, |state, _| state.sessions[&key].history().revision),
        revision
    );
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn live_messages_promote_a_model_only_draft_to_content(cx: &mut TestAppContext) {
    let (_dir, owner, key) = prepare(cx, &[]);
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
    })
    .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    control(&client, "emit_newer", "", 0).await;
    cx.condition(&owner, |state, _| !state.sessions[&key].live.is_empty())
        .await;
    owner.read_with(cx, |state, _| {
        assert_eq!(state.sessions[&key].body_state(), BodyState::Ready)
    });
    owner.update(cx, |state, cx| state.refresh(&key, cx));
    cx.condition(&owner, |state, _| !state.sessions[&key].core_read.running())
        .await;
    // A non-streaming snapshot cannot end a run before Pi emits agent_settled.
    assert!(owner.read_with(cx, |state, _| state.sessions[&key].running()));
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn model_refresh_reconnects_even_when_a_cached_snapshot_survives(cx: &mut TestAppContext) {
    let (dir, owner, key) = prepare(cx, &[]);
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
    })
    .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    assert!(
        client
            .request_raw(serde_json::json!({"type":"disconnect"}))
            .await
            .is_err()
    );
    cx.condition(&owner, |state, _| state.sessions[&key].instance.is_none())
        .await;
    owner.update(cx, |state, cx| {
        assert!(state.sessions[&key].state.is_some());
        state.refresh_models(&key, cx);
        state.refresh_models(&key, cx);
    });
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
    })
    .await;
    assert_eq!(count(dir.path(), "get_available_models"), 2);
    assert_eq!(count(dir.path(), "get_entries"), 0);
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn compaction_events_refresh_history_and_release_busy_state(cx: &mut TestAppContext) {
    let (_dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, _| state.sessions[&key].state.is_some())
        .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    control(&client, "compact_start", "", 0).await;
    cx.condition(&owner, |state, _| state.sessions[&key].compacting)
        .await;
    control(&client, "compact_end", "", 0).await;
    cx.condition(&owner, |state, _| {
        state.sessions[&key].history().entry("compact").is_some()
    })
    .await;
    owner.read_with(cx, |state, _| {
        let session = &state.sessions[&key];
        assert!(!session.busy());
        assert_eq!(
            session
                .messages(None)
                .iter()
                .map(|message| message.role().to_owned())
                .collect::<Vec<_>>(),
            ["user", "compaction"]
        );
    });
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn uncertain_model_settings_block_send_until_readback_succeeds(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
    })
    .await;
    std::fs::write(dir.path().join("fail-get_state"), "").unwrap();
    owner.update(cx, |state, cx| {
        state.set_model(&key, state.sessions[&key].model_options()[1].clone(), cx);
    });
    cx.condition(&owner, |state, _| {
        matches!(
            state.sessions[&key].model_change,
            ModelChange::Unconfirmed(_)
        )
    })
    .await;
    owner.update(cx, |state, cx| {
        state.set_draft(&key, "wait until confirmed".into(), cx);
        state.send(&key, StreamingBehavior::Steer, cx);
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    std::fs::remove_file(dir.path().join("fail-get_state")).unwrap();
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        matches!(state.sessions[&key].model_change, ModelChange::Idle)
    })
    .await;
    owner.update(cx, |state, cx| {
        assert_eq!(
            state.sessions[&key].model_identity(),
            Some(("fixture".into(), "beta".into()))
        );
        state.send(&key, StreamingBehavior::Steer, cx);
    });
    cx.condition(&owner, |state, _| state.sessions[&key].draft.is_empty())
        .await;
    assert_eq!(count(dir.path(), "set_model"), 1);
    assert_eq!(count(dir.path(), "prompt"), 1);
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn commands_own_their_tasks_and_fork_editor_until_completion(cx: &mut TestAppContext) {
    use super::super::command::SessionCommand;
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].fork_options().is_empty()
    })
    .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    std::fs::write(dir.path().join("hold-set_session_name"), "").unwrap();
    owner.update(cx, |state, cx| {
        state.rename(&key, "first".into(), cx);
        state.rename(&key, "duplicate".into(), cx);
    });
    control(&client, "wait_for", "set_session_name", 1).await;
    assert_eq!(count(dir.path(), "set_session_name"), 1);
    assert!(owner.read_with(cx, |state, _| matches!(
        state.sessions[&key].command,
        SessionCommand::Renaming { .. }
    )));
    control(&client, "release", "set_session_name", 0).await;
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].command.running()
            && !state.sessions[&key].core_read.running()
            && !state.sessions[&key].fork_options().is_empty()
    })
    .await;
    std::fs::write(dir.path().join("hold-get_entries"), "").unwrap();
    let reads = count(dir.path(), "get_entries");
    owner.update(cx, |state, cx| state.fork(&key, "old".into(), cx));
    control(&client, "wait_for", "get_entries", reads + 1).await;
    cx.condition(&owner, |state, _| matches!(&state.sessions[&key].command, SessionCommand::Forking { editor: Some(editor), .. } if editor == "extension fork draft")).await;
    assert!(owner.read_with(cx, |state, _| state.sessions[&key].draft.is_empty()));
    control(&client, "release", "get_entries", 0).await;
    cx.condition(&owner, |state, _| state.selected.as_ref() != Some(&key))
        .await;
    owner.read_with(cx, |state, _| {
        assert_eq!(state.current().unwrap().draft, "extension fork draft");
        assert!(!state.sessions[&key].command.running());
        assert!(!state.current().unwrap().command.running());
    });
    close(&owner, cx).await;
}
async fn control(client: &Client, kind: &str, command: &str, count: usize) {
    client
        .request_raw(serde_json::json!({"type":kind, "command":command, "count":count.to_string()}))
        .await
        .unwrap();
}
fn count(dir: &std::path::Path, command: &str) -> usize {
    std::fs::read_to_string(dir.join("commands.log"))
        .unwrap()
        .lines()
        .filter(|line| *line == command)
        .count()
}
async fn close(owner: &Entity<ConversationState>, cx: &mut TestAppContext) {
    owner.update(cx, |state, _| {
        state.draining = true;
        state.catalog.transition(CatalogMessage::Cancel);
        for s in state.sessions.values_mut() {
            s.reset_reads();
        }
    });
    let pi = cx.update(|cx| pi::global(cx));
    pi.update(cx, |pi, cx| pi.close_all(cx)).await;
}

#[gpui_kit::test]
async fn auxiliary_reads_do_not_block_core_or_send_and_retry_stays_local(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(
        cx,
        &[
            "hold-get_available_models",
            "fail-get_session_stats",
            "fail-get_fork_messages",
        ],
    );
    cx.condition(&owner, |state, _| {
        state.sessions[&key].state.is_some()
            && state.sessions[&key].stats.error().is_some()
            && state.sessions[&key].fork_messages.error().is_some()
    })
    .await;
    owner.update(cx, |state, cx| {
        let s = &state.sessions[&key];
        assert!(s.models.running());
        assert_eq!(s.history().leaf.as_deref(), Some("old"));
        assert_eq!(s.activity(), Activity::Idle);
        assert!(s.error.is_none());
        state.set_draft(&key, "can send without model list".into(), cx);
        state.send(&key, StreamingBehavior::Steer, cx);
    });
    cx.condition(&owner, |state, _| {
        state.sessions[&key].draft.is_empty() && !state.sessions[&key].core_read.running()
    })
    .await;
    assert_eq!(count(dir.path(), "prompt"), 1);
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    control(&client, "release", "get_available_models", 0).await;
    cx.condition(&owner, |state, _| !state.sessions[&key].models.running())
        .await;
    let entries_before = count(dir.path(), "get_entries");
    std::fs::remove_file(dir.path().join("fail-get_session_stats")).unwrap();
    owner.update(cx, |state, cx| state.refresh_stats(&key, cx));
    cx.condition(&owner, |state, _| !state.sessions[&key].stats.running())
        .await;
    assert!(owner.read_with(cx, |state, _| state.sessions[&key].stats.data().is_some()));
    owner.update(cx, |state, cx| state.refresh_models(&key, cx));
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].models.running() && !state.sessions[&key].thinking_levels.running()
    })
    .await;
    assert_eq!(count(dir.path(), "get_entries"), entries_before);
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn model_change_waits_for_readback_and_failed_command_reads_actual_value(
    cx: &mut TestAppContext,
) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, _| {
        state.sessions[&key].models.data().is_some()
            && state.sessions[&key].thinking_levels.data().is_some()
    })
    .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    std::fs::write(dir.path().join("hold-get_available_thinking_levels"), "").unwrap();
    std::fs::write(dir.path().join("hold-get_state"), "").unwrap();
    let state_reads = count(dir.path(), "get_state");
    std::fs::write(dir.path().join("fail-set_model"), "").unwrap();
    owner.update(cx, |state, cx| {
        let model = state.sessions[&key].model_options()[1].clone();
        state.set_model(&key, model, cx);
    });
    control(&client, "wait_for", "get_state", state_reads + 1).await;
    owner.update(cx, |state, cx| {
        assert!(matches!(
            state.sessions[&key].model_change,
            ModelChange::Reconciling { .. }
        ));
        state.set_draft(&key, "wait for confirmation".into(), cx);
        state.send(&key, StreamingBehavior::Steer, cx);
        state.set_thinking(&key, "off".into(), cx);
        state.refresh(&key, cx); // Coalesced until the model command settles.
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    assert_eq!(count(dir.path(), "set_thinking_level"), 0);
    control(&client, "release", "get_state", 0).await;
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].model_change.running() && !state.sessions[&key].core_read.running()
    })
    .await;
    owner.read_with(cx, |state, _| {
        let s = &state.sessions[&key];
        assert_eq!(s.model_identity(), Some(("fixture".into(), "beta".into())));
        assert!(s.model_change.error().is_some());
        assert!(!s.model_change.unconfirmed());
        assert_eq!(s.activity(), Activity::Idle);
        assert!(s.thinking_levels.running());
        assert_eq!(s.draft, "wait for confirmation");
    });
    assert_eq!(count(dir.path(), "set_model"), 1);
    assert!(count(dir.path(), "get_entries") >= 2);
    owner.update(cx, |state, cx| {
        state.send(&key, StreamingBehavior::Steer, cx)
    });
    cx.condition(&owner, |state, _| state.sessions[&key].draft.is_empty())
        .await;
    assert_eq!(count(dir.path(), "prompt"), 1);
    control(&client, "release", "get_available_thinking_levels", 0).await;
    cx.condition(&owner, |state, _| {
        !state.sessions[&key].thinking_levels.running()
    })
    .await;
    assert!(owner.read_with(cx, |state, _| {
        state.sessions[&key].levels().contains(&"high".into())
    }));
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn first_send_survives_connection_start_and_does_not_wait_for_models(
    cx: &mut TestAppContext,
) {
    let (dir, owner, key) =
        begin_with_prompt(cx, &["hold-get_available_models"], Some("first message"));
    cx.condition(&owner, |state, _| {
        state.sessions[&key].state.is_some() && state.sessions[&key].draft.is_empty()
    })
    .await;
    assert_eq!(count(dir.path(), "prompt"), 1);
    assert!(owner.read_with(cx, |state, _| state.sessions[&key].models.running()));
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn snapshot_cannot_replace_events_delivered_during_its_read(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, _| state.sessions[&key].state.is_some())
        .await;
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    std::fs::write(dir.path().join("hold-get_entries"), "").unwrap();
    owner.update(cx, |state, cx| state.refresh(&key, cx));
    control(&client, "wait_for", "get_entries", 2).await;
    control(&client, "emit_newer", "", 0).await;
    cx.condition(&owner, |state, _| {
        state.sessions[&key].running() && !state.sessions[&key].live.is_empty()
    })
    .await;
    let revision = owner.read_with(cx, |state, _| state.sessions[&key].history().revision);
    control(&client, "release", "get_entries", 0).await;
    cx.condition(&owner, |state, _| !state.sessions[&key].core_read.running())
        .await;
    owner.read_with(cx, |state, _| {
        let s = &state.sessions[&key];
        assert!(s.running());
        assert_eq!(s.history().revision, revision);
        assert_eq!(s.live.len(), 1);
    });
    control(&client, "settle", "", 0).await;
    cx.condition(&owner, |state, _| {
        state.sessions[&key].history().leaf.as_deref() == Some("new")
            && !state.sessions[&key].running()
    })
    .await;
    close(&owner, cx).await;
}
