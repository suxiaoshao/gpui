use ai_chat_core::{
    PromptId, ProviderId, ProviderModelId, RunSettingsSnapshot, ShortcutAction, ShortcutId,
    ShortcutInputSource,
};
use ai_chat_db::{DbError, NewShortcut, ShortcutRecord, UpdateShortcut};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};
use tracing::{Level, event};

use crate::{database, state};

#[derive(Clone)]
pub(crate) struct ShortcutCatalogGlobal(Entity<ShortcutCatalogStore>);

impl ShortcutCatalogGlobal {
    pub(crate) fn entity(&self) -> Entity<ShortcutCatalogStore> {
        self.0.clone()
    }
}

impl Global for ShortcutCatalogGlobal {}

pub(crate) struct ShortcutCatalogStore {
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutCatalogEvent {
    Changed(ShortcutCatalogChange),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutCatalogChange {
    Created {
        shortcut_id: ShortcutId,
    },
    Updated {
        shortcut_id: ShortcutId,
    },
    Deleted {
        shortcut_id: ShortcutId,
    },
    EnabledChanged {
        shortcut_id: ShortcutId,
        enabled: bool,
    },
    Reregistered {
        shortcut_id: ShortcutId,
    },
}

impl EventEmitter<ShortcutCatalogEvent> for ShortcutCatalogStore {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutDraft {
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<PromptId>,
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
    pub(crate) input_source: ShortcutInputSource,
}

impl ShortcutCatalogStore {
    fn new() -> Self {
        Self { revision: 0 }
    }

    pub(crate) fn create_shortcut(
        &mut self,
        draft: ShortcutDraft,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ShortcutRecord> {
        let repository = database::repository(cx);
        let settings_snapshot = settings_snapshot_for_draft(&draft, cx)?;
        let shortcut = repository.insert_shortcut(NewShortcut {
            hotkey: draft.hotkey,
            enabled: draft.enabled,
            prompt_id: draft.prompt_id,
            provider_id: Some(draft.provider_id),
            model_id: Some(draft.model_id),
            input_source: draft.input_source,
            action: ShortcutAction::OpenTemporaryConversation,
            settings_snapshot,
        })?;
        state::GlobalHotkeyState::sync_shortcut_registration(None, Some(&shortcut), cx);
        self.emit_changed(
            ShortcutCatalogChange::Created {
                shortcut_id: shortcut.id.clone(),
            },
            cx,
        );
        Ok(shortcut)
    }

    pub(crate) fn update_shortcut(
        &mut self,
        id: &ShortcutId,
        draft: ShortcutDraft,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ShortcutRecord> {
        let repository = database::repository(cx);
        let previous = repository
            .get_shortcut(id)?
            .ok_or_else(|| DbError::Invariant(format!("shortcut {id} is missing")))?;
        let settings_snapshot = settings_snapshot_for_draft(&draft, cx)?;
        let shortcut = repository.update_shortcut(
            id,
            UpdateShortcut {
                hotkey: draft.hotkey,
                enabled: draft.enabled,
                prompt_id: draft.prompt_id,
                provider_id: Some(draft.provider_id),
                model_id: Some(draft.model_id),
                input_source: draft.input_source,
                action: ShortcutAction::OpenTemporaryConversation,
                settings_snapshot,
            },
        )?;
        state::GlobalHotkeyState::sync_shortcut_registration(Some(&previous), Some(&shortcut), cx);
        self.emit_changed(
            ShortcutCatalogChange::Updated {
                shortcut_id: shortcut.id.clone(),
            },
            cx,
        );
        Ok(shortcut)
    }

    pub(crate) fn delete_shortcut(
        &mut self,
        id: &ShortcutId,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<usize> {
        let repository = database::repository(cx);
        let previous = repository.get_shortcut(id)?;
        let deleted = repository.delete_shortcut(id)?;
        if deleted > 0 {
            state::GlobalHotkeyState::sync_shortcut_registration(previous.as_ref(), None, cx);
            self.emit_changed(
                ShortcutCatalogChange::Deleted {
                    shortcut_id: id.clone(),
                },
                cx,
            );
        }
        Ok(deleted)
    }

    pub(crate) fn set_shortcut_enabled(
        &mut self,
        id: &ShortcutId,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ShortcutRecord> {
        let repository = database::repository(cx);
        let previous = repository
            .get_shortcut(id)?
            .ok_or_else(|| DbError::Invariant(format!("shortcut {id} is missing")))?;
        let shortcut = repository.set_shortcut_enabled(id, enabled)?;
        state::GlobalHotkeyState::sync_shortcut_registration(Some(&previous), Some(&shortcut), cx);
        self.emit_changed(
            ShortcutCatalogChange::EnabledChanged {
                shortcut_id: shortcut.id.clone(),
                enabled: shortcut.enabled,
            },
            cx,
        );
        Ok(shortcut)
    }

    pub(crate) fn reregister_shortcut(
        &mut self,
        id: &ShortcutId,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ShortcutRecord> {
        let shortcut = database::repository(cx)
            .get_shortcut(id)?
            .ok_or_else(|| DbError::Invariant(format!("shortcut {id} is missing")))?;
        state::GlobalHotkeyState::sync_shortcut_registration(Some(&shortcut), Some(&shortcut), cx);
        self.emit_changed(
            ShortcutCatalogChange::Reregistered {
                shortcut_id: shortcut.id.clone(),
            },
            cx,
        );
        Ok(shortcut)
    }

    fn emit_changed(&mut self, change: ShortcutCatalogChange, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.emit(ShortcutCatalogEvent::Changed(change));
        cx.notify();
    }
}

pub(crate) fn init(cx: &mut App) {
    let store = cx.new(|_| ShortcutCatalogStore::new());
    cx.set_global(ShortcutCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> Entity<ShortcutCatalogStore> {
    cx.global::<ShortcutCatalogGlobal>().entity()
}

pub(crate) fn list_shortcuts(cx: &App) -> ai_chat_db::Result<Vec<ShortcutRecord>> {
    database::repository(cx).list_shortcuts()
}

pub(crate) fn create_shortcut(
    cx: &mut App,
    draft: ShortcutDraft,
) -> ai_chat_db::Result<ShortcutRecord> {
    catalog(cx).update(cx, |catalog, cx| catalog.create_shortcut(draft, cx))
}

pub(crate) fn update_shortcut(
    cx: &mut App,
    id: &ShortcutId,
    draft: ShortcutDraft,
) -> ai_chat_db::Result<ShortcutRecord> {
    catalog(cx).update(cx, |catalog, cx| catalog.update_shortcut(id, draft, cx))
}

pub(crate) fn delete_shortcut(cx: &mut App, id: &ShortcutId) -> ai_chat_db::Result<usize> {
    catalog(cx).update(cx, |catalog, cx| catalog.delete_shortcut(id, cx))
}

pub(crate) fn set_shortcut_enabled(
    cx: &mut App,
    id: &ShortcutId,
    enabled: bool,
) -> ai_chat_db::Result<ShortcutRecord> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.set_shortcut_enabled(id, enabled, cx)
    })
}

pub(crate) fn reregister_shortcut(
    cx: &mut App,
    id: &ShortcutId,
) -> ai_chat_db::Result<ShortcutRecord> {
    catalog(cx).update(cx, |catalog, cx| catalog.reregister_shortcut(id, cx))
}

fn settings_snapshot_for_draft(
    draft: &ShortcutDraft,
    cx: &App,
) -> ai_chat_db::Result<RunSettingsSnapshot> {
    let repository = database::repository(cx);
    let prompt = match &draft.prompt_id {
        Some(prompt_id) => {
            let prompt = repository
                .get_prompt(prompt_id)?
                .ok_or_else(|| DbError::Invariant(format!("prompt {prompt_id} is missing")))?;
            Some(prompt.content)
        }
        None => None,
    };
    let provider = repository
        .get_provider(&draft.provider_id)?
        .ok_or_else(|| DbError::Invariant(format!("provider {} is missing", draft.provider_id)))?;
    if !provider.enabled {
        return Err(DbError::Invariant(format!(
            "provider {} is disabled",
            draft.provider_id
        )));
    }
    let model = repository
        .get_provider_model(&draft.provider_id, &draft.model_id)?
        .ok_or_else(|| {
            DbError::Invariant(format!(
                "model {}/{} is missing",
                draft.provider_id, draft.model_id
            ))
        })?;
    if !model.enabled {
        return Err(DbError::Invariant(format!(
            "model {}/{} is disabled",
            draft.provider_id, draft.model_id
        )));
    }

    Ok(RunSettingsSnapshot {
        prompt,
        provider_id: draft.provider_id.clone(),
        model_id: draft.model_id.clone(),
        model_capabilities: model.capabilities,
        provider_settings: provider.settings,
        reasoning_selection: None,
        tool_policy: state::conversations::default_tool_policy(),
    })
}

pub(crate) fn log_shortcut_runtime_sync_error(shortcut_id: &str, err: impl ToString) {
    event!(
        Level::ERROR,
        shortcut_id,
        error = %err.to_string(),
        "failed to sync ai-chat2 shortcut runtime registration"
    );
}
