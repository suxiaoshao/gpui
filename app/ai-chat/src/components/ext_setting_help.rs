use crate::foundation::{assets::IconName, i18n::I18n};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, StyledExt, scroll::ScrollableElement, text::TextView, v_flex,
};

pub(crate) fn help_position(bounds: Bounds<Pixels>) -> Point<Pixels> {
    Point {
        x: bounds.origin.x,
        y: bounds.origin.y + bounds.size.height + px(6.),
    }
}

pub(crate) fn help_icon(id: impl Into<SharedString>, cx: &mut App) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .size_5()
        .rounded_full()
        .text_color(cx.theme().muted_foreground)
        .hover(|this| this.text_color(cx.theme().foreground))
        .child(Icon::new(IconName::Info).size_3())
}

pub(crate) fn help_panel(
    id: impl Into<SharedString>,
    tooltip_key: &'static str,
    position: Point<Pixels>,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let markdown = cx.global::<I18n>().t(tooltip_key);

    deferred(
        anchored()
            .snap_to_window_with_margin(px(8.))
            .anchor(Anchor::TopLeft)
            .position(position)
            .child(
                div().relative().child(
                    v_flex()
                        .id(id.clone())
                        .occlude()
                        .popover_style(cx)
                        .w(px(520.))
                        .h(px(360.))
                        .p_3()
                        .overflow_hidden()
                        .on_hover(on_hover)
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-scroll")))
                                .size_full()
                                .overflow_y_scrollbar()
                                .child(
                                    div().pr_2().child(
                                        TextView::markdown(
                                            SharedString::from(format!("{id}-content")),
                                            markdown,
                                        )
                                        .selectable(false),
                                    ),
                                ),
                        ),
                ),
            ),
    )
    .with_priority(1)
}
