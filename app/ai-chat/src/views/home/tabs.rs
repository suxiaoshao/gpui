use crate::{
    state::{WorkspaceState, WorkspaceStore},
    views::home::sidebar::DragConversationTreeItem,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use std::ops::Deref;

mod conversation_panel;
mod conversation_tab;
mod template_detail;
mod template_list;

pub(crate) use conversation_panel::{
    ConversationPanelView, open_copy_conversation_dialog, open_export_conversation_prompt,
};
pub(crate) use conversation_tab::{ConversationTabView, DragTab};
pub(crate) use template_detail::TemplateDetailView;
pub(crate) use template_list::TemplateListView;

pub fn init(cx: &mut App) {
    template_list::init(cx);
}

pub(crate) struct TabsView {
    workspace: WeakEntity<WorkspaceState>,
}

impl TabsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workspace = cx.global::<WorkspaceStore>().deref().downgrade();
        Self { workspace }
    }
}

impl Render for TabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.upgrade().map(|x| x.read(cx));
        v_flex()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .map(|this| match workspace {
                Some(workspace) => this
                    .child(
                        h_flex()
                            .flex_initial()
                            .h_7()
                            .bg(cx.theme().accent)
                            .children(workspace.tabs())
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .drag_over::<DragTab>(|this, _drag, _window, cx| {
                                        this.bg(cx.theme().drop_target)
                                    })
                                    .drag_over::<DragConversationTreeItem>(
                                        |this, drag, _window, cx| {
                                            if drag.conversation_id().is_some() {
                                                this.bg(cx.theme().drop_target)
                                            } else {
                                                this
                                            }
                                        },
                                    )
                                    .on_drop(move |drag: &DragTab, _window, cx| {
                                        cx.global::<WorkspaceStore>().deref().clone().update(
                                            cx,
                                            move |workspace, cx| {
                                                workspace.move_tab(drag.id, None, cx);
                                            },
                                        );
                                    })
                                    .on_drop(move |drag: &DragConversationTreeItem, window, cx| {
                                        let Some(conversation_id) = drag.conversation_id() else {
                                            return;
                                        };
                                        cx.global::<WorkspaceStore>().deref().clone().update(
                                            cx,
                                            move |workspace, cx| {
                                                workspace.add_conversation_tab(
                                                    conversation_id,
                                                    window,
                                                    cx,
                                                );
                                                workspace.move_tab(conversation_id, None, cx);
                                            },
                                        );
                                    }),
                            ),
                    )
                    .when_some(workspace.panel(), |this, panel| {
                        this.child(div().flex_1().w_full().overflow_hidden().child(panel))
                    }),
                None => this,
            })
    }
}
