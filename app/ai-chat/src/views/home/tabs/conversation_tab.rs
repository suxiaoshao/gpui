use crate::{
    store::{ChatData, ChatDataEvent},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, label::Label};
use std::ops::Deref;

#[derive(Clone)]
pub(crate) struct DragTab {
    pub(crate) id: i32,
    icon: SharedString,
    name: SharedString,
}

impl DragTab {
    fn new(id: i32, icon: SharedString, name: SharedString) -> Self {
        Self { id, icon, name }
    }
}

impl Render for DragTab {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        h_flex()
            .gap_1()
            .px_4()
            .py_2()
            .border_1()
            .border_color(cx.theme().drag_border)
            .rounded_sm()
            .bg(cx.theme().tab_active)
            .text_color(cx.theme().tab_foreground)
            .opacity(0.85)
            .child(Label::new(&self.icon).text_xs().line_height(rems(0.75)))
            .child(Label::new(&self.name).text_xs().line_height(rems(0.75)))
    }
}

#[derive(IntoElement, Clone)]
pub(crate) struct ConversationTabView {
    pub(crate) id: i32,
    pub(crate) icon: SharedString,
    pub(crate) name: SharedString,
}

impl ConversationTabView {
    pub(crate) fn new(id: i32, icon: SharedString, name: SharedString) -> Self {
        Self { id, icon, name }
    }
}

impl RenderOnce for ConversationTabView {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let chat_data = cx.global::<ChatData>().read(cx);
        let active_id = chat_data
            .as_ref()
            .ok()
            .and_then(|data| data.active_tab_key());
        let is_active = active_id == Some(self.id);
        let icon = self.icon.clone();
        let name = self.name.clone();
        h_flex()
            .id(self.id)
            .flex_initial()
            .group("tab")
            .gap_1()
            .relative()
            .child(
                div()
                    .id(self.id)
                    .child(Icon::new(IconName::Close).size_3())
                    .absolute()
                    .top(px(6.))
                    .right_1()
                    .opacity(0.)
                    .p_0p5()
                    .rounded_sm()
                    .hover(|style| style.bg(cx.theme().secondary_hover))
                    .group_hover("tab", |style| style.opacity(1.))
                    .on_click(move |_state, _window, cx| {
                        cx.stop_propagation();
                        let chat_data = cx.global::<ChatData>().deref().clone();
                        chat_data.update(cx, |_chat_data, cx| {
                            cx.emit(ChatDataEvent::RemoveTab(self.id));
                        });
                    }),
            )
            .child(Label::new(icon.clone()).text_xs().line_height(rems(0.75)))
            .child(Label::new(name.clone()).text_xs().line_height(rems(0.75)))
            .px_6()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .when(is_active, |this| {
                this.bg(cx.theme().tab_active).border_b_0()
            })
            .drag_over::<DragTab>(move |this, _drag, _window, cx| {
                if is_active {
                    this.border_b_1().border_color(cx.theme().drag_border)
                } else {
                    this.border_color(cx.theme().drag_border)
                }
            })
            .on_drop(move |drag: &DragTab, _window, cx| {
                if drag.id == self.id {
                    return;
                }
                let chat_data = cx.global::<ChatData>().deref().clone();
                chat_data.update(cx, move |_this, cx| {
                    cx.emit(ChatDataEvent::MoveTab {
                        from_id: drag.id,
                        to_id: Some(self.id),
                    });
                });
            })
            .on_click(move |_this, _window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                chat_data.update(cx, move |_this, cx| {
                    cx.emit(ChatDataEvent::ActivateTab(self.id));
                });
            })
            .on_drag(
                DragTab::new(self.id, icon, name),
                |drag: &DragTab, _position, _window, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .cursor_pointer()
    }
}
