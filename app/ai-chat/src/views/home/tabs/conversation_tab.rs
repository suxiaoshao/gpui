use crate::{state::WorkspaceStore, views::home::sidebar::DragConversationTreeItem};
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
        let active_id = cx.global::<WorkspaceStore>().read(cx).active_tab_key();
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
                        cx.global::<WorkspaceStore>().deref().clone().update(
                            cx,
                            |workspace, cx| {
                                workspace.remove_tab(self.id, cx);
                            },
                        );
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
            .drag_over::<DragConversationTreeItem>(move |this, drag, _window, cx| {
                if drag.conversation_id().is_none() {
                    return this;
                }
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
                cx.global::<WorkspaceStore>()
                    .deref()
                    .clone()
                    .update(cx, move |workspace, cx| {
                        workspace.move_tab(drag.id, Some(self.id), cx);
                    });
            })
            .on_drop(move |drag: &DragConversationTreeItem, window, cx| {
                let Some(conversation_id) = drag.conversation_id() else {
                    return;
                };
                cx.global::<WorkspaceStore>()
                    .deref()
                    .clone()
                    .update(cx, move |workspace, cx| {
                        workspace.add_conversation_tab(conversation_id, window, cx);
                        if conversation_id != self.id {
                            workspace.move_tab(conversation_id, Some(self.id), cx);
                        }
                    });
            })
            .on_click(move |_this, _window, cx| {
                cx.global::<WorkspaceStore>()
                    .deref()
                    .clone()
                    .update(cx, move |workspace, cx| {
                        workspace.activate_tab(self.id, _window, cx);
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
