use crate::{
    platform::{
        capture::{CaptureDisplay, CaptureError, CaptureRect, capture_region},
        display::target_display,
    },
    database::GlobalShortcutBinding,
    hotkey::GlobalHotkeyState,
};
use gpui::*;
use std::cmp::{max, min};
use std::time::Duration;
use tracing::{Level, event};
use window_ext::WindowExt;

actions!(screenshot_overlay, [CancelScreenshotSelection]);

const DRAG_THRESHOLD: f32 = 2.0;
const CAPTURE_START_DELAY: Duration = Duration::from_millis(30);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelScreenshotSelection, None)]);
}

pub(crate) fn is_active(cx: &App) -> bool {
    cx.try_global::<ScreenshotOverlayState>()
        .is_some_and(|state| state.session.is_some())
}

pub(crate) fn open(binding: GlobalShortcutBinding, cx: &mut App) -> Result<(), CaptureError> {
    if is_active(cx) {
        return Err(CaptureError::BackendUnavailable(
            "screenshot selection is already active",
        ));
    }

    let primary_id = cx.primary_display().map(|display| u32::from(display.id()));
    let mut handles = Vec::new();
    let Some(display) = target_display(cx) else {
        return Err(CaptureError::BackendUnavailable(
            "no displays are available",
        ));
    };
    event!(
        Level::INFO,
        binding_id = binding.id,
        displays = cx.displays().len(),
        "Opening screenshot overlay session"
    );
    let bounds = Bounds::new(point(px(0.), px(0.)), display.bounds().size);
    let display_id = display.id();
    let display_info = CaptureDisplay {
        id_hint: u32::from(display_id),
        origin: display.bounds().origin,
        width_px: 0,
        height_px: 0,
        scale_factor: 0.0,
        is_primary: primary_id == Some(u32::from(display_id)),
    };
    event!(
        Level::INFO,
        display_id = display_info.id_hint,
        is_primary = display_info.is_primary,
        width = f32::from(bounds.size.width),
        height = f32::from(bounds.size.height),
        "Creating screenshot overlay window"
    );

    let handle = cx.open_window(
        WindowOptions {
            display_id: Some(display_id),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::Floating,
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(point(px(-100.), px(-100.))),
            }),
            window_background: WindowBackgroundAppearance::Transparent,
            focus: true,
            show: true,
            is_movable: false,
            is_resizable: false,
            window_min_size: None,
            window_decorations: None,
            ..Default::default()
        },
        move |window, cx| {
            if let Err(err) = window.set_floating() {
                event!(
                    Level::ERROR,
                    display_id = display_info.id_hint,
                    error = ?err,
                    "Failed to set screenshot overlay window floating"
                );
            }
            window.activate_window();
            let display_id = display_info.id_hint;
            event!(
                Level::INFO,
                display_id,
                scale_factor = window.scale_factor(),
                "Screenshot overlay window opened"
            );
            cx.new(|cx| ScreenshotOverlayView::new(display_info.clone(), window, cx))
        },
    );

    match handle {
        Ok(handle) => handles.push(handle),
        Err(err) => {
            for h in handles {
                let _ = h.update(cx, |_, window, _cx| window.remove_window());
            }
            #[cfg(target_os = "macos")]
            cx.update_global::<GlobalHotkeyState, _>(|state, _cx| {
                state.clear_front_app_for_screenshot();
            });
            return Err(CaptureError::SystemFailure(err.to_string()));
        }
    }

    cx.set_global(ScreenshotOverlayState {
        session: Some(ScreenshotOverlaySession { binding, handles }),
    });
    Ok(())
}

#[derive(Default)]
struct ScreenshotOverlayState {
    session: Option<ScreenshotOverlaySession>,
}

impl Global for ScreenshotOverlayState {}

struct ScreenshotOverlaySession {
    binding: GlobalShortcutBinding,
    handles: Vec<WindowHandle<ScreenshotOverlayView>>,
}

impl ScreenshotOverlaySession {
    fn retain_active_window(&mut self, current: WindowHandle<ScreenshotOverlayView>, cx: &mut App) {
        self.handles.retain(|handle| {
            if *handle == current {
                true
            } else {
                let _ = handle.update(cx, |_view, window, _cx| {
                    window.remove_window();
                });
                false
            }
        });
    }

    fn close_all(self, cx: &mut App) {
        for handle in self.handles {
            let _ = handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        }
    }
}

pub(crate) struct ScreenshotOverlayView {
    display: CaptureDisplay,
    drag_origin: Option<Point<Pixels>>,
    drag_current: Option<Point<Pixels>>,
    selection_started: bool,
    focus_handle: FocusHandle,
}

fn offset_capture_rect(
    rect: CaptureRect,
    window_origin: Point<Pixels>,
    scale_factor: f32,
    display: &CaptureDisplay,
) -> Option<CaptureRect> {
    let offset_x = logical_to_capture_delta(
        f32::from(window_origin.x) - f32::from(display.origin.x),
        scale_factor,
    );
    let offset_y = logical_to_capture_delta(
        f32::from(window_origin.y) - f32::from(display.origin.y),
        scale_factor,
    );
    let x_px = rect.x_px.checked_add_signed(offset_x)?;
    let y_px = rect.y_px.checked_add_signed(offset_y)?;
    let rect = CaptureRect {
        x_px,
        y_px,
        width_px: rect.width_px,
        height_px: rect.height_px,
    };

    (rect.x_px + rect.width_px <= display.width_px
        && rect.y_px + rect.height_px <= display.height_px)
        .then_some(rect)
}

impl ScreenshotOverlayView {
    fn new(display: CaptureDisplay, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self {
            display,
            drag_origin: None,
            drag_current: None,
            selection_started: false,
            focus_handle,
        }
    }

    fn on_cancel(
        &mut self,
        _: &CancelScreenshotSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cancel_overlay(window, cx);
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Right {
            cancel_overlay(window, cx);
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }

        let Some(handle) = window.window_handle().downcast::<ScreenshotOverlayView>() else {
            cancel_overlay(window, cx);
            return;
        };
        let display = resolved_display(window, &self.display);
        cx.update_global::<ScreenshotOverlayState, _>(|state, cx| {
            if let Some(session) = state.session.as_mut() {
                session.retain_active_window(handle, cx);
            }
        });

        self.display = display;
        self.drag_origin = Some(event.position);
        self.drag_current = Some(event.position);
        self.selection_started = false;
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(origin) = self.drag_origin else {
            return;
        };
        self.drag_current = Some(event.position);
        if !self.selection_started && drag_distance(origin, event.position) > DRAG_THRESHOLD {
            self.selection_started = true;
        }
        cx.notify();
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || self.drag_origin.is_none() {
            return;
        }
        let selection_started = self.selection_started;
        self.selection_started = false;
        self.drag_current = Some(event.position);
        if !selection_started {
            event!(
                Level::INFO,
                display_id = self.display.id_hint,
                "Screenshot selection cancelled by click without drag"
            );
            cancel_overlay(window, cx);
            return;
        }
        let Some(rect) = selection_rect(
            self.drag_origin,
            self.drag_current,
            window.scale_factor(),
            window.bounds().origin,
            &self.display,
        ) else {
            event!(
                Level::INFO,
                display_id = self.display.id_hint,
                "Screenshot selection cancelled because computed rect was empty"
            );
            cancel_overlay(window, cx);
            return;
        };

        event!(
            Level::INFO,
            display_id = self.display.id_hint,
            x_px = rect.x_px,
            y_px = rect.y_px,
            width_px = rect.width_px,
            height_px = rect.height_px,
            "Submitting screenshot selection"
        );

        let display = self.display.clone();
        let binding = cx
            .global::<ScreenshotOverlayState>()
            .session
            .as_ref()
            .map(|session| session.binding.clone());

        // Take the session out of global state. Do NOT call close_all here because that
        // tries to close the current window via handle.update, which is a GPUI reentrancy
        // error (we are already inside this window's event handler). The error is silently
        // swallowed by `let _ =` inside close_all, so the window would never actually close.
        // Other overlay windows were already closed by retain_active_window in mouse_down,
        // so taking the session is sufficient to clean up global state.
        cx.update_global::<ScreenshotOverlayState, _>(|state, _cx| {
            state.session.take();
        });
        let Some(binding) = binding else {
            return;
        };

        // Remove the current overlay window directly — this is the only safe way to do it
        // from within the window's own event handler.
        window.remove_window();

        // Use cx.spawn (App context) rather than window.spawn (window context) because the
        // overlay window has just been closed and its context will be invalid by the time the
        // async task runs after CAPTURE_START_DELAY.
        cx.spawn(async move |_this, cx| {
            Timer::after(CAPTURE_START_DELAY).await;
            let display_id = display.id_hint;
            let captured_display = display.clone();
            let captured = smol::unblock(move || capture_region(&captured_display, rect)).await;
            match captured {
                Ok(image) => {
                    event!(
                        Level::INFO,
                        display_id,
                        width = image.width,
                        height = image.height,
                        bytes_len = image.bytes_rgba8.len(),
                        "Screenshot selection capture completed"
                    );
                    let _ = cx.update_global::<GlobalHotkeyState, _>(|_hotkeys, cx| {
                        GlobalHotkeyState::process_captured_screenshot(binding.clone(), image, cx);
                    });
                }
                Err(err) => {
                    let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                        hotkeys.handle_screenshot_capture_failure(err, cx);
                    });
                }
            }
        })
        .detach();
    }
}

impl Render for ScreenshotOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("screenshot-overlay-root")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(gpui::black().opacity(0.35))
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_action(cx.listener(Self::on_cancel));

        if let Some(bounds) = selection_bounds(self.drag_origin, self.drag_current) {
            root = root.child(
                div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .border_1()
                    .border_color(gpui::white())
                    .bg(gpui::white().opacity(0.08)),
            );
        }

        root
    }
}

fn cancel_overlay(window: &mut Window, cx: &mut Context<ScreenshotOverlayView>) {
    cx.update_global::<ScreenshotOverlayState, _>(|state, cx| {
        if let Some(session) = state.session.take() {
            session.close_all(cx);
        }
    });
    #[cfg(target_os = "macos")]
    cx.update_global::<GlobalHotkeyState, _>(|state, _cx| {
        state.restore_and_clear_front_app_for_screenshot();
    });
    window.remove_window();
}

fn drag_distance(origin: Point<Pixels>, current: Point<Pixels>) -> f32 {
    let dx = f32::from(current.x) - f32::from(origin.x);
    let dy = f32::from(current.y) - f32::from(origin.y);
    (dx * dx + dy * dy).sqrt()
}

/// Convert a GPUI logical pixel coordinate to the unit xcap expects for monitor
/// matching and capture_region coordinates.
///
/// - macOS: xcap uses `CGDisplayBounds` which returns logical pixels (points),
///   so no conversion is needed.
/// - Windows (and others): xcap uses `dmPelsWidth`/`dmPelsHeight` which are
///   physical pixels, so we multiply by the DPI scale factor.
fn logical_to_capture_coord(logical: f32, scale_factor: f32) -> u32 {
    #[cfg(target_os = "macos")]
    let _ = scale_factor;
    #[cfg(target_os = "macos")]
    return logical.round() as u32;

    #[cfg(not(target_os = "macos"))]
    return (logical * scale_factor).round() as u32;
}

fn logical_to_capture_delta(logical: f32, scale_factor: f32) -> i32 {
    #[cfg(target_os = "macos")]
    let _ = scale_factor;
    #[cfg(target_os = "macos")]
    return logical.round() as i32;

    #[cfg(not(target_os = "macos"))]
    return (logical * scale_factor).round() as i32;
}

fn resolved_display(window: &Window, display: &CaptureDisplay) -> CaptureDisplay {
    let size = window.bounds().size;
    let scale_factor = window.scale_factor();
    CaptureDisplay {
        id_hint: display.id_hint,
        origin: display.origin,
        width_px: logical_to_capture_coord(f32::from(size.width), scale_factor),
        height_px: logical_to_capture_coord(f32::from(size.height), scale_factor),
        scale_factor,
        is_primary: display.is_primary,
    }
}

fn selection_rect_in_overlay_coords(
    origin: Option<Point<Pixels>>,
    current: Option<Point<Pixels>>,
    scale_factor: f32,
) -> Option<CaptureRect> {
    let bounds = selection_bounds(origin, current)?;
    let width_px = logical_to_capture_coord(f32::from(bounds.size.width), scale_factor);
    let height_px = logical_to_capture_coord(f32::from(bounds.size.height), scale_factor);
    if width_px == 0 || height_px == 0 {
        return None;
    }

    Some(CaptureRect {
        x_px: logical_to_capture_coord(f32::from(bounds.origin.x), scale_factor),
        y_px: logical_to_capture_coord(f32::from(bounds.origin.y), scale_factor),
        width_px,
        height_px,
    })
}

fn selection_rect(
    origin: Option<Point<Pixels>>,
    current: Option<Point<Pixels>>,
    scale_factor: f32,
    window_origin: Point<Pixels>,
    display: &CaptureDisplay,
) -> Option<CaptureRect> {
    let rect = selection_rect_in_overlay_coords(origin, current, scale_factor)?;
    offset_capture_rect(rect, window_origin, scale_factor, display)
}

fn selection_bounds(
    origin: Option<Point<Pixels>>,
    current: Option<Point<Pixels>>,
) -> Option<Bounds<Pixels>> {
    let (origin, current) = (origin?, current?);
    let left = px(min(f32::from(origin.x) as i32, f32::from(current.x) as i32) as f32);
    let top = px(min(f32::from(origin.y) as i32, f32::from(current.y) as i32) as f32);
    let right = px(max(f32::from(origin.x) as i32, f32::from(current.x) as i32) as f32);
    let bottom = px(max(f32::from(origin.y) as i32, f32::from(current.y) as i32) as f32);
    Some(Bounds::new(
        point(left, top),
        size(right - left, bottom - top),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        DRAG_THRESHOLD, drag_distance, logical_to_capture_coord, logical_to_capture_delta,
        selection_bounds, selection_rect, selection_rect_in_overlay_coords,
    };
    use crate::platform::capture::{CaptureDisplay, CaptureRect};
    use gpui::{point, px};

    fn display(origin_x: f32, origin_y: f32, height_px: u32) -> CaptureDisplay {
        CaptureDisplay {
            id_hint: 1,
            origin: point(px(origin_x), px(origin_y)),
            width_px: 1920,
            height_px,
            scale_factor: 1.0,
            is_primary: true,
        }
    }

    #[test]
    fn selection_rect_normalizes_drag_direction() {
        let rect = selection_rect(
            Some(point(px(200.0), px(160.0))),
            Some(point(px(20.0), px(40.0))),
            1.0,
            point(px(0.0), px(0.0)),
            &display(0.0, 0.0, 1080),
        )
        .expect("drag selection should produce a rect");

        assert_eq!(rect.x_px, 20);
        assert_eq!(rect.y_px, 40);
        assert_eq!(rect.width_px, 180);
        assert_eq!(rect.height_px, 120);
    }

    #[test]
    fn selection_rect_offsets_by_window_origin_relative_to_display_origin() {
        let rect = selection_rect(
            Some(point(px(20.0), px(40.0))),
            Some(point(px(200.0), px(160.0))),
            2.0,
            point(px(1440.0), px(12.0)),
            &display(1440.0, 0.0, 1080),
        )
        .expect("drag selection should produce a rect");

        assert_eq!(
            rect,
            CaptureRect {
                x_px: logical_to_capture_coord(20.0, 2.0),
                y_px: logical_to_capture_coord(40.0, 2.0)
                    + logical_to_capture_delta(12.0, 2.0) as u32,
                width_px: logical_to_capture_coord(180.0, 2.0),
                height_px: logical_to_capture_coord(120.0, 2.0),
            }
        );
    }

    #[test]
    fn selection_bounds_returns_none_without_complete_drag_points() {
        assert!(selection_bounds(None, Some(point(px(10.0), px(20.0)))).is_none());
    }

    #[test]
    fn drag_distance_stays_below_threshold_for_clicks() {
        let distance = drag_distance(point(px(10.0), px(10.0)), point(px(11.0), px(11.0)));
        assert!(distance < DRAG_THRESHOLD);
    }

    #[test]
    fn selection_rect_overlay_coords_keep_top_left_origin() {
        let rect = selection_rect_in_overlay_coords(
            Some(point(px(200.0), px(160.0))),
            Some(point(px(20.0), px(40.0))),
            1.0,
        )
        .expect("overlay rect should exist");

        assert_eq!(
            rect,
            CaptureRect {
                x_px: 20,
                y_px: 40,
                width_px: 180,
                height_px: 120,
            }
        );
    }
}
