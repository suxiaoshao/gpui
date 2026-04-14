use super::*;

impl GlobalHotkeyState {
    pub fn delay_close(window: &mut Window, cx: &mut App) -> Task<()> {
        window.spawn(cx, async |cx| {
            Timer::after(Duration::from_secs(600)).await;
            if let Err(err) = cx.window_handle().update(cx, |_, window, _cx| {
                window.remove_window();
            }) {
                event!(Level::ERROR, "Failed to remove temporary window: {:?}", err);
            };
        })
    }

    pub(super) fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
        cx.windows().iter().find_map(|window| {
            window.downcast::<Root>().filter(|root| {
                root.read(cx)
                    .ok()
                    .map(|root| root.view().entity_type() == TypeId::of::<TemporaryView>())
                    .unwrap_or(false)
            })
        })
    }

    pub(super) fn delay_or_hide_temporary_window(&mut self, window: &mut Window, cx: &mut App) {
        let task = Self::delay_close(window, cx);
        self.delay_close = Some(task);
        self.hide_temporary_window(window);
    }

    pub fn request_hide_with_delay(window: &mut Window, cx: &mut App) {
        if !cx.has_global::<GlobalHotkeyState>() {
            event!(
                Level::ERROR,
                "Failed to hide temporary window with delay: GlobalHotkeyState is not initialized"
            );
            return;
        }

        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
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

    fn show_temporary_window_on_mouse_display(&mut self, window: &mut Window, cx: &App) {
        self.delay_close = None;
        #[cfg(target_os = "macos")]
        if self.front_app.is_none() {
            self.front_app = record_frontmost_app();
        }
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
        if let Err(err) = window.show() {
            window.activate_window();
            event!(Level::ERROR, "Failed to show temporary window: {:?}", err);
        };
        window.activate_window();
    }

    fn create_temporary_window(&mut self, cx: &mut App) -> Option<WindowHandle<Root>> {
        #[cfg(target_os = "macos")]
        if self.front_app.is_none() {
            self.front_app = record_frontmost_app();
        }
        let target_display_id = target_display_id(cx);
        match cx.open_window(
            WindowOptions {
                kind: WindowKind::Floating,
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
                window.activate_window();
                if let Err(err) = window.set_floating() {
                    event!(Level::ERROR, error = ?err, "Failed to set floating");
                }
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
        let window =
            Self::find_temporary_window(cx).or_else(|| self.create_temporary_window(cx))?;
        let _ = window.update(cx, |_, window, cx| {
            self.show_temporary_window_on_mouse_display(window, cx);
        });
        Some(window)
    }

    pub(super) fn toggle_temporary_window(&mut self, cx: &mut App) {
        match Self::find_temporary_window(cx) {
            Some(temporary_window) => {
                if let Err(err) = temporary_window.update(cx, |_this, window, cx| {
                    if window.is_visible().unwrap_or(false) {
                        self.delay_or_hide_temporary_window(window, cx);
                    } else {
                        self.show_temporary_window_on_mouse_display(window, cx);
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
