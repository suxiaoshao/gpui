use crate::foundation::persistence;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

pub(crate) fn minimum_window_size() -> Size<Pixels> {
    size(px(800.), px(600.))
}

pub(crate) fn default_window_size() -> Size<Pixels> {
    size(px(960.), px(740.))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutState {
    pub main_window: Option<WindowPlacement>,
    #[serde(default = "default_sidebar_width", deserialize_with = "read_width")]
    pub sidebar_width: f32,
    #[serde(default = "default_history_width", deserialize_with = "read_width")]
    pub history_width: f32,
}
impl Global for LayoutState {}
impl Default for LayoutState {
    fn default() -> Self {
        Self {
            main_window: None,
            sidebar_width: default_sidebar_width(),
            history_width: default_history_width(),
        }
    }
}
fn default_sidebar_width() -> f32 {
    220.
}
fn default_history_width() -> f32 {
    300.
}
fn read_width<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = toml::Value::deserialize(deserializer)?;
    Ok(value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
        .map(|n| n as f32)
        .unwrap_or(f32::NAN))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowPlacement {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    maximized: bool,
}
impl WindowPlacement {
    fn validate(&self) -> Result<(), String> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .any(|v| !v.is_finite())
            || self.width <= 0.
            || self.height <= 0.
        {
            return Err("invalid window dimensions".into());
        }
        Ok(())
    }
    pub fn restored(self, cx: &App) -> WindowBounds {
        let displays = cx.displays();
        let chosen = displays
            .iter()
            .find(|display| {
                let b = display.bounds();
                self.x < f32::from(b.right())
                    && self.x + self.width > f32::from(b.left())
                    && self.y < f32::from(b.bottom())
                    && self.y + self.height > f32::from(b.top())
            })
            .or_else(|| displays.first());
        let Some(display) = chosen else {
            return WindowBounds::Windowed(Bounds::centered(None, default_window_size(), cx));
        };
        let b = display.bounds();
        let minimum = minimum_window_size();
        let width = self
            .width
            .max(minimum.width.into())
            .min(f32::from(b.size.width));
        let height = self
            .height
            .max(minimum.height.into())
            .min(f32::from(b.size.height));
        let intersects = self.x < f32::from(b.right())
            && self.x + self.width > f32::from(b.left())
            && self.y < f32::from(b.bottom())
            && self.y + self.height > f32::from(b.top());
        let (x, y) = if intersects {
            (
                self.x
                    .clamp(f32::from(b.left()), f32::from(b.right()) - width),
                self.y
                    .clamp(f32::from(b.top()), f32::from(b.bottom()) - height),
            )
        } else {
            (
                f32::from(b.left()) + (f32::from(b.size.width) - width) / 2.,
                f32::from(b.top()) + (f32::from(b.size.height) - height) / 2.,
            )
        };
        let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
        if self.maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }
    }
}
pub(crate) fn load(path: &Path) -> LayoutState {
    match read(path) {
        Ok(layout) => layout,
        Err(error) => {
            tracing::warn!(%error, "layout unavailable; discarding saved state");
            if let Err(error) = fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(%error, "discard layout failed");
            }
            LayoutState::default()
        }
    }
}

pub(crate) fn read(path: &Path) -> Result<LayoutState, String> {
    let Some(bytes) = persistence::read(path).map_err(|e| e.to_string())? else {
        return Ok(LayoutState::default());
    };
    let mut value: LayoutState =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
            .map_err(|_| "invalid window state".to_owned())?;
    if let Some(placement) = value.main_window {
        placement.validate()?;
    }
    if !value.sidebar_width.is_finite() || !(160. ..=480.).contains(&value.sidebar_width) {
        value.sidebar_width = default_sidebar_width();
    }
    if !value.history_width.is_finite() || !(240. ..=520.).contains(&value.history_width) {
        value.history_width = default_history_width();
    }
    Ok(value)
}
pub(crate) fn capture(window: &Window, previous: &LayoutState) -> LayoutState {
    let (bounds, maximized) = match window.window_bounds() {
        WindowBounds::Windowed(b) => (b, false),
        WindowBounds::Maximized(b) | WindowBounds::Fullscreen(b) => (b, true),
    };
    LayoutState {
        main_window: Some(WindowPlacement {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
            maximized,
        }),
        sidebar_width: previous.sidebar_width,
        history_width: previous.history_width,
    }
}
pub(crate) fn save(path: &Path, value: &LayoutState) -> Result<(), String> {
    let bytes = toml::to_string_pretty(value).map_err(|e| e.to_string())?;
    // Layout is disposable application state: replacing it must not depend on
    // being able to read the previous file or reconcile external edits.
    let parent = path.parent().ok_or("missing parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes.as_bytes())
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
