use super::{SIDEBAR_DEFAULT_WIDTH, TabKind, WorkspaceState};
use crate::{
    app::APP_NAME,
    database::Mode,
    errors::{AiChatError, AiChatResult},
};
use gpui::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, io::ErrorKind, path::PathBuf, time::Duration};
use tracing::{Level, event};

const STATE_FILE_NAME: &str = "state.toml";
const STATE_VERSION: u32 = 1;
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
pub(super) struct LatestModelPreset {
    #[serde(default)]
    pub(super) provider_name: String,
    #[serde(default)]
    pub(super) model_id: String,
    #[serde(
        default = "default_request_template",
        serialize_with = "serialize_request_template",
        deserialize_with = "deserialize_request_template"
    )]
    pub(super) request_template: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum PersistedTabKey {
    Conversation { id: i32 },
    TemplateList,
    TemplateDetail { id: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum PersistedTab {
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

const MIN_RESTORED_WINDOW_VISIBLE_WIDTH: f32 = 160.;
const MIN_RESTORED_WINDOW_VISIBLE_HEIGHT: f32 = 120.;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PersistedWindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(super) struct PersistedWindowBounds {
    #[serde(default)]
    pub(super) mode: PersistedWindowMode,
    #[serde(default)]
    pub(super) x: f32,
    #[serde(default)]
    pub(super) y: f32,
    #[serde(default)]
    pub(super) width: f32,
    #[serde(default)]
    pub(super) height: f32,
    #[serde(default)]
    pub(super) display_id: Option<u32>,
}

impl PersistedWindowBounds {
    pub(super) fn from_window_bounds(
        window_bounds: WindowBounds,
        display_id: Option<DisplayId>,
    ) -> Self {
        let (mode, bounds) = match window_bounds {
            WindowBounds::Windowed(bounds) => (PersistedWindowMode::Windowed, bounds),
            WindowBounds::Maximized(bounds) => (PersistedWindowMode::Maximized, bounds),
            WindowBounds::Fullscreen(bounds) => (PersistedWindowMode::Fullscreen, bounds),
        };

        Self {
            mode,
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            display_id: display_id.map(u32::from),
        }
    }

    pub(super) fn window_bounds(self) -> WindowBounds {
        let bounds = self.bounds();
        match self.mode {
            PersistedWindowMode::Windowed => WindowBounds::Windowed(bounds),
            PersistedWindowMode::Maximized => WindowBounds::Maximized(bounds),
            PersistedWindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
        }
    }

    pub(super) fn bounds(self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        )
    }

    fn has_valid_size(self) -> bool {
        self.width > 0. && self.height > 0.
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WindowDisplaySnapshot {
    pub(super) id: u32,
    pub(super) bounds: Bounds<Pixels>,
    pub(super) is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedWindowBounds {
    pub(super) window_bounds: WindowBounds,
    pub(super) display_id: u32,
}

fn has_meaningful_visible_area(
    window_bounds: Bounds<Pixels>,
    display_bounds: Bounds<Pixels>,
) -> bool {
    let window_left = f32::from(window_bounds.origin.x);
    let window_top = f32::from(window_bounds.origin.y);
    let window_right = window_left + f32::from(window_bounds.size.width);
    let window_bottom = window_top + f32::from(window_bounds.size.height);
    let display_left = f32::from(display_bounds.origin.x);
    let display_top = f32::from(display_bounds.origin.y);
    let display_right = display_left + f32::from(display_bounds.size.width);
    let display_bottom = display_top + f32::from(display_bounds.size.height);

    let visible_width = window_right.min(display_right) - window_left.max(display_left);
    let visible_height = window_bottom.min(display_bottom) - window_top.max(display_top);
    let required_width = f32::from(window_bounds.size.width).min(MIN_RESTORED_WINDOW_VISIBLE_WIDTH);
    let required_height =
        f32::from(window_bounds.size.height).min(MIN_RESTORED_WINDOW_VISIBLE_HEIGHT);

    visible_width >= required_width && visible_height >= required_height
}

pub(super) fn resolve_persisted_window_bounds(
    persisted: Option<PersistedWindowBounds>,
    displays: &[WindowDisplaySnapshot],
) -> Option<ResolvedWindowBounds> {
    let persisted = persisted?;
    if !persisted.has_valid_size() {
        return None;
    }

    let bounds = persisted.bounds();
    let display = match persisted.display_id {
        Some(display_id) => displays.iter().find(|display| display.id == display_id)?,
        None => displays
            .iter()
            .find(|display| has_meaningful_visible_area(bounds, display.bounds))?,
    };

    if !has_meaningful_visible_area(bounds, display.bounds) {
        return None;
    }

    Some(ResolvedWindowBounds {
        window_bounds: persisted.window_bounds(),
        display_id: display.id,
    })
}

pub(super) fn fallback_display_id_for_persisted_window(
    persisted: Option<PersistedWindowBounds>,
    displays: &[WindowDisplaySnapshot],
) -> Option<u32> {
    persisted
        .and_then(|persisted| persisted.display_id)
        .and_then(|display_id| {
            displays
                .iter()
                .any(|display| display.id == display_id)
                .then_some(display_id)
        })
        .or_else(|| {
            displays
                .iter()
                .find(|display| display.is_primary)
                .map(|display| display.id)
        })
        .or_else(|| displays.first().map(|display| display.id))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct PersistedWorkspaceState {
    #[serde(default = "default_state_version")]
    pub(super) version: u32,
    #[serde(default = "default_sidebar_width")]
    pub(super) sidebar_width: f32,
    #[serde(default)]
    pub(super) open_folder_ids: BTreeSet<i32>,
    #[serde(default)]
    pub(super) tabs: Vec<PersistedTab>,
    #[serde(default)]
    pub(super) active_tab: Option<PersistedTabKey>,
    #[serde(default)]
    pub(super) latest_model_preset: Option<LatestModelPreset>,
    #[serde(default)]
    pub(super) main_window_bounds: Option<PersistedWindowBounds>,
    #[serde(default)]
    pub(super) settings_window_bounds: Option<PersistedWindowBounds>,
}

impl ConversationDraft {
    pub(super) fn to_latest_model_preset(&self) -> Option<LatestModelPreset> {
        if self.provider_name.is_empty() || self.model_id.is_empty() {
            return None;
        }

        Some(LatestModelPreset {
            provider_name: self.provider_name.clone(),
            model_id: self.model_id.clone(),
            request_template: self.request_template.clone(),
        })
    }

    pub(super) fn from_latest_model_preset(preset: &LatestModelPreset) -> Self {
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

impl Default for PersistedWorkspaceState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            sidebar_width: default_sidebar_width(),
            open_folder_ids: Default::default(),
            tabs: Vec::new(),
            active_tab: None,
            latest_model_preset: None,
            main_window_bounds: None,
            settings_window_bounds: None,
        }
    }
}

impl WorkspaceState {
    pub(super) fn load_persisted() -> AiChatResult<PersistedWorkspaceState> {
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

    pub(super) fn write_persisted(state: &PersistedWorkspaceState) -> AiChatResult<()> {
        let path = Self::path()?;
        let text = toml::to_string_pretty(state)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    pub(super) fn path() -> AiChatResult<PathBuf> {
        let dir = dirs_next::config_dir()
            .ok_or(AiChatError::DbPath)?
            .join(APP_NAME);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(dir.join(STATE_FILE_NAME))
    }

    pub(super) fn resolve_initial_conversation_draft(
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

    pub(super) fn sync_persisted_tabs(&mut self) {
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

    pub(super) fn schedule_save(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.persisted.clone();
        self.save_task = Some(cx.spawn(async move |_, _cx| {
            smol::Timer::after(SAVE_DEBOUNCE).await;
            if let Err(err) = Self::write_persisted(&snapshot) {
                event!(Level::ERROR, "save state.toml failed: {}", err);
            }
        }));
    }

    pub(crate) fn save_now(&self) -> AiChatResult<()> {
        Self::write_persisted(&self.persisted)
    }

    pub(super) fn upsert_latest_model_preset(&mut self, preset: LatestModelPreset) -> bool {
        if self.persisted.latest_model_preset.as_ref() == Some(&preset) {
            return false;
        }

        self.persisted.latest_model_preset = Some(preset);
        true
    }

    pub(super) fn upsert_persisted_conversation_draft(
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

    pub(super) fn draft_for(&self, conversation_id: i32) -> Option<ConversationDraft> {
        self.persisted.tabs.iter().find_map(|tab| match tab {
            PersistedTab::Conversation { id, draft } if *id == conversation_id => draft.clone(),
            _ => None,
        })
    }

    pub(super) fn remove_draft(&mut self, conversation_id: i32) {
        if let Some(PersistedTab::Conversation { draft, .. }) = self.persisted.tabs.iter_mut().find(
            |tab| matches!(tab, PersistedTab::Conversation { id, .. } if *id == conversation_id),
        ) {
            *draft = None;
        }
    }
}

fn default_mode() -> Mode {
    Mode::Contextual
}

fn default_request_template() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

fn default_state_version() -> u32 {
    STATE_VERSION
}

fn default_sidebar_width() -> f32 {
    f32::from(SIDEBAR_DEFAULT_WIDTH)
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
