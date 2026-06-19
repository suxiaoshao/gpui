use ai_chat_core::{PromptContent, PromptId};
use ai_chat_db::{DbError, FreshRepository, NewPrompt, PromptRecord, UpdatePrompt};
use gpui::{App, AppContext};
use gpui_store::{SharedStore, StoreBackend, StoreBackendFuture, StoreBackendId, StoreState};

use crate::database;

const DEFAULT_SORT_ORDER_STEP: i32 = 10;

pub(crate) type PromptCatalogStore = SharedStore<PromptCatalogState, PromptCatalogBackend>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PromptCatalogState {
    prompts: Vec<PromptRecord>,
}

impl PromptCatalogState {
    pub(crate) fn prompts(&self) -> &[PromptRecord] {
        &self.prompts
    }

    pub(crate) fn prompt_records(&self) -> &Vec<PromptRecord> {
        &self.prompts
    }
}

impl StoreState for PromptCatalogState {}

#[derive(Clone)]
pub(crate) struct PromptCatalogBackend {
    repository: FreshRepository,
}

impl PromptCatalogBackend {
    fn new(repository: FreshRepository) -> Self {
        Self { repository }
    }
}

impl StoreBackend<PromptCatalogState> for PromptCatalogBackend {
    type Snapshot = Vec<PromptRecord>;
    type Event = ();
    type Subscription = ();
    type Error = DbError;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new("database:prompts")
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(Some(self.repository.list_prompts()?))
    }

    fn reconcile(&self, state: &mut PromptCatalogState, snapshot: Self::Snapshot) -> bool {
        if state.prompts == snapshot {
            return false;
        }

        state.prompts = snapshot;
        true
    }
}

pub(crate) fn init(cx: &mut App) -> ai_chat_db::Result<()> {
    let backend = PromptCatalogBackend::new(database::repository(cx));
    PromptCatalogStore::install_global_with_backend(cx, PromptCatalogState::default(), backend)?;
    Ok(())
}

pub(crate) fn catalog(cx: &impl AppContext) -> PromptCatalogStore {
    PromptCatalogStore::global(cx)
}

pub(crate) fn list_prompts(cx: &App) -> ai_chat_db::Result<Vec<PromptRecord>> {
    Ok(catalog(cx).read_cloned(cx, |state| &state.prompts))
}

pub(crate) fn create_prompt(
    cx: &mut App,
    name: String,
    text: String,
) -> ai_chat_db::Result<PromptRecord> {
    let repository = database::repository(cx);
    let sort_order = catalog(cx)
        .read(cx, |state| {
            state
                .prompts
                .last()
                .map(|prompt| prompt.sort_order + DEFAULT_SORT_ORDER_STEP)
        })
        .unwrap_or(DEFAULT_SORT_ORDER_STEP);
    let prompt = repository.insert_prompt(NewPrompt {
        name,
        content: PromptContent { text },
        enabled: true,
        sort_order,
    })?;
    sync_from_repository(cx)?;
    Ok(prompt)
}

pub(crate) fn update_prompt(
    cx: &mut App,
    id: &PromptId,
    name: String,
    text: String,
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
    sync_from_repository(cx)?;
    Ok(prompt)
}

pub(crate) fn delete_prompt(cx: &mut App, id: &PromptId) -> ai_chat_db::Result<usize> {
    let deleted = database::repository(cx).delete_prompt(id)?;
    if deleted > 0 {
        sync_from_repository(cx)?;
    }
    Ok(deleted)
}

fn sync_from_repository(cx: &mut App) -> ai_chat_db::Result<()> {
    catalog(cx).refresh_from_backend(cx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create_prompt, delete_prompt, init, list_prompts, update_prompt};
    use crate::database::FreshStoreGlobal;
    use gpui::TestAppContext;

    #[gpui::test]
    fn prompt_catalog_tracks_committed_database_rows(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");

        cx.update(|cx| {
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            init(cx).expect("init prompt catalog");

            assert!(list_prompts(cx).expect("list initial prompts").is_empty());

            let prompt = create_prompt(
                cx,
                "Write release notes".to_string(),
                "Summarize changes".to_string(),
            )
            .expect("create prompt");
            assert_eq!(
                list_prompts(cx)
                    .expect("list created prompts")
                    .iter()
                    .map(|prompt| prompt.id.as_str())
                    .collect::<Vec<_>>(),
                vec![prompt.id.as_str()]
            );

            let updated = update_prompt(
                cx,
                &prompt.id,
                "Write changelog".to_string(),
                "Summarize every change".to_string(),
            )
            .expect("update prompt");
            assert_eq!(
                list_prompts(cx)
                    .expect("list updated prompts")
                    .first()
                    .map(|prompt| (prompt.name.as_str(), prompt.content.text.as_str())),
                Some((updated.name.as_str(), updated.content.text.as_str()))
            );

            assert_eq!(delete_prompt(cx, &prompt.id).expect("delete prompt"), 1);
            assert!(list_prompts(cx).expect("list deleted prompts").is_empty());
        });
    }
}
