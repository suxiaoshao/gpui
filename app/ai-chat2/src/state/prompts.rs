use ai_chat_core::{PromptContent, PromptId};
use ai_chat_db::{DbError, NewPrompt, PromptRecord, UpdatePrompt};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};

use crate::database;

const DEFAULT_SORT_ORDER_STEP: i32 = 10;

#[derive(Clone)]
pub(crate) struct PromptCatalogGlobal(Entity<PromptCatalogStore>);

impl PromptCatalogGlobal {
    pub(crate) fn entity(&self) -> Entity<PromptCatalogStore> {
        self.0.clone()
    }
}

impl Global for PromptCatalogGlobal {}

pub(crate) struct PromptCatalogStore {
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PromptCatalogEvent {
    Changed(PromptCatalogChange),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PromptCatalogChange {
    Created { prompt_id: PromptId },
    Updated { prompt_id: PromptId },
    Deleted { prompt_id: PromptId },
}

impl EventEmitter<PromptCatalogEvent> for PromptCatalogStore {}

impl PromptCatalogStore {
    fn new() -> Self {
        Self { revision: 0 }
    }

    pub(crate) fn create_prompt(
        &mut self,
        name: String,
        text: String,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<PromptRecord> {
        let repository = database::repository(cx);
        let sort_order = repository
            .list_prompts()?
            .last()
            .map(|prompt| prompt.sort_order + DEFAULT_SORT_ORDER_STEP)
            .unwrap_or(DEFAULT_SORT_ORDER_STEP);
        let prompt = repository.insert_prompt(NewPrompt {
            name,
            content: PromptContent { text },
            enabled: true,
            sort_order,
        })?;
        self.emit_changed(
            PromptCatalogChange::Created {
                prompt_id: prompt.id.clone(),
            },
            cx,
        );
        Ok(prompt)
    }

    pub(crate) fn update_prompt(
        &mut self,
        id: &PromptId,
        name: String,
        text: String,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<PromptRecord> {
        let repository = database::repository(cx);
        let current = repository
            .get_prompt(id)?
            .ok_or_else(|| DbError::Invariant(format!("prompt {id} is missing")))?;
        let prompt = repository.update_prompt(
            id,
            UpdatePrompt {
                name,
                content: PromptContent { text },
                enabled: current.enabled,
                sort_order: current.sort_order,
            },
        )?;
        self.emit_changed(
            PromptCatalogChange::Updated {
                prompt_id: prompt.id.clone(),
            },
            cx,
        );
        Ok(prompt)
    }

    pub(crate) fn delete_prompt(
        &mut self,
        id: &PromptId,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<usize> {
        let deleted = database::repository(cx).delete_prompt(id)?;
        if deleted > 0 {
            self.emit_changed(
                PromptCatalogChange::Deleted {
                    prompt_id: id.clone(),
                },
                cx,
            );
        }
        Ok(deleted)
    }

    fn emit_changed(&mut self, change: PromptCatalogChange, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.emit(PromptCatalogEvent::Changed(change));
        cx.notify();
    }
}

pub(crate) fn init(cx: &mut App) {
    let store = cx.new(|_| PromptCatalogStore::new());
    cx.set_global(PromptCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> Entity<PromptCatalogStore> {
    cx.global::<PromptCatalogGlobal>().entity()
}

pub(crate) fn list_prompts(cx: &App) -> ai_chat_db::Result<Vec<PromptRecord>> {
    database::repository(cx).list_prompts()
}

pub(crate) fn create_prompt(
    cx: &mut App,
    name: String,
    text: String,
) -> ai_chat_db::Result<PromptRecord> {
    catalog(cx).update(cx, |catalog, cx| catalog.create_prompt(name, text, cx))
}

pub(crate) fn update_prompt(
    cx: &mut App,
    id: &PromptId,
    name: String,
    text: String,
) -> ai_chat_db::Result<PromptRecord> {
    catalog(cx).update(cx, |catalog, cx| catalog.update_prompt(id, name, text, cx))
}

pub(crate) fn delete_prompt(cx: &mut App, id: &PromptId) -> ai_chat_db::Result<usize> {
    catalog(cx).update(cx, |catalog, cx| catalog.delete_prompt(id, cx))
}
