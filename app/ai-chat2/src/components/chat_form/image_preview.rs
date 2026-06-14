use crate::{
    foundation::{self, assets::IconName},
    state::attachments::ComposerAttachment,
};
use fluent_bundle::FluentArgs;
use gpui::{
    Context, CursorStyle, InteractiveElement as _, IntoElement as _, MouseButton, ObjectFit,
    ParentElement as _, PinchEvent, Pixels, Point, Render, ScrollDelta, ScrollHandle,
    ScrollWheelEvent, Size, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window,
    black, div, img, point, prelude::FluentBuilder as _, px, white,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt as ComponentWindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
};
use std::path::PathBuf;

const SIDE_INSET: f32 = 56.;
const TOP_INSET: f32 = 52.;
const BOTTOM_INSET: f32 = 88.;
const CLOSE_INSET: f32 = 16.;
const TOOLBAR_BOTTOM: f32 = 24.;
const CONTROL_BUTTON_SIZE: f32 = 40.;
const SCROLL_LINE_MULTIPLIER: f32 = 20.;
const MAX_ZOOM_PERCENT: f32 = 800.;
const ZOOM_EPSILON: f32 = 0.01;
const ZOOM_RAMP_PERCENT: [f32; 10] = [25., 50., 75., 100., 125., 150., 200., 300., 400., 800.];

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreviewSize {
    width: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreviewPoint {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ZoomStep {
    In,
    Out,
}

pub(super) struct ImagePreview {
    attachment: ComposerAttachment,
    natural_size: Option<PreviewSize>,
    load_error: Option<String>,
    zoom_percent: Option<f32>,
    scroll_handle: ScrollHandle,
}

impl ImagePreview {
    pub(super) fn new(attachment: ComposerAttachment, _cx: &mut Context<Self>) -> Self {
        let (natural_size, load_error) = natural_size_for_attachment(&attachment)
            .map(|size| (Some(size), None))
            .unwrap_or_else(|err| (None, Some(err)));

        Self {
            attachment,
            natural_size,
            load_error,
            zoom_percent: None,
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn current_zoom_percent(&self, viewport_size: PreviewSize) -> f32 {
        let Some(natural_size) = self.natural_size else {
            return 100.;
        };
        let fit = fit_zoom_percent(natural_size, viewport_size);
        self.zoom_percent
            .map(|zoom| clamp_zoom_percent(zoom, fit))
            .unwrap_or(fit)
    }

    fn preview_viewport_size(&self, window: &Window) -> PreviewSize {
        preview_area_size(preview_size_from_pixels(window.viewport_size()))
    }

    fn set_zoom_percent(
        &mut self,
        next_zoom: f32,
        anchor_position: Option<Point<Pixels>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(natural_size) = self.natural_size else {
            return;
        };

        let viewport_size = self.preview_viewport_size(window);
        let fit = fit_zoom_percent(natural_size, viewport_size);
        let old_zoom = self.current_zoom_percent(viewport_size);
        let next_zoom = clamp_zoom_percent(next_zoom, fit);

        let bounds = self.scroll_handle.bounds();
        let anchor = anchor_position
            .map(|position| {
                let local = position - bounds.origin;
                PreviewPoint {
                    x: local.x.into(),
                    y: local.y.into(),
                }
            })
            .unwrap_or_else(|| PreviewPoint {
                x: viewport_size.width / 2.,
                y: viewport_size.height / 2.,
            });

        let scroll_viewport_size = if bounds.size.width > px(0.) && bounds.size.height > px(0.) {
            preview_size_from_pixels(bounds.size)
        } else {
            viewport_size
        };
        let old_offset = preview_point_from_pixels(self.scroll_handle.offset());
        let next_offset = anchored_scroll_offset(
            old_offset,
            scroll_viewport_size,
            natural_size,
            old_zoom,
            next_zoom,
            anchor,
        );

        self.zoom_percent = Some(next_zoom);
        self.scroll_handle
            .set_offset(point(px(next_offset.x), px(next_offset.y)));
        cx.notify();
    }

    fn step_zoom(&mut self, direction: ZoomStep, window: &mut Window, cx: &mut Context<Self>) {
        let Some(natural_size) = self.natural_size else {
            return;
        };
        let viewport_size = self.preview_viewport_size(window);
        let fit = fit_zoom_percent(natural_size, viewport_size);
        let current = self.current_zoom_percent(viewport_size);
        let next = next_zoom_step(current, fit, direction);
        self.set_zoom_percent(next, None, window, cx);
    }

    fn zoom_by_factor(
        &mut self,
        factor: f32,
        anchor_position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !factor.is_finite() || factor <= 0. {
            return;
        }

        let current = self.current_zoom_percent(self.preview_viewport_size(window));
        self.set_zoom_percent(current * factor, Some(anchor_position), window, cx);
    }

    fn handle_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !(event.modifiers.control || event.modifiers.platform) {
            return;
        }

        let delta = match event.delta {
            ScrollDelta::Pixels(pixels) => pixels.y.into(),
            ScrollDelta::Lines(lines) => lines.y * SCROLL_LINE_MULTIPLIER,
        };
        let factor = if delta > 0. {
            1. + delta.abs() * 0.01
        } else {
            1. / (1. + delta.abs() * 0.01)
        };

        cx.stop_propagation();
        self.zoom_by_factor(factor, event.position, window, cx);
    }

    fn handle_pinch(&mut self, event: &PinchEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.zoom_by_factor((1. + event.delta).max(0.01), event.position, window, cx);
    }

    fn render_image(
        &self,
        viewport_size: PreviewSize,
        zoom_percent: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let natural_size = self.natural_size.unwrap_or(PreviewSize {
            width: 1.,
            height: 1.,
        });
        let image_size = image_render_size(natural_size, zoom_percent);
        let content_size = scroll_content_size(viewport_size, image_size);
        let image_origin = centered_image_origin(content_size, image_size);

        div()
            .id("chat-form-image-preview-scroll")
            .absolute()
            .top(px(TOP_INSET))
            .right(px(SIDE_INSET))
            .bottom(px(BOTTOM_INSET))
            .left(px(SIDE_INSET))
            .overflow_scroll()
            .track_scroll(&self.scroll_handle)
            .cursor(CursorStyle::Arrow)
            .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
            .on_pinch(cx.listener(Self::handle_pinch))
            .child(
                div()
                    .relative()
                    .w(px(content_size.width))
                    .h(px(content_size.height))
                    .child(
                        div()
                            .absolute()
                            .left(px(image_origin.x))
                            .top(px(image_origin.y))
                            .w(px(image_size.width))
                            .h(px(image_size.height))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                img(self.attachment.path.clone())
                                    .w(px(image_size.width))
                                    .h(px(image_size.height))
                                    .object_fit(ObjectFit::Fill),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_load_error(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let label = cx
            .global::<foundation::I18n>()
            .t("chat-form-image-preview-load-failed");
        let message = self
            .load_error
            .as_ref()
            .map(|err| format!("{label}: {err}"))
            .unwrap_or(label);

        div()
            .absolute()
            .top(px(TOP_INSET))
            .right(px(SIDE_INSET))
            .bottom(px(BOTTOM_INSET))
            .left(px(SIDE_INSET))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Label::new(message)
                    .text_sm()
                    .text_color(white().opacity(0.78)),
            )
            .into_any_element()
    }

    fn render_header(&self) -> gpui::AnyElement {
        h_flex()
            .absolute()
            .top(px(CLOSE_INSET))
            .left(px(CLOSE_INSET))
            .max_w(px(520.))
            .px_3()
            .py_2()
            .rounded(px(999.))
            .bg(black().opacity(0.28))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Label::new(self.attachment.name.clone())
                    .text_sm()
                    .text_color(white().opacity(0.84))
                    .truncate(),
            )
            .into_any_element()
    }

    fn render_close_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tooltip = cx
            .global::<foundation::I18n>()
            .t("chat-form-image-preview-close");

        div()
            .absolute()
            .top(px(CLOSE_INSET))
            .right(px(CLOSE_INSET))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new("chat-form-image-preview-close")
                    .ghost()
                    .with_size(px(CONTROL_BUTTON_SIZE))
                    .size(px(CONTROL_BUTTON_SIZE))
                    .p_0()
                    .rounded(px(999.))
                    .bg(white().opacity(0.94))
                    .child(Icon::new(IconName::X).with_size(px(22.)))
                    .tooltip(tooltip)
                    .on_click(|_, window, cx| {
                        window.close_dialog(cx);
                        cx.stop_propagation();
                    }),
            )
            .into_any_element()
    }

    fn render_zoom_controls(
        &self,
        zoom_percent: f32,
        viewport_size: PreviewSize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let fit = self
            .natural_size
            .map(|natural_size| fit_zoom_percent(natural_size, viewport_size))
            .unwrap_or(100.);
        let zoom_in_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-image-preview-zoom-in");
        let zoom_out_label = cx
            .global::<foundation::I18n>()
            .t("chat-form-image-preview-zoom-out");
        let mut zoom_args = FluentArgs::new();
        zoom_args.set("percent", format_zoom_percent(zoom_percent));
        let zoom_tooltip = cx
            .global::<foundation::I18n>()
            .t_with_args("chat-form-image-preview-zoom-percent", &zoom_args);
        let can_zoom_out =
            next_zoom_step(zoom_percent, fit, ZoomStep::Out) < zoom_percent - ZOOM_EPSILON;
        let can_zoom_in =
            next_zoom_step(zoom_percent, fit, ZoomStep::In) > zoom_percent + ZOOM_EPSILON;

        h_flex()
            .absolute()
            .bottom(px(TOOLBAR_BOTTOM))
            .left_0()
            .right_0()
            .justify_center()
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded(px(999.))
                    .bg(white().opacity(0.94))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("chat-form-image-preview-zoom-out")
                            .ghost()
                            .with_size(px(CONTROL_BUTTON_SIZE))
                            .size(px(CONTROL_BUTTON_SIZE))
                            .p_0()
                            .rounded(px(999.))
                            .disabled(!can_zoom_out)
                            .child(Icon::new(IconName::Minus).with_size(px(20.)))
                            .tooltip(zoom_out_label)
                            .on_click(cx.listener(|preview, _, window, cx| {
                                cx.stop_propagation();
                                preview.step_zoom(ZoomStep::Out, window, cx);
                            })),
                    )
                    .child(
                        Button::new("chat-form-image-preview-zoom-percent")
                            .ghost()
                            .min_w(px(68.))
                            .h(px(CONTROL_BUTTON_SIZE))
                            .label(format!("{}%", format_zoom_percent(zoom_percent)))
                            .tooltip(zoom_tooltip),
                    )
                    .child(
                        Button::new("chat-form-image-preview-zoom-in")
                            .ghost()
                            .with_size(px(CONTROL_BUTTON_SIZE))
                            .size(px(CONTROL_BUTTON_SIZE))
                            .p_0()
                            .rounded(px(999.))
                            .disabled(!can_zoom_in)
                            .child(Icon::new(IconName::Plus).with_size(px(20.)))
                            .tooltip(zoom_in_label)
                            .on_click(cx.listener(|preview, _, window, cx| {
                                cx.stop_propagation();
                                preview.step_zoom(ZoomStep::In, window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ImagePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let viewport_size = self.preview_viewport_size(window);
        let zoom_percent = self.current_zoom_percent(viewport_size);

        div()
            .id("chat-form-image-preview-overlay")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(black().opacity(0.88))
            .text_color(cx.theme().foreground)
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.close_dialog(cx);
                cx.stop_propagation();
            })
            .when(self.natural_size.is_some(), |this| {
                this.child(self.render_image(viewport_size, zoom_percent, cx))
                    .child(self.render_zoom_controls(zoom_percent, viewport_size, cx))
            })
            .when(self.natural_size.is_none(), |this| {
                this.child(self.render_load_error(cx))
            })
            .child(self.render_header())
            .child(self.render_close_button(cx))
    }
}

fn natural_size_for_attachment(attachment: &ComposerAttachment) -> Result<PreviewSize, String> {
    if let (Some(width), Some(height)) = (attachment.width, attachment.height)
        && width > 0
        && height > 0
    {
        return Ok(PreviewSize {
            width: width as f32,
            height: height as f32,
        });
    }

    image_dimensions(&attachment.path).map(|(width, height)| PreviewSize {
        width: width as f32,
        height: height as f32,
    })
}

fn image_dimensions(path: &PathBuf) -> Result<(u32, u32), String> {
    image::image_dimensions(path).map_err(|err| err.to_string())
}

fn preview_size_from_pixels(size: Size<Pixels>) -> PreviewSize {
    PreviewSize {
        width: size.width.into(),
        height: size.height.into(),
    }
}

fn preview_point_from_pixels(point: Point<Pixels>) -> PreviewPoint {
    PreviewPoint {
        x: point.x.into(),
        y: point.y.into(),
    }
}

fn preview_area_size(viewport_size: PreviewSize) -> PreviewSize {
    PreviewSize {
        width: (viewport_size.width - SIDE_INSET * 2.).max(1.),
        height: (viewport_size.height - TOP_INSET - BOTTOM_INSET).max(1.),
    }
}

fn fit_zoom_percent(natural_size: PreviewSize, viewport_size: PreviewSize) -> f32 {
    if natural_size.width <= 0. || natural_size.height <= 0. {
        return 100.;
    }

    let width_ratio = viewport_size.width / natural_size.width;
    let height_ratio = viewport_size.height / natural_size.height;
    (width_ratio.min(height_ratio).min(1.) * 100.).max(ZOOM_EPSILON)
}

fn clamp_zoom_percent(zoom_percent: f32, fit_percent: f32) -> f32 {
    if !zoom_percent.is_finite() {
        return fit_percent;
    }
    zoom_percent.clamp(fit_percent, MAX_ZOOM_PERCENT)
}

fn zoom_ramp(fit_percent: f32) -> Vec<f32> {
    let fit_percent = fit_percent.clamp(ZOOM_EPSILON, 100.);
    let mut ramp = Vec::with_capacity(ZOOM_RAMP_PERCENT.len() + 1);
    ramp.push(fit_percent);
    ramp.extend(
        ZOOM_RAMP_PERCENT
            .iter()
            .copied()
            .filter(|zoom| *zoom >= fit_percent - ZOOM_EPSILON),
    );
    ramp.sort_by(|a, b| a.total_cmp(b));
    ramp.dedup_by(|a, b| (*a - *b).abs() < ZOOM_EPSILON);
    ramp
}

fn next_zoom_step(current_zoom: f32, fit_percent: f32, direction: ZoomStep) -> f32 {
    let ramp = zoom_ramp(fit_percent);
    let current_zoom = clamp_zoom_percent(current_zoom, fit_percent);

    match direction {
        ZoomStep::In => ramp
            .iter()
            .copied()
            .find(|zoom| *zoom > current_zoom + ZOOM_EPSILON)
            .unwrap_or(MAX_ZOOM_PERCENT),
        ZoomStep::Out => ramp
            .iter()
            .rev()
            .copied()
            .find(|zoom| *zoom < current_zoom - ZOOM_EPSILON)
            .unwrap_or_else(|| ramp[0]),
    }
}

fn image_render_size(natural_size: PreviewSize, zoom_percent: f32) -> PreviewSize {
    let scale = zoom_percent / 100.;
    PreviewSize {
        width: (natural_size.width * scale).max(1.),
        height: (natural_size.height * scale).max(1.),
    }
}

fn scroll_content_size(viewport_size: PreviewSize, image_size: PreviewSize) -> PreviewSize {
    PreviewSize {
        width: image_size.width.max(viewport_size.width),
        height: image_size.height.max(viewport_size.height),
    }
}

fn centered_image_origin(content_size: PreviewSize, image_size: PreviewSize) -> PreviewPoint {
    PreviewPoint {
        x: ((content_size.width - image_size.width) / 2.).max(0.),
        y: ((content_size.height - image_size.height) / 2.).max(0.),
    }
}

fn anchored_scroll_offset(
    old_offset: PreviewPoint,
    viewport_size: PreviewSize,
    natural_size: PreviewSize,
    old_zoom_percent: f32,
    new_zoom_percent: f32,
    anchor: PreviewPoint,
) -> PreviewPoint {
    let old_image_size = image_render_size(natural_size, old_zoom_percent);
    let new_image_size = image_render_size(natural_size, new_zoom_percent);
    let old_content_size = scroll_content_size(viewport_size, old_image_size);
    let new_content_size = scroll_content_size(viewport_size, new_image_size);
    let old_image_origin = centered_image_origin(old_content_size, old_image_size);
    let new_image_origin = centered_image_origin(new_content_size, new_image_size);

    let content_point = PreviewPoint {
        x: anchor.x - old_offset.x,
        y: anchor.y - old_offset.y,
    };
    let image_unit = PreviewPoint {
        x: (content_point.x - old_image_origin.x) / old_image_size.width,
        y: (content_point.y - old_image_origin.y) / old_image_size.height,
    };
    let next_content_point = PreviewPoint {
        x: new_image_origin.x + image_unit.x * new_image_size.width,
        y: new_image_origin.y + image_unit.y * new_image_size.height,
    };

    clamp_scroll_offset(
        PreviewPoint {
            x: anchor.x - next_content_point.x,
            y: anchor.y - next_content_point.y,
        },
        viewport_size,
        new_content_size,
    )
}

fn clamp_scroll_offset(
    offset: PreviewPoint,
    viewport_size: PreviewSize,
    content_size: PreviewSize,
) -> PreviewPoint {
    let min_x = (viewport_size.width - content_size.width).min(0.);
    let min_y = (viewport_size.height - content_size.height).min(0.);
    PreviewPoint {
        x: offset.x.clamp(min_x, 0.),
        y: offset.y.clamp(min_y, 0.),
    }
}

fn format_zoom_percent(zoom_percent: f32) -> String {
    format!("{:.0}", zoom_percent.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_zoom_caps_at_full_size_for_small_images() {
        assert_eq!(
            fit_zoom_percent(
                PreviewSize {
                    width: 400.,
                    height: 300.
                },
                PreviewSize {
                    width: 1000.,
                    height: 800.
                },
            ),
            100.
        );
    }

    #[test]
    fn fit_zoom_handles_wide_tall_and_large_images() {
        assert_eq!(
            fit_zoom_percent(
                PreviewSize {
                    width: 2000.,
                    height: 500.
                },
                PreviewSize {
                    width: 1000.,
                    height: 800.
                },
            ),
            50.
        );
        assert_eq!(
            fit_zoom_percent(
                PreviewSize {
                    width: 500.,
                    height: 2000.
                },
                PreviewSize {
                    width: 1000.,
                    height: 800.
                },
            ),
            40.
        );
        assert_eq!(
            fit_zoom_percent(
                PreviewSize {
                    width: 4000.,
                    height: 4000.
                },
                PreviewSize {
                    width: 1000.,
                    height: 800.
                },
            ),
            20.
        );
    }

    #[test]
    fn zoom_ramp_dedups_fit_and_clamps_to_fit_minimum() {
        assert_eq!(
            zoom_ramp(50.),
            vec![50., 75., 100., 125., 150., 200., 300., 400., 800.]
        );
        assert_eq!(
            zoom_ramp(62.),
            vec![62., 75., 100., 125., 150., 200., 300., 400., 800.]
        );
        assert_eq!(
            zoom_ramp(100.),
            vec![100., 125., 150., 200., 300., 400., 800.]
        );
    }

    #[test]
    fn zoom_step_respects_bounds() {
        assert_eq!(next_zoom_step(50., 50., ZoomStep::In), 75.);
        assert_eq!(next_zoom_step(50., 50., ZoomStep::Out), 50.);
        assert_eq!(next_zoom_step(400., 50., ZoomStep::In), 800.);
        assert_eq!(next_zoom_step(800., 50., ZoomStep::In), 800.);
        assert_eq!(next_zoom_step(125., 50., ZoomStep::Out), 100.);
    }

    #[test]
    fn anchored_scroll_offset_keeps_focus_point_stable() {
        let old_offset = PreviewPoint { x: -100., y: -50. };
        let viewport_size = PreviewSize {
            width: 1000.,
            height: 800.,
        };
        let natural_size = PreviewSize {
            width: 2000.,
            height: 1600.,
        };
        let anchor = PreviewPoint { x: 500., y: 400. };

        let next =
            anchored_scroll_offset(old_offset, viewport_size, natural_size, 50., 100., anchor);

        assert_eq!(next, PreviewPoint { x: -700., y: -500. });
    }

    #[test]
    fn anchored_scroll_offset_clamps_when_zooming_back_to_fit() {
        let next = anchored_scroll_offset(
            PreviewPoint { x: -700., y: -500. },
            PreviewSize {
                width: 1000.,
                height: 800.,
            },
            PreviewSize {
                width: 2000.,
                height: 1600.,
            },
            100.,
            50.,
            PreviewPoint { x: 500., y: 400. },
        );

        assert_eq!(next, PreviewPoint { x: 0., y: 0. });
    }
}
