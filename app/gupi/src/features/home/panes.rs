use super::*;

const CONTENT_MIN: f32 = 360.;
const LEFT_MIN: f32 = 160.;
const LEFT_MAX: f32 = 480.;
const RIGHT_MIN: f32 = 240.;
const RIGHT_MAX: f32 = 520.;

#[derive(Clone, Copy)]
pub(super) enum Side {
    Left,
    Right,
}

/// Displayed widths are independent of persisted preferences. Re-fit them only
/// when the available space or visible panels change, never after a sibling drag.
#[derive(Default)]
pub(super) struct PaneLayout {
    container: Option<(f32, bool, bool)>,
    pub left: f32,
    pub right: f32,
    pub overlay: bool,
}

impl PaneLayout {
    pub fn fit(
        &mut self,
        width: f32,
        left: bool,
        right: bool,
        preferences: &layout::LayoutState,
    ) -> bool {
        let container = (width, left, right);
        if self.container == Some(container) {
            return false;
        }
        self.container = Some(container);
        self.overlay = right && width < 1100.;
        let right_min = if right && !self.overlay {
            RIGHT_MIN
        } else {
            0.
        };
        self.left = if left {
            preferences
                .sidebar_width
                .min((width - CONTENT_MIN - right_min).max(0.))
        } else {
            0.
        };
        self.right = if right {
            preferences.history_width.min(if self.overlay {
                (width - 100.).max(0.)
            } else {
                (width - CONTENT_MIN - self.left).max(0.)
            })
        } else {
            0.
        };
        true
    }

    pub fn width(&self, side: Side) -> f32 {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }

    fn drag_to(&mut self, side: Side, x: f32) {
        let Some((width, _, _)) = self.container else {
            return;
        };
        self.resize(
            side,
            match side {
                Side::Left => x,
                Side::Right => width - x,
            },
        );
    }

    pub fn resize(&mut self, side: Side, requested: f32) {
        let Some((width, _, _)) = self.container else {
            return;
        };
        let (minimum, maximum) = match side {
            Side::Left => (
                LEFT_MIN,
                (width - CONTENT_MIN - if self.overlay { 0. } else { self.right }).min(LEFT_MAX),
            ),
            Side::Right => (
                RIGHT_MIN,
                if self.overlay {
                    width - 100.
                } else {
                    width - CONTENT_MIN - self.left
                }
                .min(RIGHT_MAX),
            ),
        };
        let value = requested.clamp(minimum.min(maximum.max(0.)), maximum.max(0.));
        match side {
            Side::Left => self.left = value,
            Side::Right => self.right = value,
        }
    }
}

pub(super) struct Drag {
    side: Side,
    start_width: f32,
}

struct DragPreview;
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl HomeView {
    pub(super) fn pane_handle(&self, side: Side, cx: &Context<Self>) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        gpui_kit::base::resize_handle(
            match side {
                Side::Left => "sidebar-resize",
                Side::Right => "history-resize",
            },
            Axis::Horizontal,
        )
        .on_drag(side, move |_, _, window, cx| {
            let _ = owner.update(cx, |this, cx| {
                this.pane_drag = Some(Drag {
                    side,
                    start_width: this.pane_layout.width(side),
                });
                this.pane_layout
                    .drag_to(side, f32::from(window.mouse_position().x));
                cx.notify();
            });
            cx.new(|_| DragPreview)
        })
    }

    fn finish_pane_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.pane_drag.take() else {
            return;
        };
        let width = self.pane_layout.width(drag.side);
        if (width - drag.start_width).abs() > 0.01 {
            match drag.side {
                Side::Left => self.save_layout(Some(width), None, window, cx),
                Side::Right => self.save_layout(None, Some(width), window, cx),
            }
        }
        cx.notify();
    }
}

/// Listen across the window so releasing outside the narrow handle still ends
/// the gesture. The kit owns handle appearance, hit testing and drag initiation.
pub(super) struct ResizeEvents(pub WeakEntity<HomeView>);
impl IntoElement for ResizeEvents {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ResizeEvents {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (window.request_layout(Style::default(), None, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let owner = self.0.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if !phase.bubble() || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let _ = owner.update(cx, |this, cx| {
                if let Some(drag) = &this.pane_drag {
                    this.pane_layout
                        .drag_to(drag.side, f32::from(event.position.x));
                    cx.notify();
                    cx.stop_propagation();
                }
            });
        });
        let owner = self.0.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.bubble() || event.button != MouseButton::Left {
                return;
            }
            let _ = owner.update(cx, |this, cx| this.finish_pane_drag(window, cx));
        });
        let owner = self.0.clone();
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if !phase.capture() || event.keystroke.key != "escape" {
                return;
            }
            let _ = owner.update(cx, |this, cx| {
                if this.pane_drag.take().is_some() {
                    this.pane_layout.container = None;
                    cx.stop_active_drag(window);
                    cx.notify();
                    cx.stop_propagation();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneLayout, Side};
    use crate::state::layout;
    #[test]
    fn resizing_one_sidebar_never_borrows_from_the_other() {
        let preferences = layout::LayoutState {
            sidebar_width: 300.,
            history_width: 300.,
            ..Default::default()
        };
        let mut panes = PaneLayout::default();
        panes.fit(1200., true, true, &preferences);
        panes.resize(Side::Right, 600.);
        assert_eq!((panes.left, panes.right), (300., 520.));
        panes.resize(Side::Left, 480.);
        assert_eq!((panes.left, panes.right), (320., 520.));
        panes.resize(Side::Right, 100.);
        assert_eq!((panes.left, panes.right), (320., 240.));
    }
    #[test]
    fn temporary_constraints_and_sibling_drag_do_not_restore_or_overwrite_preferences() {
        let preferences = layout::LayoutState {
            sidebar_width: 480.,
            history_width: 520.,
            ..Default::default()
        };
        let mut panes = PaneLayout::default();
        panes.fit(1100., true, true, &preferences);
        assert_eq!((panes.left, panes.right), (480., 260.));
        panes.resize(Side::Left, 400.);
        // A redraw after dragging the left edge must not expand the right edge.
        assert!(!panes.fit(1100., true, true, &preferences));
        assert_eq!((panes.left, panes.right), (400., 260.));
        panes.fit(1500., true, true, &preferences);
        assert_eq!((panes.left, panes.right), (480., 520.));
        assert_eq!(
            (preferences.sidebar_width, preferences.history_width),
            (480., 520.)
        );
    }
    #[test]
    fn overlay_and_visibility_changes_keep_the_original_pixel_preferences() {
        let preferences = layout::LayoutState {
            sidebar_width: 480.,
            history_width: 520.,
            ..Default::default()
        };
        let mut panes = PaneLayout::default();
        panes.fit(800., true, true, &preferences);
        assert!(panes.overlay);
        assert_eq!((panes.left, panes.right), (440., 520.));
        panes.resize(Side::Right, 300.);
        assert_eq!(panes.left, 440.);
        panes.fit(1500., true, false, &preferences);
        assert_eq!((panes.left, panes.right), (480., 0.));
        panes.fit(1500., true, true, &preferences);
        assert_eq!((panes.left, panes.right), (480., 520.));
    }
}
