mod persistence;
mod tabs;
#[cfg(test)]
mod tests;

pub(crate) use self::persistence::ConversationDraft;
use self::{
    persistence::PersistedWorkspaceState,
    tabs::{AppTab, TabKind, TabPanel},
};

use crate::{
    database::Conversation,
    state::{ChatData, ChatDataInner},
    views::home::{ConversationPanelView, ConversationTabView},
};
use gpui::*;
use std::{collections::BTreeSet, ops::Deref};
use tracing::{Level, event};

pub(crate) struct WorkspaceState {
    persisted: PersistedWorkspaceState,
    tabs: Vec<AppTab>,
    active_tab: Option<TabKind>,
    save_task: Option<Task<()>>,
}

impl WorkspaceState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            persisted: Self::load_persisted().unwrap_or_default(),
            tabs: Vec::new(),
            active_tab: None,
            save_task: None,
        };
        this.restore_tabs(window, cx);
        this.sanitize_open_folders(cx);
        this
    }

    pub(crate) fn sidebar_width(&self) -> Pixels {
        px(self.persisted.sidebar_width.max(100.))
    }

    pub(crate) fn set_sidebar_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.persisted.sidebar_width = f32::from(width.max(px(100.)));
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn toggle_folder_open(&mut self, folder_id: i32, cx: &mut Context<Self>) {
        if !self.persisted.open_folder_ids.insert(folder_id) {
            self.persisted.open_folder_ids.remove(&folder_id);
        }
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn sanitize_open_folders(&mut self, cx: &mut Context<Self>) {
        let valid_ids = cx
            .global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .map(ChatDataInner::folder_ids)
            .unwrap_or_default();
        let before = self.persisted.open_folder_ids.len();
        self.persisted
            .open_folder_ids
            .retain(|folder_id| valid_ids.contains(folder_id));
        if self.persisted.open_folder_ids.len() != before {
            self.schedule_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn add_conversation_tab(
        &mut self,
        conversation_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match (
            self.tabs
                .iter()
                .any(|tab| tab.kind == TabKind::Conversation(conversation_id)),
            cx.global::<ChatData>()
                .read(cx)
                .as_ref()
                .ok()
                .and_then(|data| data.conversation(conversation_id)),
            self.draft_for(conversation_id),
        ) {
            (true, Some(_), _) => {
                self.active_tab = Some(TabKind::Conversation(conversation_id));
            }
            (false, Some(conversation), _) => {
                let conversation = conversation.clone();
                let conversation_id = conversation.id;
                self.push_conversation_tab(conversation, None, window, cx);
                self.active_tab = Some(TabKind::Conversation(conversation_id));
            }
            (false, None, _) => {}
            (true, None, _) => {
                self.remove_tab(conversation_id, cx);
                return;
            }
        }
        self.focus_active_conversation_panel(window, cx);
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn open_template_list_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .tabs
            .iter()
            .all(|tab| tab.kind != TabKind::TemplateList)
        {
            self.tabs.push(Self::template_tab(window, cx));
        }
        self.active_tab = Some(TabKind::TemplateList);
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn open_template_detail_tab(
        &mut self,
        template_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !Self::template_exists(template_id, cx) {
            return;
        }
        let kind = TabKind::TemplateDetail(template_id);
        if self.tabs.iter().all(|tab| tab.kind != kind) {
            self.tabs
                .push(Self::template_detail_tab(template_id, window, cx));
        }
        self.active_tab = Some(kind);
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn activate_tab(
        &mut self,
        tab_key: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_tab = self
            .tabs
            .iter()
            .find(|tab| tab.kind.key() == tab_key)
            .map(|tab| tab.kind);
        self.focus_active_conversation_panel(window, cx);
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    fn focus_active_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.active_conversation_panel() else {
            return;
        };
        panel.update(cx, |panel, cx| panel.focus_chat_form(window, cx));
    }

    pub(crate) fn remove_tab(&mut self, tab_key: i32, cx: &mut Context<Self>) {
        if let Some(removed_kind) = self
            .tabs
            .iter()
            .find(|tab| tab.kind.key() == tab_key)
            .map(|tab| tab.kind)
        {
            self.tabs.retain(|tab| tab.kind.key() != tab_key);
            if let TabKind::Conversation(id) = removed_kind {
                self.remove_draft(id);
            }
            if self.active_tab.is_some_and(|kind| kind.key() == tab_key) {
                self.active_tab = self.tabs.first().map(|tab| tab.kind);
            }
            self.sync_persisted_tabs();
            self.schedule_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn move_tab(&mut self, from_id: i32, to_id: Option<i32>, cx: &mut Context<Self>) {
        if to_id == Some(from_id) {
            return;
        }
        let Some(from_ix) = self.tabs.iter().position(|tab| tab.kind.key() == from_id) else {
            return;
        };
        let moved_kind = self.tabs[from_ix].kind;
        let item = self.tabs.remove(from_ix);
        let to_ix = to_id
            .and_then(|target_id| self.tabs.iter().position(|tab| tab.kind.key() == target_id));
        match to_ix {
            Some(target_ix) => self.tabs.insert(target_ix.min(self.tabs.len()), item),
            None => self.tabs.push(item),
        }
        self.active_tab = Some(moved_kind);
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn remove_conversation_tab(&mut self, conversation_id: i32, cx: &mut Context<Self>) {
        let existed = self
            .tabs
            .iter()
            .any(|tab| tab.kind == TabKind::Conversation(conversation_id));
        self.tabs
            .retain(|tab| tab.kind != TabKind::Conversation(conversation_id));
        self.remove_draft(conversation_id);
        if existed
            && !self
                .tabs
                .iter()
                .any(|tab| Some(tab.kind) == self.active_tab)
        {
            self.active_tab = self.tabs.first().map(|tab| tab.kind);
        }
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn sync_conversation_metadata(
        &mut self,
        conversation: &Conversation,
        cx: &mut Context<Self>,
    ) {
        let mut updated = false;
        for tab in &mut self.tabs {
            if tab.kind != tabs::TabKind::Conversation(conversation.id) {
                continue;
            }
            tab.icon = conversation.icon.clone().into();
            tab.name = conversation.title.clone().into();
            if let tabs::TabPanel::Conversation(panel) = &tab.panel {
                panel.update(cx, |panel, cx| panel.sync_metadata(conversation, cx));
            }
            updated = true;
        }
        if updated {
            cx.notify();
        }
    }

    pub(crate) fn remove_template_detail_tab(&mut self, template_id: i32, cx: &mut Context<Self>) {
        let existed = self
            .tabs
            .iter()
            .any(|tab| tab.kind == TabKind::TemplateDetail(template_id));
        self.tabs
            .retain(|tab| tab.kind != TabKind::TemplateDetail(template_id));
        if existed
            && !self
                .tabs
                .iter()
                .any(|tab| Some(tab.kind) == self.active_tab)
        {
            self.active_tab = self.tabs.first().map(|tab| tab.kind);
        }
        self.sync_persisted_tabs();
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn sync_conversation_chat_form_state(
        &mut self,
        conversation_id: i32,
        draft: Option<ConversationDraft>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .tabs
            .iter()
            .any(|tab| tab.kind == TabKind::Conversation(conversation_id))
        {
            return;
        }
        let preset_changed = if let Some(preset) = draft
            .as_ref()
            .and_then(ConversationDraft::to_latest_model_preset)
        {
            self.upsert_latest_model_preset(preset)
        } else {
            false
        };
        let draft_changed = self.upsert_persisted_conversation_draft(conversation_id, draft);

        if draft_changed || preset_changed {
            self.schedule_save(cx);
        }
    }

    pub(crate) fn tabs(&self) -> Vec<ConversationTabView> {
        self.tabs
            .iter()
            .map(|tab| ConversationTabView::new(tab.kind.key(), tab.icon.clone(), tab.name.clone()))
            .collect()
    }

    pub(crate) fn active_tab_key(&self) -> Option<i32> {
        self.active_tab.map(TabKind::key)
    }

    pub(crate) fn active_tab_title(&self) -> Option<SharedString> {
        self.tabs
            .iter()
            .find(|tab| Some(tab.kind) == self.active_tab)
            .map(|tab| tab.name.clone())
    }

    pub(crate) fn panel(&self) -> Option<AnyElement> {
        self.tabs.iter().find_map(|tab| {
            if Some(tab.kind) == self.active_tab {
                Some(tab.panel.clone().into_any_element())
            } else {
                None
            }
        })
    }

    pub(crate) fn active_conversation_panel(&self) -> Option<Entity<ConversationPanelView>> {
        self.tabs.iter().find_map(|tab| {
            if Some(tab.kind) != self.active_tab {
                return None;
            }
            match &tab.panel {
                TabPanel::Conversation(panel) => Some(panel.clone()),
                TabPanel::TemplateList(_) | TabPanel::TemplateDetail(_) => None,
            }
        })
    }

    pub(crate) fn open_folder_ids(&self) -> BTreeSet<i32> {
        self.persisted.open_folder_ids.clone()
    }
}

pub(crate) struct WorkspaceStore {
    data: Entity<WorkspaceState>,
}

impl Deref for WorkspaceStore {
    type Target = Entity<WorkspaceState>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Global for WorkspaceStore {}

pub(crate) fn init(window: &mut Window, cx: &mut Context<crate::views::home::HomeView>) {
    let data = cx.new(|cx| WorkspaceState::new(window, cx));
    cx.set_global(WorkspaceStore { data });
}

pub(crate) fn save_now(cx: &App) {
    if !cx.has_global::<WorkspaceStore>() {
        return;
    }
    if let Err(err) = cx.global::<WorkspaceStore>().read(cx).save_now() {
        event!(Level::ERROR, "save state.toml on quit failed: {}", err);
    }
}
