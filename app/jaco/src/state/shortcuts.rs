use std::fmt;

use gpui::{App, Task};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::Store;
use jaco_core::{
    PromptId, ProviderId, ProviderModelId, ReasoningSelectionSnapshot, RunSettingsSnapshot,
    ShortcutAction, ShortcutId, ShortcutInputSource, ToolApprovalMode,
};
use jaco_db::{DbError, NewShortcut, ShortcutRecord, UpdateShortcut};
use tokio::sync::oneshot;
use tracing::{Level, event};

use crate::{
    components::run_settings::reasoning_selection_is_valid,
    database,
    state::{self, session::CatalogMutation},
};

pub(crate) type ShortcutOperation = refresh::Operation<ShortcutData, ShortcutProblem, Task<()>>;
pub(crate) type ShortcutStore = Store<ShortcutOperation>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShortcutData {
    shortcuts: Vec<ShortcutRecord>,
}

impl ShortcutData {
    pub(crate) fn shortcuts(&self) -> &[ShortcutRecord] {
        &self.shortcuts
    }

    fn upsert(&mut self, shortcut: ShortcutRecord) {
        match self
            .shortcuts
            .iter_mut()
            .find(|current| current.id == shortcut.id)
        {
            Some(current) => *current = shortcut,
            None => self.shortcuts.push(shortcut),
        }
        sort_shortcuts(&mut self.shortcuts);
    }

    fn remove(&mut self, shortcut_id: &ShortcutId) {
        self.shortcuts
            .retain(|shortcut| &shortcut.id != shortcut_id);
    }
}

#[derive(Debug)]
pub(crate) struct ShortcutProblem(DbError);

impl fmt::Display for ShortcutProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ShortcutProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub(crate) enum ShortcutMessage {
    Upsert(Box<ShortcutRecord>),
    Remove(ShortcutId),
}

impl Transition<ShortcutMessage> for &mut ShortcutData {
    type Output = ();

    fn transition(self, message: ShortcutMessage) {
        match message {
            ShortcutMessage::Upsert(shortcut) => self.upsert(*shortcut),
            ShortcutMessage::Remove(shortcut_id) => self.remove(&shortcut_id),
        }
    }
}

fn sort_shortcuts(shortcuts: &mut [ShortcutRecord]) {
    shortcuts.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShortcutDraft {
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
    pub(crate) prompt_id: Option<PromptId>,
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
    pub(crate) input_source: ShortcutInputSource,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
}

fn apply(message: ShortcutMessage, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        let ShortcutOperation::Ready(ready) = operation else {
            panic!("shortcut commit requires an exact Ready operation");
        };
        ready.transition(message);
    });
}

fn ensure_ready(cx: &App) -> jaco_db::Result<()> {
    catalog(cx).read(cx, |operation| {
        matches!(operation, ShortcutOperation::Ready(_))
            .then_some(())
            .ok_or_else(|| DbError::Invariant("shortcut resource is not ready".to_string()))
    })
}

pub(crate) fn init(cx: &mut App) {
    ShortcutStore::install_global(cx, ShortcutOperation::new());
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_shortcuts().map(|mut shortcuts| {
                    sort_shortcuts(&mut shortcuts);
                    ShortcutData { shortcuts }
                })
            })
            .await
            .map_err(ShortcutProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if matches!(operation, ShortcutOperation::Loading(_)) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| operation.transition(Load(task)));
}

pub(crate) fn catalog(cx: &impl gpui::AppContext) -> ShortcutStore {
    ShortcutStore::global(cx)
}

pub(crate) fn request_refresh(cx: &mut App) {
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    if catalog(cx).read(cx, ShortcutOperation::is_running) {
        return;
    }
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_shortcuts().map(|mut shortcuts| {
                    sort_shortcuts(&mut shortcuts);
                    ShortcutData { shortcuts }
                })
            })
            .await
            .map_err(ShortcutProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if operation.is_running() {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| match operation {
        ShortcutOperation::Idle(_) => operation.transition(Load(task)),
        ShortcutOperation::Ready(_) | ShortcutOperation::Degraded(_) => {
            operation.transition(Refresh(task))
        }
        ShortcutOperation::Unavailable(_) => operation.transition(Retry(task)),
        ShortcutOperation::Loading(_)
        | ShortcutOperation::Refreshing(_)
        | ShortcutOperation::Retrying(_)
        | ShortcutOperation::RefreshingDegraded(_) => {}
    });
}

pub(crate) fn list_shortcuts(cx: &App) -> jaco_db::Result<Vec<ShortcutRecord>> {
    catalog(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.shortcuts.clone())
            .ok_or_else(|| DbError::Invariant("shortcut resource is not ready".to_string()))
    })
}

pub(crate) fn create_shortcut(
    cx: &mut App,
    draft: ShortcutDraft,
) -> Task<jaco_db::Result<ShortcutRecord>> {
    let settings_snapshot = match settings_snapshot_for_draft(&draft, cx) {
        Ok(snapshot) => snapshot,
        Err(error) => return Task::ready(Err(error)),
    };
    spawn_shortcut_mutation(
        cx,
        move |repo| {
            repo.insert_shortcut(NewShortcut {
                hotkey: draft.hotkey,
                enabled: draft.enabled,
                prompt_id: draft.prompt_id,
                provider_id: Some(draft.provider_id),
                model_id: Some(draft.model_id),
                input_source: draft.input_source,
                action: ShortcutAction::OpenTemporaryConversation,
                settings_snapshot,
            })
        },
        |shortcut, cx| {
            apply(ShortcutMessage::Upsert(Box::new(shortcut.clone())), cx);
        },
    )
}

pub(crate) fn update_shortcut(
    cx: &mut App,
    id: ShortcutId,
    draft: ShortcutDraft,
) -> Task<jaco_db::Result<ShortcutRecord>> {
    let settings_snapshot = match settings_snapshot_for_draft(&draft, cx) {
        Ok(snapshot) => snapshot,
        Err(error) => return Task::ready(Err(error)),
    };
    let command_id = id.clone();
    let task = spawn_shortcut_mutation(
        cx,
        move |repo| {
            let previous = repo
                .get_shortcut(&command_id)?
                .ok_or_else(|| DbError::Invariant(format!("shortcut {command_id} is missing")))?;
            let shortcut = repo.update_shortcut(
                &command_id,
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
            Ok((previous, shortcut))
        },
        |(_previous, shortcut), cx| {
            apply(ShortcutMessage::Upsert(Box::new(shortcut.clone())), cx);
        },
    );
    cx.spawn(async move |_| task.await.map(|(_, shortcut)| shortcut))
}

pub(crate) fn delete_shortcut(cx: &mut App, id: ShortcutId) -> Task<jaco_db::Result<usize>> {
    let command_id = id.clone();
    let task = spawn_shortcut_mutation(
        cx,
        move |repo| {
            Ok((
                repo.get_shortcut(&command_id)?,
                repo.delete_shortcut(&command_id)?,
            ))
        },
        move |(_previous, deleted), cx| {
            if *deleted > 0 {
                apply(ShortcutMessage::Remove(id), cx);
            }
        },
    );
    cx.spawn(async move |_| task.await.map(|(_, deleted)| deleted))
}

pub(crate) fn set_shortcut_enabled(
    cx: &mut App,
    id: ShortcutId,
    enabled: bool,
) -> Task<jaco_db::Result<ShortcutRecord>> {
    let command_id = id.clone();
    let task = spawn_shortcut_mutation(
        cx,
        move |repo| {
            let previous = repo
                .get_shortcut(&command_id)?
                .ok_or_else(|| DbError::Invariant(format!("shortcut {command_id} is missing")))?;
            Ok((previous, repo.set_shortcut_enabled(&command_id, enabled)?))
        },
        |(_previous, shortcut), cx| {
            apply(ShortcutMessage::Upsert(Box::new(shortcut.clone())), cx);
        },
    );
    cx.spawn(async move |_| task.await.map(|(_, shortcut)| shortcut))
}

pub(crate) fn reregister_shortcut(cx: &mut App, id: ShortcutId) -> jaco_db::Result<ShortcutRecord> {
    let shortcut = catalog(cx).read(cx, |operation| match operation {
        ShortcutOperation::Ready(ready) => ready
            .data()
            .shortcuts()
            .iter()
            .find(|shortcut| shortcut.id == id)
            .cloned()
            .ok_or_else(|| DbError::Invariant(format!("shortcut {id} is missing"))),
        _ => Err(DbError::Invariant(
            "shortcut resource is not ready".to_string(),
        )),
    })?;
    state::GlobalHotkeyState::sync_shortcut_registration(Some(&shortcut), Some(&shortcut), cx);
    Ok(shortcut)
}

fn settings_snapshot_for_draft(
    draft: &ShortcutDraft,
    cx: &App,
) -> jaco_db::Result<RunSettingsSnapshot> {
    let prompt = state::prompts::catalog(cx).read(cx, |operation| match operation {
        state::prompts::PromptOperation::Ready(ready) => match &draft.prompt_id {
            Some(prompt_id) => ready
                .data()
                .prompts()
                .iter()
                .find(|prompt| &prompt.id == prompt_id)
                .map(|prompt| Some(prompt.content.clone()))
                .ok_or_else(|| DbError::Invariant(format!("prompt {prompt_id} is missing"))),
            None => Ok(None),
        },
        _ if draft.prompt_id.is_none() => Ok(None),
        _ => Err(DbError::Invariant(
            "prompt resource is not ready".to_string(),
        )),
    })?;
    state::providers::catalog(cx).read(cx, |operation| {
        let state::providers::ProviderOperation::Ready(ready) = operation else {
            return Err(DbError::Invariant(
                "provider resource is not ready".to_string(),
            ));
        };
        let (provider, models) = ready
            .data()
            .providers()
            .iter()
            .find(|(provider, _)| provider.id == draft.provider_id)
            .ok_or_else(|| {
                DbError::Invariant(format!("provider {} is missing", draft.provider_id))
            })?;
        if !provider.enabled {
            return Err(DbError::Invariant(format!(
                "provider {} is disabled",
                draft.provider_id
            )));
        }
        let model = models
            .iter()
            .find(|model| model.model_id == draft.model_id)
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
        if let Some(selection) = draft.reasoning_selection.as_ref()
            && !reasoning_selection_is_valid(model.capabilities.reasoning.as_ref(), selection)
        {
            return Err(DbError::Invariant(format!(
                "reasoning setting is not supported by model {}/{}",
                draft.provider_id, draft.model_id
            )));
        }

        Ok(RunSettingsSnapshot {
            prompt,
            provider_id: draft.provider_id.clone(),
            model_id: draft.model_id.clone(),
            model_capabilities: model.capabilities.clone(),
            provider_settings: provider.settings.clone(),
            reasoning_selection: draft.reasoning_selection.clone(),
            tool_policy: {
                let mut policy = state::conversations::default_tool_policy();
                policy.approval_mode = draft.approval_mode;
                policy
            },
        })
    })
}

fn spawn_shortcut_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&jaco_db::FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    publish: impl FnOnce(&R, &mut App) + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static,
{
    if let Err(error) = ensure_ready(cx) {
        return Task::ready(Err(error));
    }
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(DbError::Invariant(
            "shortcut mutation requires an exact Ready session".to_string(),
        )));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    cx.spawn(async move |cx| {
        let result = executor.mutate(CatalogMutation::Shortcut, command).await;
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
            Err(DbError::Invariant(
                "shortcut mutation driver ended without a result".to_string(),
            ))
        })
    })
}

pub(crate) fn log_shortcut_runtime_sync_error(shortcut_id: &str, err: impl ToString) {
    event!(
        Level::ERROR,
        shortcut_id,
        error = %err.to_string(),
        "failed to sync jaco shortcut runtime registration"
    );
}
