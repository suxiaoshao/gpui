//! A bounded activity list with native scrolling and theme-colored edge fades.
use gpui_kit::component::{
    ActiveTheme,
    scroll::{ScrollableMask, Scrollbar},
};
use gpui_kit::*;

#[derive(IntoElement)]
pub(super) struct ActivityViewport {
    id: String,
    content: AnyElement,
}
impl ActivityViewport {
    pub fn new(id: String, content: AnyElement) -> Self {
        Self { id, content }
    }
}
impl RenderOnce for ActivityViewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scroll = window
            .use_keyed_state(format!("activity-scroll-{}", self.id), cx, |_, _| {
                ScrollHandle::new()
            })
            .read(cx)
            .clone();
        div()
            .relative()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .id(self.id.clone())
                    .w_full()
                    .min_w_0()
                    .max_h(px(224.))
                    .overflow_hidden()
                    .track_scroll(&scroll)
                    .child(self.content),
            )
            .child(ScrollableMask::new(Axis::Vertical, &scroll).id(self.id.clone()))
            .child(edge_fades(scroll.clone(), cx.theme().background))
            .child(Scrollbar::vertical(&scroll))
    }
}

fn edge_fades(scroll: ScrollHandle, background: Hsla) -> impl IntoElement {
    canvas(
        move |_, _, _| {
            let bounds = scroll.bounds();
            let offset = -f32::from(scroll.offset().y);
            let max = f32::from(scroll.max_offset().y);
            (bounds, fade_heights(offset, max))
        },
        move |_, (bounds, (top, bottom)), window, _| {
            let mask = ContentMask {
                bounds: bounds.intersect(&window.content_mask().bounds),
            };
            window.with_content_mask(Some(mask), |window| {
                for (height, y, angle) in [
                    (top, bounds.top(), 180.),
                    (bottom, bounds.bottom() - px(bottom), 0.),
                ] {
                    if height > 0. {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left(), y),
                                size(bounds.size.width, px(height)),
                            ),
                            linear_gradient(
                                angle,
                                linear_color_stop(background, 0.),
                                linear_color_stop(background.opacity(0.), 1.),
                            ),
                        ));
                    }
                }
            });
        },
    )
    .absolute()
    .size_full()
}

fn fade_heights(offset: f32, max: f32) -> (f32, f32) {
    (offset.clamp(0., 24.), (max - offset).clamp(0., 24.))
}

#[cfg(test)]
mod tests {
    use super::fade_heights;
    #[test]
    fn edges_are_readable_when_no_more_content_is_hidden() {
        assert_eq!(fade_heights(0., 0.), (0., 0.));
        assert_eq!(fade_heights(0., 100.), (0., 24.));
        assert_eq!(fade_heights(50., 100.), (24., 24.));
        assert_eq!(fade_heights(100., 100.), (24., 0.));
        assert_eq!(fade_heights(3., 10.), (3., 7.));
    }
}
