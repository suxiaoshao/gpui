use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::*;
use serde::{Deserialize, Serialize};
use tracing::{Level, event};

use crate::{errors::AiChat2Result, state::AiChat2Config};

const STATE_FILE_NAME: &str = "state.toml";
const STATE_VERSION: u32 = 1;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

pub(crate) const SIDEBAR_MIN_WIDTH: Pixels = px(180.);
pub(crate) const SIDEBAR_DEFAULT_WIDTH: Pixels = px(300.);
pub(crate) const SIDEBAR_MAX_WIDTH: Pixels = px(420.);

const MIN_RESTORED_WINDOW_VISIBLE_WIDTH: f32 = 160.;
const MIN_RESTORED_WINDOW_VISIBLE_HEIGHT: f32 = 120.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowPlacementKind {
    Main,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowPlacement {
    pub(crate) window_bounds: WindowBounds,
    pub(crate) display_id: Option<DisplayId>,
}

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
    main_window_bounds: Option<PersistedWindowBounds>,
    settings_window_bounds: Option<PersistedWindowBounds>,
}

impl Default for PersistedLayoutState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_DEFAULT_WIDTH),
            main_window_bounds: None,
            settings_window_bounds: None,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PersistedWindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct PersistedWindowBounds {
    #[serde(default)]
    mode: PersistedWindowMode,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    width: f32,
    #[serde(default)]
    height: f32,
    #[serde(default)]
    display_id: Option<u64>,
}

impl PersistedWindowBounds {
    fn from_window_bounds(window_bounds: WindowBounds, display_id: Option<DisplayId>) -> Self {
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
            display_id: display_id.map(u64::from),
        }
    }

    fn window_bounds(self) -> WindowBounds {
        let bounds = self.bounds();
        match self.mode {
            PersistedWindowMode::Windowed => WindowBounds::Windowed(bounds),
            PersistedWindowMode::Maximized => WindowBounds::Maximized(bounds),
            PersistedWindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
        }
    }

    fn bounds(self) -> Bounds<Pixels> {
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
struct WindowDisplaySnapshot {
    id: u64,
    bounds: Bounds<Pixels>,
    is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedWindowBounds {
    window_bounds: WindowBounds,
    display_id: u64,
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

    pub(crate) fn set_window_bounds(
        &mut self,
        kind: WindowPlacementKind,
        window_bounds: WindowBounds,
        display_id: Option<DisplayId>,
        cx: &mut Context<Self>,
    ) {
        let next = PersistedWindowBounds::from_window_bounds(window_bounds, display_id);
        if self.update_persisted_window_bounds(kind, next) {
            self.schedule_save(cx);
        }
    }

    fn update_persisted_window_bounds(
        &mut self,
        kind: WindowPlacementKind,
        next: PersistedWindowBounds,
    ) -> bool {
        let next = Some(next);
        let current = match kind {
            WindowPlacementKind::Main => &mut self.persisted.main_window_bounds,
            WindowPlacementKind::Settings => &mut self.persisted.settings_window_bounds,
        };

        if *current == next {
            return false;
        }

        *current = next;
        true
    }

    pub(crate) fn save_now(&self) -> AiChat2Result<()> {
        let path = Self::path()?;
        write_persisted_to_path(&path, &self.persisted)
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

pub(crate) fn restored_window_placement(
    kind: WindowPlacementKind,
    fallback_size: Size<Pixels>,
    cx: &App,
) -> WindowPlacement {
    let persisted = if cx.has_global::<LayoutStateStore>() {
        cx.global::<LayoutStateStore>()
            .entity()
            .read(cx)
            .persisted
            .clone()
    } else {
        match AiChat2LayoutState::path().and_then(|path| load_or_create_from_path(&path)) {
            Ok(persisted) => persisted,
            Err(err) => {
                event!(Level::ERROR, error = ?err, "load ai-chat2 layout state for window placement failed");
                PersistedLayoutState::default()
            }
        }
    };
    restored_window_placement_from_state(&persisted, kind, fallback_size, cx)
}

fn restored_window_placement_from_state(
    persisted: &PersistedLayoutState,
    kind: WindowPlacementKind,
    fallback_size: Size<Pixels>,
    cx: &App,
) -> WindowPlacement {
    let displays = window_display_snapshots(cx);
    let persisted_bounds = persisted_window_bounds(persisted, kind);

    if let Some(resolved) = resolve_persisted_window_bounds(persisted_bounds, &displays) {
        return WindowPlacement {
            window_bounds: resolved.window_bounds,
            display_id: display_id_from_raw(cx, resolved.display_id),
        };
    }

    let fallback_display_id = fallback_display_id_for_persisted_window(persisted_bounds, &displays);
    let fallback_display_bounds = fallback_display_id.and_then(|display_id| {
        displays
            .iter()
            .find(|display| display.id == display_id)
            .map(|display| display.bounds)
    });
    let fallback_size = clamp_fallback_window_size(fallback_size, fallback_display_bounds);
    let display_id = fallback_display_id.and_then(|display_id| display_id_from_raw(cx, display_id));
    WindowPlacement {
        window_bounds: WindowBounds::Windowed(Bounds::centered(display_id, fallback_size, cx)),
        display_id,
    }
}

fn persisted_window_bounds(
    persisted: &PersistedLayoutState,
    kind: WindowPlacementKind,
) -> Option<PersistedWindowBounds> {
    match kind {
        WindowPlacementKind::Main => persisted.main_window_bounds,
        WindowPlacementKind::Settings => persisted.settings_window_bounds,
    }
}

fn window_display_snapshots(cx: &App) -> Vec<WindowDisplaySnapshot> {
    let primary_id = cx.primary_display().map(|display| u64::from(display.id()));
    cx.displays()
        .into_iter()
        .map(|display| WindowDisplaySnapshot {
            id: u64::from(display.id()),
            bounds: display.bounds(),
            is_primary: primary_id == Some(u64::from(display.id())),
        })
        .collect()
}

fn display_id_from_raw(cx: &App, raw_id: u64) -> Option<DisplayId> {
    cx.displays()
        .into_iter()
        .find(|display| u64::from(display.id()) == raw_id)
        .map(|display| display.id())
}

pub(crate) fn save_now(cx: &App) {
    if !cx.has_global::<LayoutStateStore>() {
        return;
    }

    let state = cx.global::<LayoutStateStore>().entity();
    if let Err(err) = state.read(cx).save_now() {
        event!(Level::ERROR, error = ?err, "save ai-chat2 state.toml on quit failed");
    }
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

fn clamp_fallback_window_size(
    fallback_size: Size<Pixels>,
    display_bounds: Option<Bounds<Pixels>>,
) -> Size<Pixels> {
    let Some(display_bounds) = display_bounds else {
        return fallback_size;
    };
    let display_width = f32::from(display_bounds.size.width);
    let display_height = f32::from(display_bounds.size.height);
    if display_width <= 0. || display_height <= 0. {
        return fallback_size;
    }

    size(
        px(f32::from(fallback_size.width).min(display_width)),
        px(f32::from(fallback_size.height).min(display_height)),
    )
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

fn resolve_persisted_window_bounds(
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

fn fallback_display_id_for_persisted_window(
    persisted: Option<PersistedWindowBounds>,
    displays: &[WindowDisplaySnapshot],
) -> Option<u64> {
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
        AiChat2LayoutState, PersistedLayoutState, PersistedWindowBounds, PersistedWindowMode,
        SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, STATE_VERSION,
        WindowDisplaySnapshot, WindowPlacementKind, clamp_fallback_window_size,
        clamp_sidebar_width, fallback_display_id_for_persisted_window, load_or_create_from_path,
        resolve_persisted_window_bounds, write_persisted_to_path,
    };
    use gpui::{Bounds, WindowBounds, point, px, size};

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
            ..PersistedLayoutState::default()
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
            ..PersistedLayoutState::default()
        });
        let above_max = AiChat2LayoutState::new(PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_MAX_WIDTH) + 1.,
            ..PersistedLayoutState::default()
        });
        let normal = AiChat2LayoutState::new(PersistedLayoutState {
            version: STATE_VERSION,
            sidebar_width: f32::from(SIDEBAR_DEFAULT_WIDTH),
            ..PersistedLayoutState::default()
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

    fn window_bounds(x: f32, y: f32, width: f32, height: f32) -> PersistedWindowBounds {
        PersistedWindowBounds {
            mode: PersistedWindowMode::Windowed,
            x,
            y,
            width,
            height,
            display_id: Some(1),
        }
    }

    fn display(
        id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        is_primary: bool,
    ) -> WindowDisplaySnapshot {
        WindowDisplaySnapshot {
            id,
            bounds: Bounds::new(point(px(x), px(y)), size(px(width), px(height))),
            is_primary,
        }
    }

    #[test]
    fn layout_state_roundtrips_window_bounds_per_window_type() {
        let main_window_bounds = window_bounds(10., 20., 1200., 800.);
        let settings_window_bounds = PersistedWindowBounds {
            mode: PersistedWindowMode::Maximized,
            x: 50.,
            y: 60.,
            width: 960.,
            height: 720.,
            display_id: Some(2),
        };
        let state = PersistedLayoutState {
            main_window_bounds: Some(main_window_bounds),
            settings_window_bounds: Some(settings_window_bounds),
            ..PersistedLayoutState::default()
        };

        let text = toml::to_string_pretty(&state).unwrap();
        let parsed: PersistedLayoutState = toml::from_str(&text).unwrap();

        assert_eq!(parsed.main_window_bounds, Some(main_window_bounds));
        assert_eq!(parsed.settings_window_bounds, Some(settings_window_bounds));
    }

    #[test]
    fn layout_window_bounds_resolve_only_when_visible_on_current_display() {
        let displays = vec![display(1, 0., 0., 1440., 900., true)];
        let valid = window_bounds(10., 20., 1200., 800.);

        let resolved = resolve_persisted_window_bounds(Some(valid), &displays).unwrap();

        assert_eq!(resolved.display_id, 1);
        assert_eq!(
            resolved.window_bounds,
            WindowBounds::Windowed(Bounds::new(
                point(px(10.), px(20.)),
                size(px(1200.), px(800.)),
            ))
        );
        assert_eq!(
            resolve_persisted_window_bounds(Some(window_bounds(10., 20., 0., 800.)), &displays),
            None
        );
        assert_eq!(
            resolve_persisted_window_bounds(
                Some(window_bounds(2000., 20., 1200., 800.)),
                &displays
            ),
            None
        );
        assert_eq!(
            resolve_persisted_window_bounds(
                Some(window_bounds(-1100., 20., 1200., 800.)),
                &displays
            ),
            None
        );
        assert_eq!(
            resolve_persisted_window_bounds(
                Some(PersistedWindowBounds {
                    display_id: Some(99),
                    ..valid
                }),
                &displays,
            ),
            None
        );
    }

    #[test]
    fn layout_fallback_display_prefers_saved_display_then_primary() {
        let displays = vec![
            display(1, 0., 0., 1440., 900., true),
            display(2, 1440., 0., 1920., 1080., false),
        ];

        assert_eq!(
            fallback_display_id_for_persisted_window(
                Some(PersistedWindowBounds {
                    display_id: Some(2),
                    ..window_bounds(3000., 3000., 1200., 800.)
                }),
                &displays,
            ),
            Some(2)
        );
        assert_eq!(
            fallback_display_id_for_persisted_window(
                Some(PersistedWindowBounds {
                    display_id: Some(99),
                    ..window_bounds(3000., 3000., 1200., 800.)
                }),
                &displays,
            ),
            Some(1)
        );
    }

    #[test]
    fn layout_fallback_window_size_clamps_to_display_bounds() {
        let small_display = display(1, 0., 0., 1366., 768., true);

        assert_eq!(
            clamp_fallback_window_size(size(px(1536.), px(864.)), Some(small_display.bounds)),
            size(px(1366.), px(768.))
        );
        assert_eq!(
            clamp_fallback_window_size(size(px(960.), px(720.)), Some(small_display.bounds)),
            size(px(960.), px(720.))
        );
        assert_eq!(
            clamp_fallback_window_size(size(px(1536.), px(864.)), None),
            size(px(1536.), px(864.))
        );
    }

    #[test]
    fn layout_window_bounds_updates_are_separated_by_window_type() {
        let main = window_bounds(10., 20., 1200., 800.);
        let settings = window_bounds(30., 40., 960., 720.);
        let mut state = AiChat2LayoutState {
            persisted: PersistedLayoutState::default(),
            save_task: None,
        };

        assert!(state.update_persisted_window_bounds(WindowPlacementKind::Main, main));
        assert!(!state.update_persisted_window_bounds(WindowPlacementKind::Main, main));
        assert!(state.update_persisted_window_bounds(WindowPlacementKind::Settings, settings));

        assert_eq!(state.persisted.main_window_bounds, Some(main));
        assert_eq!(state.persisted.settings_window_bounds, Some(settings));
    }
}
