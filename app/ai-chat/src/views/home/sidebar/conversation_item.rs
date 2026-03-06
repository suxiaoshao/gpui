use super::conversation_tree::{
    DragConversationTreeItem, DropState, SidebarConversationNode, folder_drop_state,
    reset_drop_target, root_drop_state, set_drop_target, target_for_folder, target_for_root,
};
use crate::{
    components::delete_confirm::open_delete_confirm_dialog,
    store::{ChatData, ChatDataEvent},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
};
use std::ops::Deref;

#[derive(IntoElement)]
pub(super) struct ConversationTreeItem {
    conversation: SidebarConversationNode,
    collapsed: bool,
    depth: usize,
    active_conversation_id: Option<i32>,
    target_folder: Option<(i32, SharedString)>,
    root_drop_target: bool,
}

impl ConversationTreeItem {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        collapsed: bool,
        depth: usize,
        active_conversation_id: Option<i32>,
        target_folder: Option<(i32, SharedString)>,
        root_drop_target: bool,
    ) -> Self {
        Self {
            conversation,
            collapsed,
            depth,
            active_conversation_id,
            target_folder,
            root_drop_target,
        }
    }
}

impl RenderOnce for ConversationTreeItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let conversation = self.conversation;
        let id = conversation.id;
        let title = conversation.title.clone();
        let padding_left = px((self.depth as f32) * 14. + 26.);
        let is_active = self.active_conversation_id == Some(id);
        let target_folder = self.target_folder.clone();
        let is_root_conversation = target_folder.is_none();
        let root_drop_target = self.root_drop_target;

        h_flex()
            .id(("conversation-tree-conversation", id as usize))
            .group(format!("conversation-group-{id}"))
            .w_full()
            .items_center()
            .gap_2()
            .rounded(cx.theme().radius)
            .px_2()
            .py_1()
            .pl(padding_left)
            .when(self.collapsed, |this| this.justify_center())
            .when(is_active, |this| {
                this.bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .when(!is_active && !root_drop_target, |this| {
                this.hover(|style| style.bg(cx.theme().sidebar_accent.opacity(0.8)))
            })
            .child(
                Label::new(conversation.icon.clone())
                    .text_xs()
                    .line_height(rems(0.75)),
            )
            .when(!self.collapsed, |this| {
                this.child(
                    h_flex()
                        .flex_1()
                        .items_center()
                        .justify_between()
                        .overflow_x_hidden()
                        .child(div().flex_1().overflow_x_hidden().child(title.clone()))
                        .child(
                            Button::new(("conversation-tree-conversation-menu", id as usize))
                                .icon(IconName::EllipsisVertical)
                                .ghost()
                                .xsmall()
                                .opacity(0.)
                                .group_hover(format!("conversation-group-{id}"), |style| {
                                    style.opacity(1.)
                                })
                                .dropdown_menu({
                                    let conversation = conversation.clone();
                                    move |menu, window, cx| {
                                        conversation_popup_menu(menu, &conversation, window, cx)
                                    }
                                }),
                        ),
                )
            })
            .on_drag_move::<DragConversationTreeItem>({
                let target_folder = target_folder.clone();
                move |event, window, cx| {
                    let target = target_folder
                        .as_ref()
                        .and_then(|(folder_id, folder_path)| {
                            target_for_folder(event.drag(cx), *folder_id, folder_path)
                        })
                        .or_else(|| target_for_root(event.drag(cx)));
                    if event.bounds.contains(&event.event.position) {
                        match target {
                            Some(target) => set_drop_target(window, cx, target),
                            None => reset_drop_target(window, cx),
                        }
                    } else if is_root_conversation && let Some(target) = target {
                        super::conversation_tree::clear_drop_target(window, cx, target);
                    }
                }
            })
            .on_drop({
                let target_folder = target_folder.clone();
                move |drag: &DragConversationTreeItem, window, cx| {
                    cx.stop_propagation();
                    reset_drop_target(window, cx);
                    if let Some((folder_id, folder_path)) = target_folder.as_ref() {
                        if folder_drop_state(drag, *folder_id, folder_path) != DropState::Valid {
                            return;
                        }
                        drag.move_to_folder(*folder_id, cx);
                    } else if root_drop_state(drag) == DropState::Valid {
                        drag.move_to_root(cx);
                    }
                }
            })
            .cursor_pointer()
            .on_click(move |_this, _window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                chat_data.update(cx, |_this, cx| {
                    cx.emit(ChatDataEvent::AddTab(id));
                });
            })
            .on_drag(
                DragConversationTreeItem::conversation(&conversation),
                |drag, _position, _window, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .context_menu({
                let conversation = conversation.clone();
                move |menu, window, cx| conversation_popup_menu(menu, &conversation, window, cx)
            })
    }
}

pub(super) fn conversation_popup_menu(
    menu: PopupMenu,
    conversation: &SidebarConversationNode,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let id = conversation.id;
    let title = conversation.title.clone();
    menu.item(
        PopupMenuItem::new("Delete")
            .icon(IconName::Delete)
            .on_click(move |_, window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                open_delete_confirm_dialog(
                    "Delete Conversation",
                    SharedString::from(format!(
                        "Delete conversation \"{title}\"? This action cannot be undone."
                    )),
                    move |_window, cx| {
                        chat_data.update(cx, |_this, cx| {
                            cx.emit(ChatDataEvent::DeleteConversation(id));
                        });
                    },
                    window,
                    cx,
                );
            }),
    )
}
