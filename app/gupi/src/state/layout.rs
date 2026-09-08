use crate::foundation::persistence;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
pub(crate) fn load(path: &Path) -> Result<LayoutState, String> {
    let Some(bytes) = persistence::read(path).map_err(|e| e.to_string())? else {
        return Ok(LayoutState { main_window: None });
    };
    let value: LayoutState =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
            .map_err(|_| "invalid window state".to_owned())?;
    if let Some(placement) = value.main_window {
        placement.validate()?;
    }
    Ok(value)
}
pub(crate) fn capture(window: &Window) -> LayoutState {
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
    }
}
pub(crate) fn save(path: &Path, value: &LayoutState) -> Result<(), String> {
    let bytes = toml::to_string_pretty(value).map_err(|e| e.to_string())?;
    let expected = persistence::read(path).map_err(|e| e.to_string())?;
    persistence::replace(path, expected.as_deref(), bytes.as_bytes()).map_err(|e| e.to_string())
}
pub(crate) fn reset(path: &Path) -> Result<Option<PathBuf>, String> {
    let old = persistence::read(path).map_err(|e| e.to_string())?;
    let backup = old
        .as_ref()
        .map(|b| persistence::backup(path, b))
        .transpose()
        .map_err(|e| e.to_string())?;
    let bytes =
        toml::to_string_pretty(&LayoutState { main_window: None }).map_err(|e| e.to_string())?;
    persistence::replace(path, old.as_deref(), bytes.as_bytes()).map_err(|e| e.to_string())?;
    Ok(backup)
}
