use gpui::{App, Bounds, Pixels, PlatformDisplay, Point, point, px};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
struct DisplaySnapshot {
    id: u64,
    bounds: Bounds<Pixels>,
    is_primary: bool,
}

pub(crate) fn target_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
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

#[cfg(test)]
mod tests {
    use super::{DisplaySnapshot, display_id_for_mouse_location};
    use gpui::{Bounds, point, px, size};

    fn snapshot(
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
            snapshot(1, 0.0, 0.0, 1440.0, 900.0, true),
            snapshot(2, 1440.0, 0.0, 1920.0, 1080.0, false),
        ];

        assert_eq!(
            display_id_for_mouse_location(&displays, point(px(1800.0), px(300.0))),
            Some(2)
        );
    }

    #[test]
    fn mouse_location_falls_back_to_primary_display() {
        let displays = vec![
            snapshot(1, 0.0, 0.0, 1440.0, 900.0, true),
            snapshot(2, 1440.0, 0.0, 1920.0, 1080.0, false),
        ];

        assert_eq!(
            display_id_for_mouse_location(&displays, point(px(-100.0), px(-100.0))),
            Some(1)
        );
    }
}
