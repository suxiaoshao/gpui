use std::{rc::Rc, time::Duration};

use crate::{app::APP_NAME, features::temporary::TemporaryWindow};
use gpui::*;
use gpui_component::Root;
#[cfg(target_os = "macos")]
use platform_ext::app::{
    NSRunningApplication, Retained, record_frontmost_app, restore_frontmost_app,
};
use tracing::{Level, event};
use window_ext::{NativeWindowHandle, WindowExt as _, WindowLevel};

const TEMPORARY_WINDOW_SIZE: Size<Pixels> = size(px(960.), px(620.));
const TEMPORARY_WINDOW_LEVEL: WindowLevel = WindowLevel::ModalPanel;

#[derive(Clone, Copy)]
struct TemporaryWindowReveal {
    native_window: NativeWindowHandle,
    target_bounds: Bounds<Pixels>,
    target_display_id: Option<DisplayId>,
}

struct TemporaryWindowLifecycleState {
    delay_close: Option<Task<()>>,
    #[cfg(target_os = "macos")]
    front_app: Option<Retained<NSRunningApplication>>,
}

impl Global for TemporaryWindowLifecycleState {}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<TemporaryWindowLifecycleState>() {
        return;
    }

    cx.set_global(TemporaryWindowLifecycleState {
        delay_close: None,
        #[cfg(target_os = "macos")]
        front_app: None,
    });
}

pub(crate) fn open_temporary_window(cx: &mut App) -> Option<WindowHandle<Root>> {
    with_lifecycle_state(cx, |state, cx| state.ensure_temporary_window_visible(cx)).flatten()
}

pub(crate) fn show_created_conversation(
    created: crate::state::conversations::CreatedConversation,
    cx: &mut App,
) -> Option<WindowHandle<Root>> {
    with_lifecycle_state(cx, |state, cx| state.show_created_conversation(created, cx)).flatten()
}

pub(crate) fn toggle_temporary_window(cx: &mut App) {
    let _ = with_lifecycle_state(cx, |state, cx| {
        state.toggle_temporary_window(cx);
    });
}

pub(crate) fn request_hide_for_window_activation(window: &mut Window, cx: &mut App) {
    let is_visible = window.is_visible().unwrap_or(false);
    if !should_hide_for_window_activation(window.is_window_active(), is_visible) {
        return;
    }

    request_hide_with_delay(window, cx);
}

pub(crate) fn request_hide_with_delay(window: &mut Window, cx: &mut App) {
    let _ = with_lifecycle_state(cx, |state, cx| {
        state.delay_or_hide_temporary_window(window, cx);
    });
}

fn with_lifecycle_state<R>(
    cx: &mut App,
    callback: impl FnOnce(&mut TemporaryWindowLifecycleState, &mut App) -> R,
) -> Option<R> {
    if !cx.has_global::<TemporaryWindowLifecycleState>() {
        event!(
            Level::ERROR,
            "jaco temporary window lifecycle state is not initialized"
        );
        return None;
    }

    Some(cx.update_global::<TemporaryWindowLifecycleState, _>(callback))
}

impl TemporaryWindowLifecycleState {
    fn show_created_conversation(
        &mut self,
        created: crate::state::conversations::CreatedConversation,
        cx: &mut App,
    ) -> Option<WindowHandle<Root>> {
        let window = find_temporary_window(cx).or_else(|| self.create_temporary_window(cx))?;
        let mut reveal = None;
        let mut handled = false;
        let update_result = window.update(cx, |root, window, cx| {
            let Ok(view) = root.view().clone().downcast::<TemporaryWindow>() else {
                event!(
                    Level::ERROR,
                    "temporary window root did not contain TemporaryWindow view"
                );
                return;
            };
            handled = true;
            view.update(cx, |view, cx| {
                let started = view.open_created_conversation(created, window, cx);
                if !started {
                    event!(
                        Level::DEBUG,
                        "temporary conversation run was already active"
                    );
                }
            });
            reveal = self.prepare_temporary_window(root, window, cx);
        });
        match update_result {
            Ok(()) => {
                if !handled {
                    return None;
                }
                if let Some(reveal) = reveal {
                    self.schedule_temporary_window_reveal(reveal, cx);
                }
                Some(window)
            }
            Err(err) => {
                event!(Level::ERROR, error = ?err, "show created conversation in temporary window failed");
                None
            }
        }
    }

    fn ensure_temporary_window_visible(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        let window = find_temporary_window(cx).or_else(|| self.create_temporary_window(cx))?;
        let mut reveal = None;
        match window.update(cx, |root, window, cx| {
            reveal = self.prepare_temporary_window(root, window, cx);
        }) {
            Ok(()) => {
                if let Some(reveal) = reveal {
                    self.schedule_temporary_window_reveal(reveal, cx);
                }
                Some(window)
            }
            Err(err) => {
                event!(Level::ERROR, error = ?err, "activate jaco temporary window failed");
                None
            }
        }
    }

    fn toggle_temporary_window(&mut self, cx: &mut App) {
        match find_temporary_window(cx) {
            Some(window) => {
                let mut reveal = None;
                if let Err(err) = window.update(cx, |root, window, cx| {
                    if window.is_visible().unwrap_or(false) {
                        self.delay_or_hide_temporary_window(window, cx);
                    } else {
                        reveal = self.prepare_temporary_window(root, window, cx);
                    }
                }) {
                    event!(Level::ERROR, error = ?err, "toggle jaco temporary window failed");
                }
                if let Some(reveal) = reveal {
                    self.schedule_temporary_window_reveal(reveal, cx);
                }
            }
            None => {
                let _ = self.ensure_temporary_window_visible(cx);
            }
        }
    }

    fn create_temporary_window(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        let target_display_id = target_display_id(cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    target_display_id,
                    TEMPORARY_WINDOW_SIZE,
                    cx,
                ))),
                titlebar: Some(temporary_titlebar_options()),
                window_background: WindowBackgroundAppearance::Opaque,
                is_resizable: false,
                kind: WindowKind::PopUp,
                focus: false,
                show: false,
                display_id: target_display_id,
                app_id: Some(APP_NAME.to_owned()),
                ..Default::default()
            },
            |window, cx| {
                set_temporary_window_level(window);
                let view = cx.new(|cx| TemporaryWindow::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        );

        match result {
            Ok(window) => Some(window),
            Err(err) => {
                event!(Level::ERROR, error = ?err, "open jaco temporary window failed");
                None
            }
        }
    }

    fn prepare_temporary_window(
        &mut self,
        root: &mut Root,
        window: &mut Window,
        cx: &mut Context<Root>,
    ) -> Option<TemporaryWindowReveal> {
        self.delay_close = None;
        let target_display_id = target_display_id(cx);
        let target_bounds = recentered_bounds_for_display(
            target_display_id,
            window.bounds().size,
            TEMPORARY_WINDOW_SIZE,
            cx,
        );
        focus_search_input(root, window, cx);
        let native_window = match window.native_window_handle() {
            Ok(handle) => handle,
            Err(err) => {
                event!(Level::ERROR, error = ?err, "get jaco temporary window handle failed");
                return None;
            }
        };

        Some(TemporaryWindowReveal {
            native_window,
            target_bounds,
            target_display_id,
        })
    }

    fn schedule_temporary_window_reveal(&mut self, reveal: TemporaryWindowReveal, cx: &mut App) {
        #[cfg(target_os = "macos")]
        self.record_front_app();

        cx.defer(move |cx| {
            let mut restore_front_app = false;
            if let Err(err) = reveal
                .native_window
                .set_window_level(TEMPORARY_WINDOW_LEVEL)
            {
                event!(
                    Level::ERROR,
                    error = ?err,
                    level = ?TEMPORARY_WINDOW_LEVEL,
                    "set jaco temporary window level failed"
                );
            }
            if let Err(err) = reveal
                .native_window
                .move_and_resize(reveal.target_bounds, reveal.target_display_id)
            {
                event!(Level::ERROR, error = ?err, "reposition jaco temporary window failed");
            }

            if let Err(err) = reveal.native_window.show() {
                event!(Level::ERROR, error = ?err, "show jaco temporary window failed");
                restore_front_app = true;
            }

            if restore_front_app {
                let _ = with_lifecycle_state(cx, |state, _cx| {
                    state.restore_front_app();
                });
            }
        });
    }

    fn delay_or_hide_temporary_window(&mut self, window: &mut Window, cx: &mut App) {
        self.delay_close = Some(delay_close_temporary_window(window, cx));
        self.hide_temporary_window(window);
    }

    fn hide_temporary_window(&mut self, window: &mut Window) {
        if let Err(err) = window.hide() {
            event!(Level::ERROR, error = ?err, "hide jaco temporary window failed");
        }

        self.restore_front_app();
    }

    #[cfg(target_os = "macos")]
    fn record_front_app(&mut self) {
        if self.front_app.is_none() {
            self.front_app = record_frontmost_app();
        }
    }

    fn restore_front_app(&mut self) {
        #[cfg(target_os = "macos")]
        {
            restore_frontmost_app(&self.front_app);
            self.front_app = None;
        }
    }
}

fn set_temporary_window_level(window: &mut Window) {
    if let Err(err) = window.set_window_level(TEMPORARY_WINDOW_LEVEL) {
        event!(
            Level::ERROR,
            error = ?err,
            level = ?TEMPORARY_WINDOW_LEVEL,
            "set jaco temporary window level failed"
        );
    }
}

fn delay_close_temporary_window(window: &mut Window, cx: &mut App) -> Task<()> {
    let timer = cx.background_executor().timer(Duration::from_secs(600));
    window.spawn(cx, async |cx| {
        timer.await;
        if let Err(err) = cx.window_handle().update(cx, |_root, window, _cx| {
            window.remove_window();
        }) {
            event!(Level::ERROR, error = ?err, "remove jaco temporary window failed");
        }
    })
}

fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let root_view = root.read(cx).ok()?.view().clone();
        root_view.downcast::<TemporaryWindow>().ok().map(|_| root)
    })
}

fn focus_search_input(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let _ = root
        .view()
        .clone()
        .downcast::<TemporaryWindow>()
        .map(|view| {
            view.update(cx, |view, cx| view.focus_search_input(window, cx));
        });
}

fn temporary_titlebar_options() -> TitlebarOptions {
    TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(point(px(-100.), px(-100.))),
    }
}

fn target_display_id(cx: &App) -> Option<DisplayId> {
    target_display(cx).map(|display| display.id())
}

fn target_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    let displays = cx.displays();
    if displays.is_empty() {
        return None;
    }

    if let Some(display_id) = platform_ext::app::current_mouse_display_id()
        && let Some(display) = displays
            .iter()
            .find(|display| u64::from(display.id()) == u64::from(display_id))
    {
        return Some(display.clone());
    }

    let snapshots = display_snapshots(cx);
    if let Some((x, y)) = platform_ext::app::current_mouse_location()
        && let Some(display_id) = display_id_for_mouse_location(&snapshots, point(px(x), px(y)))
        && let Some(display) = displays
            .iter()
            .find(|display| u64::from(display.id()) == display_id)
    {
        return Some(display.clone());
    }

    cx.primary_display().or_else(|| displays.into_iter().next())
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DisplaySnapshot {
    id: u64,
    bounds: Bounds<Pixels>,
    is_primary: bool,
}

fn display_snapshots(cx: &App) -> Vec<DisplaySnapshot> {
    let primary_id = cx.primary_display().map(|display| u64::from(display.id()));
    cx.displays()
        .into_iter()
        .map(|display| DisplaySnapshot {
            id: u64::from(display.id()),
            bounds: display.bounds(),
            is_primary: primary_id == Some(u64::from(display.id())),
        })
        .collect()
}

fn display_id_for_mouse_location(
    displays: &[DisplaySnapshot],
    mouse_location: Point<Pixels>,
) -> Option<u64> {
    displays
        .iter()
        .find(|display| display.bounds.contains(&mouse_location))
        .map(|display| display.id)
        .or_else(|| {
            displays
                .iter()
                .find(|display| display.is_primary)
                .map(|display| display.id)
        })
}

fn recentered_bounds_for_display(
    display_id: Option<DisplayId>,
    current_size: Size<Pixels>,
    fallback_size: Size<Pixels>,
    cx: &App,
) -> Bounds<Pixels> {
    let size = display_id
        .and_then(|display_id| cx.find_display(display_id))
        .map(|display| {
            preserve_or_fallback_size(current_size, display.bounds().size, fallback_size)
        })
        .unwrap_or(fallback_size);

    Bounds::centered(display_id, size, cx)
}

fn preserve_or_fallback_size(
    current_size: Size<Pixels>,
    display_size: Size<Pixels>,
    fallback_size: Size<Pixels>,
) -> Size<Pixels> {
    if current_size.width <= px(0.)
        || current_size.height <= px(0.)
        || current_size.width > display_size.width
        || current_size.height > display_size.height
    {
        fallback_size
    } else {
        current_size
    }
}

fn should_hide_for_window_activation(is_active: bool, is_visible: bool) -> bool {
    !is_active && is_visible
}

#[cfg(test)]
mod tests {
    use super::{
        DisplaySnapshot, TEMPORARY_WINDOW_LEVEL, TEMPORARY_WINDOW_SIZE,
        display_id_for_mouse_location, preserve_or_fallback_size,
        should_hide_for_window_activation,
    };
    use gpui::{Bounds, point, px, size};
    use window_ext::WindowLevel;

    fn display(
        id: u64,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        is_primary: bool,
    ) -> DisplaySnapshot {
        DisplaySnapshot {
            id,
            bounds: Bounds::new(
                point(px(origin_x), px(origin_y)),
                size(px(width), px(height)),
            ),
            is_primary,
        }
    }

    #[test]
    fn mouse_location_selects_matching_display() {
        let displays = vec![
            display(1, 0., 0., 1440., 900., true),
            display(2, 1440., 0., 1920., 1080., false),
        ];

        assert_eq!(
            display_id_for_mouse_location(&displays, point(px(1800.), px(300.))),
            Some(2)
        );
    }

    #[test]
    fn mouse_location_falls_back_to_primary_display() {
        let displays = vec![
            display(1, 0., 0., 1440., 900., true),
            display(2, 1440., 0., 1920., 1080., false),
        ];

        assert_eq!(
            display_id_for_mouse_location(&displays, point(px(-100.), px(-100.))),
            Some(1)
        );
    }

    #[test]
    fn preserves_existing_window_size_when_it_fits_target_display() {
        let current = size(px(900.), px(600.));
        let display = size(px(1920.), px(1080.));

        assert_eq!(
            preserve_or_fallback_size(current, display, TEMPORARY_WINDOW_SIZE),
            current
        );
    }

    #[test]
    fn falls_back_to_temporary_size_when_existing_window_is_too_large() {
        let current = size(px(2400.), px(1400.));
        let display = size(px(1920.), px(1080.));

        assert_eq!(
            preserve_or_fallback_size(current, display, TEMPORARY_WINDOW_SIZE),
            TEMPORARY_WINDOW_SIZE
        );
    }

    #[test]
    fn window_activation_hide_only_applies_to_visible_inactive_window() {
        assert!(should_hide_for_window_activation(false, true));
        assert!(!should_hide_for_window_activation(true, true));
        assert!(!should_hide_for_window_activation(false, false));
    }

    #[test]
    fn temporary_window_level_matches_launcher_modal_panel() {
        assert_eq!(TEMPORARY_WINDOW_LEVEL, WindowLevel::ModalPanel);
    }
}
