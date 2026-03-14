use crate::{
    APP_NAME,
    database::{ConversationTemplate, Db, Mode},
    errors::{AiChatError, AiChatResult},
    i18n::I18n,
    store::{ChatData, ChatDataInner},
    views::home::{
        ConversationPanelView, ConversationTabView, TemplateDetailView, TemplateListView,
    },
};
use gpui::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, io::ErrorKind, ops::Deref, path::PathBuf, time::Duration};
use tracing::{Level, event};

const STATE_FILE_NAME: &str = "state.toml";
const STATE_VERSION: u32 = 1;
const DEFAULT_SIDEBAR_WIDTH: f32 = 300.;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ConversationDraft {
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) provider_name: String,
    #[serde(default)]
    pub(crate) model_id: String,
    #[serde(default = "default_mode")]
    pub(crate) mode: Mode,
    #[serde(default)]
    pub(crate) selected_template_id: Option<i32>,
    #[serde(
        default = "default_request_template",
        serialize_with = "serialize_request_template",
        deserialize_with = "deserialize_request_template"
    )]
    pub(crate) request_template: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LatestModelPreset {
    #[serde(default)]
    provider_name: String,
    #[serde(default)]
    model_id: String,
    #[serde(
        default = "default_request_template",
        serialize_with = "serialize_request_template",
        deserialize_with = "deserialize_request_template"
    )]
    request_template: serde_json::Value,
}

impl ConversationDraft {
    fn to_latest_model_preset(&self) -> Option<LatestModelPreset> {
        if self.provider_name.is_empty() || self.model_id.is_empty() {
            return None;
        }

        Some(LatestModelPreset {
            provider_name: self.provider_name.clone(),
            model_id: self.model_id.clone(),
            request_template: self.request_template.clone(),
        })
    }

    fn from_latest_model_preset(preset: &LatestModelPreset) -> Self {
        Self {
            text: String::new(),
            provider_name: preset.provider_name.clone(),
            model_id: preset.model_id.clone(),
            mode: Mode::Contextual,
            selected_template_id: None,
            request_template: preset.request_template.clone(),
        }
    }
}

fn default_mode() -> Mode {
    Mode::Contextual
}

fn default_request_template() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

fn serialize_request_template<S>(
    value: &serde_json::Value,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn deserialize_request_template<'de, D>(deserializer: D) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StoredRequestTemplate {
        Json(String),
        Toml(toml::Value),
    }

    match StoredRequestTemplate::deserialize(deserializer)? {
        StoredRequestTemplate::Json(value) => {
            serde_json::from_str(&value).map_err(serde::de::Error::custom)
        }
        StoredRequestTemplate::Toml(value) => {
            serde_json::to_value(value).map_err(serde::de::Error::custom)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedTabKey {
    Conversation { id: i32 },
    TemplateList,
    TemplateDetail { id: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedTab {
    Conversation {
        id: i32,
        #[serde(default)]
        draft: Option<ConversationDraft>,
    },
    TemplateList,
    TemplateDetail {
        id: i32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistedWorkspaceState {
    #[serde(default = "default_state_version")]
    version: u32,
    #[serde(default = "default_sidebar_width")]
    sidebar_width: f32,
    #[serde(default)]
    open_folder_ids: BTreeSet<i32>,
    #[serde(default)]
    tabs: Vec<PersistedTab>,
    #[serde(default)]
    active_tab: Option<PersistedTabKey>,
    #[serde(default)]
    latest_model_preset: Option<LatestModelPreset>,
}

fn default_state_version() -> u32 {
    STATE_VERSION
}

fn default_sidebar_width() -> f32 {
    DEFAULT_SIDEBAR_WIDTH
}

impl Default for PersistedWorkspaceState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            open_folder_ids: Default::default(),
            tabs: Vec::new(),
            active_tab: None,
            latest_model_preset: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabKind {
    Conversation(i32),
    TemplateList,
    TemplateDetail(i32),
}

impl TabKind {
    fn key(self) -> i32 {
        match self {
            TabKind::Conversation(id) => id,
            TabKind::TemplateList => i32::MIN,
            TabKind::TemplateDetail(id) => id.saturating_add(1).saturating_neg(),
        }
    }

    fn persisted_key(self) -> PersistedTabKey {
        match self {
            Self::Conversation(id) => PersistedTabKey::Conversation { id },
            Self::TemplateList => PersistedTabKey::TemplateList,
            Self::TemplateDetail(id) => PersistedTabKey::TemplateDetail { id },
        }
    }
}

#[derive(Clone)]
enum TabPanel {
    Conversation(Entity<ConversationPanelView>),
    TemplateList(Entity<TemplateListView>),
    TemplateDetail(Entity<TemplateDetailView>),
}

impl TabPanel {
    fn into_any_element(self) -> AnyElement {
        match self {
            Self::Conversation(panel) => panel.into_any_element(),
            Self::TemplateList(panel) => panel.into_any_element(),
            Self::TemplateDetail(panel) => panel.into_any_element(),
        }
    }
}

#[derive(Clone)]
struct AppTab {
    kind: TabKind,
    icon: SharedString,
    name: SharedString,
    panel: TabPanel,
}

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

    fn path() -> AiChatResult<PathBuf> {
        let dir = dirs_next::config_dir()
            .ok_or(AiChatError::DbPath)?
            .join(APP_NAME);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(dir.join(STATE_FILE_NAME))
    }

    fn load_persisted() -> AiChatResult<PersistedWorkspaceState> {
        let path = Self::path()?;
        match std::fs::read_to_string(&path) {
            Ok(file) => match toml::from_str::<PersistedWorkspaceState>(&file) {
                Ok(mut state) => {
                    state.version = STATE_VERSION;
                    Ok(state)
                }
                Err(err) => {
                    event!(Level::ERROR, "parse state.toml failed: {}", err);
                    let state = PersistedWorkspaceState::default();
                    Self::write_persisted(&state)?;
                    Ok(state)
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let state = PersistedWorkspaceState::default();
                Self::write_persisted(&state)?;
                Ok(state)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn write_persisted(state: &PersistedWorkspaceState) -> AiChatResult<()> {
        let path = Self::path()?;
        let text = toml::to_string_pretty(state)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    fn restore_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let restored = self.persisted.tabs.clone();
        for tab in restored {
            match tab {
                PersistedTab::Conversation { id, draft } => {
                    let _ = self.try_add_restored_conversation_tab(id, draft, window, cx);
                }
                PersistedTab::TemplateList => {
                    self.tabs.push(Self::template_tab(window, cx));
                }
                PersistedTab::TemplateDetail { id } => {
                    if Self::template_exists(id, cx) {
                        self.tabs.push(Self::template_detail_tab(id, window, cx));
                    }
                }
            }
        }

        if self.tabs.is_empty()
            && let Some(conversation) = cx
                .global::<ChatData>()
                .read(cx)
                .as_ref()
                .ok()
                .and_then(ChatDataInner::first_conversation)
                .cloned()
        {
            self.push_conversation_tab(conversation, None, window, cx);
        }

        self.active_tab = self
            .persisted
            .active_tab
            .as_ref()
            .and_then(|active| {
                self.tabs
                    .iter()
                    .find(|tab| &tab.kind.persisted_key() == active)
            })
            .map(|tab| tab.kind)
            .or_else(|| self.tabs.first().map(|tab| tab.kind));
        self.sync_persisted_tabs();
    }

    fn try_add_restored_conversation_tab(
        &mut self,
        conversation_id: i32,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(conversation) = cx
            .global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .and_then(|data| data.conversation(conversation_id))
            .cloned()
        else {
            return false;
        };
        self.push_conversation_tab(conversation, draft, window, cx);
        true
    }

    fn template_exists(template_id: i32, cx: &App) -> bool {
        cx.global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::find(template_id, &mut conn).ok())
            .is_some()
    }

    fn conversation_tab(
        conversation: crate::database::Conversation,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut App,
    ) -> AppTab {
        let panel = cx.new(|cx| ConversationPanelView::new(&conversation, window, cx));
        if let Some(draft) = draft.clone() {
            panel.update(cx, |panel, cx| {
                panel.restore_draft(draft, window, cx);
            });
        }
        AppTab {
            kind: TabKind::Conversation(conversation.id),
            icon: SharedString::from(conversation.icon),
            name: SharedString::from(conversation.title),
            panel: TabPanel::Conversation(panel),
        }
    }

    fn push_conversation_tab(
        &mut self,
        conversation: crate::database::Conversation,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let resolved_draft = self.resolve_initial_conversation_draft(conversation.id, draft);
        self.upsert_persisted_conversation_draft(conversation.id, resolved_draft.clone());
        self.tabs.push(Self::conversation_tab(
            conversation,
            resolved_draft,
            window,
            cx,
        ));
    }

    fn resolve_initial_conversation_draft(
        &self,
        conversation_id: i32,
        explicit_draft: Option<ConversationDraft>,
    ) -> Option<ConversationDraft> {
        explicit_draft
            .or_else(|| self.draft_for(conversation_id))
            .or_else(|| {
                self.persisted
                    .latest_model_preset
                    .as_ref()
                    .map(ConversationDraft::from_latest_model_preset)
            })
    }

    fn template_tab(window: &mut Window, cx: &mut App) -> AppTab {
        AppTab {
            kind: TabKind::TemplateList,
            icon: "📋".into(),
            name: cx.global::<I18n>().t("tab-templates").into(),
            panel: TabPanel::TemplateList(cx.new(|cx| TemplateListView::new(window, cx))),
        }
    }

    fn template_detail_tab(template_id: i32, window: &mut Window, cx: &mut App) -> AppTab {
        let (icon, name) = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::find(template_id, &mut conn).ok())
            .map(|template| {
                (
                    SharedString::from(template.icon),
                    SharedString::from(format!(
                        "{}: {}",
                        cx.global::<I18n>().t("tab-template"),
                        template.name
                    )),
                )
            })
            .unwrap_or_else(|| {
                (
                    SharedString::from("🧩"),
                    SharedString::from(format!(
                        "{} #{template_id}",
                        cx.global::<I18n>().t("tab-template")
                    )),
                )
            });
        AppTab {
            kind: TabKind::TemplateDetail(template_id),
            icon,
            name,
            panel: TabPanel::TemplateDetail(
                cx.new(|cx| TemplateDetailView::new(template_id, window, cx)),
            ),
        }
    }

    fn sync_persisted_tabs(&mut self) {
        let mut next_tabs = Vec::with_capacity(self.tabs.len());
        for tab in &self.tabs {
            match tab.kind {
                TabKind::Conversation(id) => next_tabs.push(
                    self.persisted
                        .tabs
                        .iter()
                        .find_map(|persisted| match persisted {
                            PersistedTab::Conversation {
                                id: persisted_id,
                                draft,
                            } if *persisted_id == id => Some(PersistedTab::Conversation {
                                id,
                                draft: draft.clone(),
                            }),
                            _ => None,
                        })
                        .unwrap_or(PersistedTab::Conversation { id, draft: None }),
                ),
                TabKind::TemplateList => next_tabs.push(PersistedTab::TemplateList),
                TabKind::TemplateDetail(id) => next_tabs.push(PersistedTab::TemplateDetail { id }),
            }
        }
        self.persisted.tabs = next_tabs;
        self.persisted.active_tab = self.active_tab.map(TabKind::persisted_key);
        self.persisted.version = STATE_VERSION;
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.persisted.clone();
        self.save_task = Some(cx.spawn(async move |_, _cx| {
            Timer::after(SAVE_DEBOUNCE).await;
            if let Err(err) = Self::write_persisted(&snapshot) {
                event!(Level::ERROR, "save state.toml failed: {}", err);
            }
        }));
    }

    pub(crate) fn save_now(&self) -> AiChatResult<()> {
        Self::write_persisted(&self.persisted)
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

    pub(crate) fn activate_tab(&mut self, tab_key: i32, window: &mut Window, cx: &mut Context<Self>) {
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

    fn upsert_latest_model_preset(&mut self, preset: LatestModelPreset) -> bool {
        if self.persisted.latest_model_preset.as_ref() == Some(&preset) {
            return false;
        }

        self.persisted.latest_model_preset = Some(preset);
        true
    }

    fn upsert_persisted_conversation_draft(
        &mut self,
        conversation_id: i32,
        draft: Option<ConversationDraft>,
    ) -> bool {
        if let Some(PersistedTab::Conversation { draft: current, .. }) =
            self.persisted.tabs.iter_mut().find(|tab| {
                matches!(tab, PersistedTab::Conversation { id, .. } if *id == conversation_id)
            })
        {
            if *current == draft {
                return false;
            }
            *current = draft;
            return true;
        }

        self.persisted.tabs.push(PersistedTab::Conversation {
            id: conversation_id,
            draft,
        });
        true
    }

    fn draft_for(&self, conversation_id: i32) -> Option<ConversationDraft> {
        self.persisted.tabs.iter().find_map(|tab| match tab {
            PersistedTab::Conversation { id, draft } if *id == conversation_id => draft.clone(),
            _ => None,
        })
    }

    fn remove_draft(&mut self, conversation_id: i32) {
        if let Some(PersistedTab::Conversation { draft, .. }) = self.persisted.tabs.iter_mut().find(
            |tab| matches!(tab, PersistedTab::Conversation { id, .. } if *id == conversation_id),
        ) {
            *draft = None;
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

#[cfg(test)]
#[path = "workspace_state_tests.rs"]
mod tests;

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
