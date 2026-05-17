use crate::foundation::{assets::IconName, i18n::I18n};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, hover_card::HoverCard, scroll::ScrollableElement, text::TextView,
};
use std::time::Duration;

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

pub(crate) fn help_card(
    id: impl Into<SharedString>,
    tooltip_key: &'static str,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let markdown = cx.global::<I18n>().t(tooltip_key);

    HoverCard::new(id.clone())
        .anchor(Anchor::TopLeft)
        .open_delay(Duration::from_millis(200))
        .close_delay(Duration::from_millis(200))
        .w(px(520.))
        .h(px(360.))
        .overflow_hidden()
        .trigger(help_icon(SharedString::from(format!("{id}-trigger")), cx))
        .child(
            div()
                .id(SharedString::from(format!("{id}-scroll")))
                .size_full()
                .overflow_y_scrollbar()
                .child(
                    div().pr_2().child(
                        TextView::markdown(SharedString::from(format!("{id}-content")), markdown)
                            .selectable(false),
                    ),
                ),
        )
}
