use super::conversation_item::ConversationTreeItem;
use super::conversation_tree::{
    ActiveDropTarget, DragConversationTreeItem, DropState, SidebarFolderNode,
    folder_block_drop_target, folder_drop_state, reset_drop_target, set_drop_target,
    target_for_conversation_group, target_for_folder,
};
use crate::{
    components::{
        add_conversation::add_conversation_dialog, add_folder::add_folder_dialog,
        delete_confirm::open_delete_confirm_dialog,
    },
    store::{ChatData, ChatDataEvent},
    workspace_state::WorkspaceStore,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    v_flex,
};
use std::collections::BTreeSet;
use std::ops::Deref;

#[derive(IntoElement)]
pub(super) struct FolderTreeItem {
    folder: SidebarFolderNode,
    collapsed: bool,
    depth: usize,
    active_conversation_id: Option<i32>,
    open_folder_ids: BTreeSet<i32>,
    active_drop_target: Option<ActiveDropTarget>,
}

impl FolderTreeItem {
    pub(super) fn new(
        folder: SidebarFolderNode,
        collapsed: bool,
        depth: usize,
        active_conversation_id: Option<i32>,
        open_folder_ids: &BTreeSet<i32>,
        active_drop_target: Option<ActiveDropTarget>,
    ) -> Self {
        Self {
            folder,
            collapsed,
            depth,
            active_conversation_id,
            open_folder_ids: open_folder_ids.clone(),
            active_drop_target,
        }
    }
}

impl RenderOnce for FolderTreeItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let folder = self.folder;
        let id = folder.id;
        let name = folder.name.clone();
        let path = folder.path.clone();
        let active_drop_target = self.active_drop_target;
        let block_drop_target = folder_block_drop_target(active_drop_target, id);
        let root_drop_target = active_drop_target.is_some_and(ActiveDropTarget::is_root);
        let is_open = !self.collapsed && self.open_folder_ids.contains(&id);
        let padding_left = px((self.depth as f32) * 14.);

        v_flex()
            .id(("conversation-tree-folder-block", id as usize))
            .border_1()
            .border_color(cx.theme().transparent)
            .rounded(cx.theme().radius)
            .when_some(block_drop_target, |this, invalid| {
                if invalid {
                    this.bg(cx.theme().danger.opacity(0.12))
                        .border_color(cx.theme().danger)
                        .text_color(cx.theme().danger)
                } else {
                    this.bg(cx.theme().drop_target)
                        .border_color(cx.theme().drag_border)
                }
            })
            .child(
                h_flex()
                    .id(("conversation-tree-folder", id as usize))
                    .group(format!("folder-group-{id}"))
                    .w_full()
                    .items_center()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .px_2()
                    .py_1()
                    .pl(padding_left)
                    .when_some(block_drop_target, |this, invalid| {
                        if invalid {
                            this.text_color(cx.theme().danger)
                        } else {
                            this.text_color(cx.theme().sidebar_foreground)
                        }
                    })
                    .when(
                        !self.collapsed && block_drop_target.is_none() && !root_drop_target,
                        |this| this.hover(|style| style.bg(cx.theme().sidebar_accent.opacity(0.8))),
                    )
                    .when(self.collapsed, |this| this.justify_center())
                    .child(
                        Button::new(("conversation-tree-folder-caret", id as usize))
                            .ghost()
                            .xsmall()
                            .icon(
                                Icon::new(IconName::ChevronRight)
                                    .size_3()
                                    .when(is_open, |this| this.rotate(percentage(90. / 360.))),
                            )
                            .when(self.collapsed, |this| this.invisible())
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.global::<WorkspaceStore>()
                                    .deref()
                                    .clone()
                                    .update(cx, |workspace, cx| {
                                        workspace.toggle_folder_open(id, cx);
                                    });
                            }),
                    )
                    .child(Icon::new(IconName::Folder).size_4())
                    .when(!self.collapsed, |this| {
                        this.child(
                            h_flex()
                                .flex_1()
                                .items_center()
                                .justify_between()
                                .overflow_x_hidden()
                                .child(div().flex_1().overflow_x_hidden().child(name.clone()))
                                .child(
                                    Button::new(("conversation-tree-folder-menu", id as usize))
                                        .icon(IconName::EllipsisVertical)
                                        .ghost()
                                        .xsmall()
                                        .opacity(0.)
                                        .group_hover(format!("folder-group-{id}"), |style| {
                                            style.opacity(1.)
                                        })
                                        .on_click(|_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .dropdown_menu({
                                            let folder = folder.clone();
                                            move |menu, window, cx| {
                                                folder_popup_menu(menu, &folder, window, cx)
                                            }
                                        }),
                                ),
                        )
                    })
                    .cursor_pointer()
                    .on_click(move |_event, _window, cx| {
                        cx.global::<WorkspaceStore>()
                            .deref()
                            .clone()
                            .update(cx, |workspace, cx| {
                                workspace.toggle_folder_open(id, cx);
                            });
                    })
                    .on_drag(
                        DragConversationTreeItem::folder(&folder),
                        |drag, _position, _window, cx| {
                            cx.stop_propagation();
                            cx.new(|_| drag.clone())
                        },
                    )
                    .context_menu({
                        let folder = folder.clone();
                        move |menu, window, cx| folder_popup_menu(menu, &folder, window, cx)
                    }),
            )
            .on_drag_move::<DragConversationTreeItem>({
                let path = path.clone();
                move |event, window, cx| {
                    let target = target_for_folder(event.drag(cx), id, &path);
                    if event.bounds.contains(&event.event.position) {
                        match target {
                            Some(target) => set_drop_target(window, cx, target),
                            None => reset_drop_target(window, cx),
                        }
                    } else if let Some(target) = target {
                        super::conversation_tree::clear_drop_target(window, cx, target);
                    }
                }
            })
            .on_drop({
                let path = path.clone();
                move |drag: &DragConversationTreeItem, window, cx| {
                    cx.stop_propagation();
                    reset_drop_target(window, cx);
                    if folder_drop_state(drag, id, &path) != DropState::Valid {
                        return;
                    }
                    drag.move_to_folder(id, cx);
                }
            })
            .when(is_open, |this| {
                let has_conversations = !folder.conversations.is_empty();
                let child_folders = folder.folders;
                let child_conversations = folder.conversations;
                let conversation_group = v_flex()
                    .gap_1()
                    .children(child_conversations.into_iter().map(|conversation| {
                        ConversationTreeItem::new(
                            conversation,
                            self.collapsed,
                            self.depth + 1,
                            self.active_conversation_id,
                            root_drop_target,
                        )
                        .into_any_element()
                    }))
                    .on_drag_move::<DragConversationTreeItem>({
                        let path = path.clone();
                        move |event, window, cx| {
                            let target = target_for_conversation_group(
                                event.drag(cx),
                                Some((id, path.as_ref())),
                            );
                            if event.bounds.contains(&event.event.position) {
                                match target {
                                    Some(target) => set_drop_target(window, cx, target),
                                    None => reset_drop_target(window, cx),
                                }
                            } else if let Some(target) = target {
                                super::conversation_tree::clear_drop_target(window, cx, target);
                            }
                        }
                    })
                    .on_drop({
                        let path = path.clone();
                        move |drag: &DragConversationTreeItem, window, cx| {
                            cx.stop_propagation();
                            reset_drop_target(window, cx);
                            if folder_drop_state(drag, id, &path) != DropState::Valid {
                                return;
                            }
                            drag.move_to_folder(id, cx);
                        }
                    });
                this.child(
                    v_flex()
                        .gap_1()
                        .children(child_folders.into_iter().map(|child| {
                            FolderTreeItem::new(
                                child,
                                self.collapsed,
                                self.depth + 1,
                                self.active_conversation_id,
                                &self.open_folder_ids,
                                active_drop_target,
                            )
                            .into_any_element()
                        }))
                        .when(has_conversations, |this| this.child(conversation_group)),
                )
            })
    }
}

pub(super) fn folder_popup_menu(
    menu: PopupMenu,
    folder: &SidebarFolderNode,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let id = folder.id;
    let name = folder.name.clone();
    menu.item(
        PopupMenuItem::new("Add Conversation")
            .icon(IconName::Plus)
            .on_click(move |_, window, cx| add_conversation_dialog(Some(id), window, cx)),
    )
    .item(
        PopupMenuItem::new("Add Folder")
            .icon(IconName::Plus)
            .on_click(move |_, window, cx| add_folder_dialog(Some(id), window, cx)),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new("Delete")
            .icon(IconName::Delete)
            .on_click(move |_, window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                open_delete_confirm_dialog(
                    "Delete Folder",
                    SharedString::from(format!(
                        "Delete folder \"{name}\" and its contents? This action cannot be undone."
                    )),
                    move |_window, cx| {
                        chat_data.update(cx, |_this, cx| {
                            cx.emit(ChatDataEvent::DeleteFolder(id));
                        });
                    },
                    window,
                    cx,
                );
            }),
    )
}
