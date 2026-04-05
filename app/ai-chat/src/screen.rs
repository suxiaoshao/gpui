use gpui::{App, Bounds, DisplayId, Pixels, PlatformDisplay, Point, Size, point, px, size};
use std::rc::Rc;

pub(crate) const TEMPORARY_WINDOW_SIZE: Size<Pixels> = size(px(800.), px(600.));

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplaySnapshot {
    pub id: u32,
    pub bounds: Bounds<Pixels>,
    pub is_primary: bool,
}

pub(crate) fn target_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    let displays = cx.displays();
    if displays.is_empty() {
        return None;
    }

    if let Some(display_id) = platform_ext::app::current_mouse_display_id()
        && let Some(display) = displays
            .iter()
            .find(|display| u32::from(display.id()) == display_id)
    {
        return Some(display.clone());
    }

    let snapshots = display_snapshots(cx);
    if let Some((x, y)) = platform_ext::app::current_mouse_location()
        && let Some(display_id) = display_id_for_mouse_location(&snapshots, point(px(x), px(y)))
        && let Some(display) = displays
            .iter()
            .find(|display| u32::from(display.id()) == display_id)
    {
        return Some(display.clone());
    }

    cx.primary_display().or_else(|| displays.into_iter().next())
}

pub(crate) fn target_display_id(cx: &App) -> Option<DisplayId> {
    target_display(cx).map(|display| display.id())
}

pub(crate) fn centered_bounds_for_display(
    display_id: Option<DisplayId>,
    size: Size<Pixels>,
    cx: &App,
) -> Bounds<Pixels> {
    Bounds::centered(display_id, size, cx)
}

pub(crate) fn recentered_bounds_for_display(
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

    centered_bounds_for_display(display_id, size, cx)
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

fn display_snapshots(cx: &App) -> Vec<DisplaySnapshot> {
    let primary_id = cx.primary_display().map(|display| u32::from(display.id()));
    cx.displays()
        .into_iter()
        .map(|display| DisplaySnapshot {
            id: u32::from(display.id()),
            bounds: display.bounds(),
            is_primary: primary_id == Some(u32::from(display.id())),
        })
        .collect()
}

fn display_id_for_mouse_location(
    displays: &[DisplaySnapshot],
    mouse_location: Point<Pixels>,
) -> Option<u32> {
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
    use super::{
        DisplaySnapshot, TEMPORARY_WINDOW_SIZE, display_id_for_mouse_location,
        preserve_or_fallback_size,
    };
    use gpui::{Bounds, point, px, size};

    fn snapshot(
        id: u32,
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

    #[test]
    fn preserve_existing_size_when_it_fits_target_display() {
        let current = size(px(960.0), px(720.0));
        let display = size(px(1920.0), px(1080.0));

        assert_eq!(
            preserve_or_fallback_size(current, display, TEMPORARY_WINDOW_SIZE),
            current
        );
    }

    #[test]
    fn fallback_size_is_used_when_existing_window_is_too_large() {
        let current = size(px(2400.0), px(1400.0));
        let display = size(px(1920.0), px(1080.0));

        assert_eq!(
            preserve_or_fallback_size(current, display, TEMPORARY_WINDOW_SIZE),
            TEMPORARY_WINDOW_SIZE
        );
    }
}
