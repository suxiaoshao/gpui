use super::*;
use crate::app::{find_window_by_view, with_root_view};

pub(crate) struct TemporaryWindowState {
    delay_close: Option<Task<()>>,
    #[cfg(target_os = "macos")]
    front_app: Option<Retained<NSRunningApplication>>,
}

impl Global for TemporaryWindowState {}

pub(super) enum FocusRestoreTarget {
    ExistingOrRecordCurrent,
    #[cfg(target_os = "macos")]
    Override(Option<Retained<NSRunningApplication>>),
}

impl FocusRestoreTarget {
    pub(super) fn restore_if_override(self) {
        #[cfg(target_os = "macos")]
        if let Self::Override(front_app) = self {
            restore_frontmost_app(&front_app);
        }
    }
}

pub(crate) fn init_temporary_window_state(cx: &mut App) {
    if cx.has_global::<TemporaryWindowState>() {
        return;
    }

    cx.set_global(TemporaryWindowState {
        delay_close: None,
        #[cfg(target_os = "macos")]
        front_app: None,
    });
}

fn with_temporary_window_state<R>(
    cx: &mut App,
    callback: impl FnOnce(&mut TemporaryWindowState, &mut App) -> R,
) -> Option<R> {
    if !cx.has_global::<TemporaryWindowState>() {
        event!(
            Level::ERROR,
            "Failed to access temporary window state: TemporaryWindowState is not initialized"
        );
        return None;
    }

    Some(cx.update_global::<TemporaryWindowState, _>(callback))
}

fn focus_temporary_window_chat_form(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let _ = with_root_view::<TemporaryView, _>(root, cx, |view, cx| {
        view.update(cx, |view, cx| view.focus_chat_form(window, cx));
    });
}

impl TemporaryWindowState {
    pub fn delay_close(window: &mut Window, cx: &mut App) -> Task<()> {
        window.spawn(cx, async |cx| {
            smol::Timer::after(Duration::from_secs(600)).await;
            if let Err(err) = cx.window_handle().update(cx, |_, window, _cx| {
                window.remove_window();
            }) {
                event!(Level::ERROR, "Failed to remove temporary window: {:?}", err);
            };
        })
    }

    pub(super) fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
        find_window_by_view::<TemporaryView>(cx)
    }

    pub(super) fn delay_or_hide_temporary_window(&mut self, window: &mut Window, cx: &mut App) {
        let task = Self::delay_close(window, cx);
        self.delay_close = Some(task);
        self.hide_temporary_window(window);
    }

    #[cfg(target_os = "macos")]
    fn record_front_app(&mut self) {
        if self.front_app.is_none() {
            self.front_app = record_frontmost_app();
        }
    }

    #[cfg(target_os = "macos")]
    fn prepare_front_app_for_visible_session(&mut self, restore_target: FocusRestoreTarget) {
        match restore_target {
            FocusRestoreTarget::ExistingOrRecordCurrent => self.record_front_app(),
            FocusRestoreTarget::Override(front_app) => {
                self.front_app = front_app;
            }
        }
    }

    pub fn request_hide_with_delay(window: &mut Window, cx: &mut App) {
        let _ = with_temporary_window_state(cx, |hotkeys, cx| {
            hotkeys.delay_or_hide_temporary_window(window, cx);
        });
    }

    fn hide_temporary_window(&mut self, window: &mut Window) {
        if let Err(err) = window.hide() {
            event!(Level::ERROR, "Failed to hide temporary window: {:?}", err);
        };
        #[cfg(target_os = "macos")]
        {
            restore_frontmost_app(&self.front_app);
            self.front_app = None;
        }
    }

    fn show_temporary_window_on_mouse_display(
        &mut self,
        window: &mut Window,
        cx: &App,
        restore_target: FocusRestoreTarget,
    ) -> bool {
        self.delay_close = None;
        #[cfg(not(target_os = "macos"))]
        let _ = &restore_target;
        let target_display_id = target_display_id(cx);
        let target_bounds = recentered_bounds_for_display(
            target_display_id,
            window.bounds().size,
            TEMPORARY_WINDOW_SIZE,
            cx,
        );
        if let Err(err) = window.move_and_resize(target_bounds, target_display_id) {
            event!(Level::ERROR, error = ?err, "Failed to reposition temporary window");
        }
        if let Err(err) = window.show_without_activation() {
            event!(Level::ERROR, error = ?err, "Failed to show temporary window");
            restore_target.restore_if_override();
            return false;
        }
        window.activate_window();
        #[cfg(target_os = "macos")]
        self.prepare_front_app_for_visible_session(restore_target);
        true
    }

    fn create_temporary_window(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        let target_display_id = target_display_id(cx);
        match cx.open_window(
            WindowOptions {
                kind: WindowKind::PopUp,
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(-100.), px(-100.))),
                }),
                display_id: target_display_id,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    target_display_id,
                    TEMPORARY_WINDOW_SIZE,
                    cx,
                ))),
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| TemporaryView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(handle) => Some(handle),
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Failed to open temporary window");
                None
            }
        }
    }

    pub(super) fn ensure_temporary_window_visible(
        &mut self,
        cx: &mut App,
    ) -> Option<WindowHandle<Root>> {
        self.ensure_temporary_window_visible_with_restore_target(
            cx,
            FocusRestoreTarget::ExistingOrRecordCurrent,
        )
    }

    pub(super) fn ensure_temporary_window_visible_with_restore_target(
        &mut self,
        cx: &mut App,
        restore_target: FocusRestoreTarget,
    ) -> Option<WindowHandle<Root>> {
        let window =
            Self::find_temporary_window(cx).or_else(|| self.create_temporary_window(cx))?;
        let mut restore_target = Some(restore_target);
        match window.update(cx, |root, window, cx| {
            let restore_target = restore_target
                .take()
                .expect("restore target should be consumed by temporary window update");
            if !self.show_temporary_window_on_mouse_display(window, cx, restore_target) {
                return false;
            }
            focus_temporary_window_chat_form(root, window, cx);
            true
        }) {
            Ok(true) => Some(window),
            Ok(false) => None,
            Err(err) => {
                if let Some(restore_target) = restore_target.take() {
                    restore_target.restore_if_override();
                }
                event!(Level::ERROR, "Failed to update temporary window: {:?}", err);
                None
            }
        }
    }

    pub(super) fn toggle_temporary_window(&mut self, cx: &mut App) {
        match Self::find_temporary_window(cx) {
            Some(temporary_window) => {
                if let Err(err) = temporary_window.update(cx, |_this, window, cx| {
                    if window.is_visible().unwrap_or(false) {
                        self.delay_or_hide_temporary_window(window, cx);
                    } else {
                        self.show_temporary_window_on_mouse_display(
                            window,
                            cx,
                            FocusRestoreTarget::ExistingOrRecordCurrent,
                        );
                    }
                }) {
                    event!(Level::ERROR, "Failed to update temporary window: {:?}", err);
                };
            }
            None => {
                let _ = self.ensure_temporary_window_visible(cx);
            }
        }
    }
}

impl GlobalHotkeyState {
    pub(super) fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
        TemporaryWindowState::find_temporary_window(cx)
    }

    pub(super) fn delay_or_hide_temporary_window(&mut self, window: &mut Window, cx: &mut App) {
        let _ = with_temporary_window_state(cx, |state, cx| {
            state.delay_or_hide_temporary_window(window, cx);
        });
    }

    pub(crate) fn request_hide_with_delay(window: &mut Window, cx: &mut App) {
        TemporaryWindowState::request_hide_with_delay(window, cx);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn record_front_app_for_screenshot(&mut self) {
        if self.front_app.is_none() {
            self.front_app = record_frontmost_app();
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn clear_front_app_for_screenshot(&mut self) {
        self.front_app = None;
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn restore_and_clear_front_app_for_screenshot(&mut self) {
        restore_frontmost_app(&self.front_app);
        self.front_app = None;
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn take_front_app_for_screenshot(
        &mut self,
    ) -> Option<Retained<NSRunningApplication>> {
        self.front_app.take()
    }

    pub(super) fn ensure_temporary_window_visible_with_restore_target(
        &mut self,
        cx: &mut App,
        restore_target: FocusRestoreTarget,
    ) -> Option<WindowHandle<Root>> {
        with_temporary_window_state(cx, |state, cx| {
            state.ensure_temporary_window_visible_with_restore_target(cx, restore_target)
        })
        .flatten()
    }

    pub(super) fn toggle_temporary_window(&mut self, cx: &mut App) {
        let _ = with_temporary_window_state(cx, |state, cx| {
            state.toggle_temporary_window(cx);
        });
    }
}

pub(crate) fn open_temporary_window(cx: &mut App) {
    let _ = with_temporary_window_state(cx, |state, cx| {
        state.ensure_temporary_window_visible(cx);
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn record_front_app_for_temporary_window(cx: &mut App) {
    let _ = with_temporary_window_state(cx, |state, _cx| {
        state.record_front_app();
    });
}

pub(crate) fn toggle_temporary_window(cx: &mut App) {
    let _ = with_temporary_window_state(cx, |state, cx| {
        state.toggle_temporary_window(cx);
    });
}
