use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::{App, AppContext, Context, Entity, Global, Pixels, Task, px};
use serde::{Deserialize, Serialize};
use tracing::{Level, event};

use crate::{errors::AiChat2Result, state::AiChat2Config};

const STATE_FILE_NAME: &str = "state.toml";
const STATE_VERSION: u32 = 1;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

pub(crate) const SIDEBAR_MIN_WIDTH: Pixels = px(180.);
pub(crate) const SIDEBAR_DEFAULT_WIDTH: Pixels = px(300.);
pub(crate) const SIDEBAR_MAX_WIDTH: Pixels = px(420.);

#[derive(Clone)]
pub(crate) struct LayoutStateStore(Entity<AiChat2LayoutState>);

impl LayoutStateStore {
    pub(crate) fn entity(&self) -> Entity<AiChat2LayoutState> {
        self.0.clone()
    }
}

impl Global for LayoutStateStore {}

pub(crate) struct AiChat2LayoutState {
    persisted: PersistedLayoutState,
    save_task: Option<Task<()>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
struct PersistedLayoutState {
    version: u32,
    sidebar_width: f32,
}

impl Default for PersistedLayoutState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_DEFAULT_WIDTH),
        }
    }
}

impl PersistedLayoutState {
    fn normalize(mut self) -> Self {
        self.version = STATE_VERSION;
        self.sidebar_width = clamp_sidebar_width_value(self.sidebar_width);
        self
    }
}

impl AiChat2LayoutState {
    fn new(persisted: PersistedLayoutState) -> Self {
        Self {
            persisted: persisted.normalize(),
            save_task: None,
        }
    }

    pub(crate) fn sidebar_width(&self) -> Pixels {
        px(self.persisted.sidebar_width)
    }

    pub(crate) fn set_sidebar_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let next_width = f32::from(clamp_sidebar_width(width));
        if self.persisted.sidebar_width == next_width {
            return;
        }

        self.persisted.sidebar_width = next_width;
        self.schedule_save(cx);
        cx.notify();
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        let path = Self::path();
        let snapshot = self.persisted.clone();
        self.save_task = Some(cx.spawn(async move |_, _cx| {
            smol::Timer::after(SAVE_DEBOUNCE).await;
            let path = match path {
                Ok(path) => path,
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "resolve ai-chat2 state path failed");
                    return;
                }
            };

            if let Err(err) = write_persisted_to_path(&path, &snapshot) {
                event!(Level::ERROR, error = ?err, "save ai-chat2 state.toml failed");
            }
        }));
    }

    fn load_or_create() -> AiChat2Result<Self> {
        let path = Self::path()?;
        Ok(Self::new(load_or_create_from_path(&path)?))
    }

    fn path() -> AiChat2Result<PathBuf> {
        Ok(AiChat2Config::config_dir()?.join(STATE_FILE_NAME))
    }
}

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let state = AiChat2LayoutState::load_or_create()?;
    event!(
        Level::INFO,
        sidebar_width = state.persisted.sidebar_width,
        "loaded ai-chat2 layout state"
    );
    let state = cx.new(|_| state);
    cx.set_global(LayoutStateStore(state));
    Ok(())
}

pub(crate) fn clamp_sidebar_width(width: Pixels) -> Pixels {
    width.max(SIDEBAR_MIN_WIDTH).min(SIDEBAR_MAX_WIDTH)
}

fn clamp_sidebar_width_value(width: f32) -> f32 {
    if !width.is_finite() {
        return f32::from(SIDEBAR_DEFAULT_WIDTH);
    }

    f32::from(clamp_sidebar_width(px(width)))
}

fn load_or_create_from_path(path: &Path) -> AiChat2Result<PersistedLayoutState> {
    match fs::read_to_string(path) {
        Ok(source) => match toml::from_str::<PersistedLayoutState>(&source) {
            Ok(state) => {
                let normalized = state.normalize();
                write_persisted_to_path(path, &normalized)?;
                Ok(normalized)
            }
            Err(err) => {
                event!(Level::ERROR, error = ?err, "parse ai-chat2 state.toml failed");
                let state = PersistedLayoutState::default();
                write_persisted_to_path(path, &state)?;
                Ok(state)
            }
        },
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let state = PersistedLayoutState::default();
            write_persisted_to_path(path, &state)?;
            Ok(state)
        }
        Err(err) => Err(err.into()),
    }
}

fn write_persisted_to_path(path: &Path, state: &PersistedLayoutState) -> AiChat2Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, toml::to_string_pretty(state)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AiChat2LayoutState, PersistedLayoutState, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH,
        SIDEBAR_MIN_WIDTH, STATE_VERSION, clamp_sidebar_width, load_or_create_from_path,
        write_persisted_to_path,
    };
    use gpui::px;

    #[test]
    fn layout_state_toml_defaults_and_roundtrips() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("state.toml");

        let default_state = load_or_create_from_path(&path).expect("create default layout state");

        assert_eq!(default_state.version, STATE_VERSION);
        assert_eq!(
            default_state.sidebar_width,
            f32::from(SIDEBAR_DEFAULT_WIDTH)
        );

        let source = std::fs::read_to_string(&path).expect("read default state");
        assert!(source.contains("version = 1"));
        assert!(source.contains("sidebar_width = 300.0"));

        let custom_state = PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: 260.,
        };
        write_persisted_to_path(&path, &custom_state).expect("write custom state");

        let loaded = load_or_create_from_path(&path).expect("load custom state");

        assert_eq!(loaded, custom_state);
    }

    #[test]
    fn sidebar_width_clamps_loaded_layout_values() {
        let below_min = AiChat2LayoutState::new(PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_MIN_WIDTH) - 1.,
        });
        let above_max = AiChat2LayoutState::new(PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_MAX_WIDTH) + 1.,
        });
        let normal = AiChat2LayoutState::new(PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_DEFAULT_WIDTH),
        });

        assert_eq!(below_min.sidebar_width(), SIDEBAR_MIN_WIDTH);
        assert_eq!(above_max.sidebar_width(), SIDEBAR_MAX_WIDTH);
        assert_eq!(normal.sidebar_width(), SIDEBAR_DEFAULT_WIDTH);
    }

    #[test]
    fn sidebar_width_clamps_direct_values() {
        assert_eq!(clamp_sidebar_width(px(100.)), SIDEBAR_MIN_WIDTH);
        assert_eq!(clamp_sidebar_width(px(500.)), SIDEBAR_MAX_WIDTH);
        assert_eq!(clamp_sidebar_width(px(240.)), px(240.));
    }
}
