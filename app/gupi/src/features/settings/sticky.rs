use crate::foundation::i18n::t;
use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::*;

pub(super) const HEADER_HEIGHT: f32 = 32.;

pub(super) fn heading(key: &'static str, cx: &App) -> Div {
    h_flex()
        .h(px(HEADER_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .block_mouse_except_scroll()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .font_weight(FontWeight::MEDIUM)
        .child(t(cx, key))
}

// Resolve against this frame's unscrolled child bounds. No render-time cache or
// wheel-event sampling: dragging the scrollbar and programmatic scrolling agree.
pub(super) fn overlay(scroll: ScrollHandle) -> impl IntoElement {
    canvas(
        move |_, window, cx| {
            let viewport = scroll.bounds();
            let light = scroll.bounds_for_item(1)?.top() + scroll.offset().y;
            let dark = scroll.bounds_for_item(2)?.top() + scroll.offset().y;
            let (key, y) = pinned_header(light, dark, viewport.top(), px(HEADER_HEIGHT))?;
            let mask = ContentMask {
                bounds: viewport.intersect(&window.content_mask().bounds),
            };
            let mut header = heading(key, cx).into_any_element();
            header.layout_as_root(
                size(
                    AvailableSpace::Definite(viewport.size.width),
                    AvailableSpace::Definite(px(HEADER_HEIGHT)),
                ),
                window,
                cx,
            );
            window.with_content_mask(Some(mask), |window| {
                header.prepaint_at(point(viewport.left(), y), window, cx);
            });
            Some((header, mask))
        },
        |_, overlay: Option<(AnyElement, ContentMask<Pixels>)>, window, cx| {
            if let Some((mut header, mask)) = overlay {
                window.with_content_mask(Some(mask), |window| header.paint(window, cx));
            }
        },
    )
    .absolute()
    .size_full()
}

fn pinned_header(
    light: Pixels,
    dark: Pixels,
    top: Pixels,
    height: Pixels,
) -> Option<(&'static str, Pixels)> {
    if dark <= top {
        Some(("dark-themes", top))
    } else if light < top {
        Some(("light-themes", top.min(dark - height)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pinned_header;
    use gpui_kit::px;
    #[test]
    fn headers_pin_and_push_in_both_scroll_directions() {
        assert_eq!(pinned_header(px(80.), px(500.), px(40.), px(32.)), None);
        assert_eq!(
            pinned_header(px(0.), px(500.), px(40.), px(32.)),
            Some(("light-themes", px(40.)))
        );
        assert_eq!(
            pinned_header(px(-400.), px(55.), px(40.), px(32.)),
            Some(("light-themes", px(23.)))
        );
        assert_eq!(
            pinned_header(px(-450.), px(40.), px(40.), px(32.)),
            Some(("dark-themes", px(40.)))
        );
        assert_eq!(
            pinned_header(px(-450.), px(41.), px(40.), px(32.)),
            Some(("light-themes", px(9.)))
        );
    }
}
