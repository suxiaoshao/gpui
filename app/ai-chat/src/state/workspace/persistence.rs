use super::{TabKind, WorkspaceState};
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
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            open_folder_ids: Default::default(),
            tabs: Vec::new(),
            active_tab: None,
            latest_model_preset: None,
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
    DEFAULT_SIDEBAR_WIDTH
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
