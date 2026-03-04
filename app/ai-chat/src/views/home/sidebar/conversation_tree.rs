use crate::{
    components::{add_conversation::add_conversation_dialog, add_folder::add_folder_dialog},
    database::{Conversation, Folder},
    store::{ChatData, ChatDataEvent, ChatDataInner},
};
use super::{conversation_item::ConversationTreeItem, folder_item::FolderTreeItem};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Collapsible, Icon, IconName, Side,
    h_flex, label::Label,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    v_flex,
};
use serde::Deserialize;
use std::ops::Deref;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DropState {
    Valid,
    InvalidDescendant,
    Noop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(super) enum ActiveDropTarget {
    Root,
    Folder { id: i32, invalid: bool },
}

#[derive(Action, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = conversation_tree, no_json)]
struct SetDropTarget(pub ActiveDropTarget);

#[derive(Action, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = conversation_tree, no_json)]
struct ClearDropTarget(pub ActiveDropTarget);

actions!(conversation_tree, [ResetDropTarget]);

impl ActiveDropTarget {
    fn folder(id: i32, invalid: bool) -> Self {
        Self::Folder { id, invalid }
    }
}

const DROP_TARGET_KEY: (&str, usize) = ("conversation-tree-drop-target", 0);

#[derive(IntoElement)]
pub(super) struct ConversationTree {
    collapsed: bool,
    folders: Vec<Folder>,
    conversations: Vec<Conversation>,
    active_conversation_id: Option<i32>,
    root_label: SharedString,
}

impl ConversationTree {
    pub(super) fn empty_with_label(root_label: SharedString) -> Self {
        Self {
            collapsed: false,
            folders: Vec::new(),
            conversations: Vec::new(),
            active_conversation_id: None,
            root_label,
        }
    }

    pub(super) fn new(data: &ChatDataInner, root_label: SharedString) -> Self {
        Self {
            collapsed: false,
            folders: data.folders.clone(),
            conversations: data.conversations.clone(),
            active_conversation_id: data.active_tab_key().filter(|id| *id > 0),
            root_label,
        }
    }
}

impl Collapsible for ConversationTree {
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl RenderOnce for ConversationTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root_label = self.root_label.clone();
        let drop_target =
            window.use_keyed_state(DROP_TARGET_KEY, cx, |_, _| Option::<ActiveDropTarget>::None);
        let active_drop_target = *drop_target.read(cx);
        let root_is_highlighted = active_drop_target == Some(ActiveDropTarget::Root);
        v_flex()
            .id("conversation-tree")
            .focusable()
            .gap_2()
            .border_1()
            .border_color(cx.theme().transparent)
            .rounded(cx.theme().radius)
            .p_1()
            .when(root_is_highlighted, |this| {
                this.bg(cx.theme().drop_target)
                    .border_color(cx.theme().drag_border)
            })
            .on_action({
                let drop_target = drop_target.clone();
                move |action: &SetDropTarget, _window, cx| {
                    drop_target.update(cx, |current, cx| {
                        if *current != Some(action.0) {
                            *current = Some(action.0);
                            cx.notify();
                        }
                    });
                }
            })
            .on_action({
                let drop_target = drop_target.clone();
                move |action: &ClearDropTarget, _window, cx| {
                    drop_target.update(cx, |current, cx| {
                        if *current == Some(action.0) {
                            *current = None;
                            cx.notify();
                        }
                    });
                }
            })
            .on_action({
                let drop_target = drop_target.clone();
                move |_: &ResetDropTarget, _window, cx| {
                    drop_target.update(cx, |current, cx| {
                        if current.is_some() {
                            *current = None;
                            cx.notify();
                        }
                    });
                }
            })
            .children(
                self.folders
                    .into_iter()
                    .map(|folder| {
                        FolderTreeItem::new(
                            folder,
                            self.collapsed,
                            0,
                            self.active_conversation_id,
                            active_drop_target,
                        )
                        .into_any_element()
                    })
                    .chain(self.conversations.into_iter().map(|conversation| {
                        ConversationTreeItem::new(
                            conversation,
                            self.collapsed,
                            0,
                            self.active_conversation_id,
                            None,
                            active_drop_target == Some(ActiveDropTarget::Root),
                        )
                        .into_any_element()
                    })),
            )
            .child(
                div()
                    .min_h(px(52.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(root_label)
                    .on_drag_move::<DragConversationTreeItem>({
                        move |event, window, cx| {
                            let target = target_for_root(event.drag(cx));
                            if event.bounds.contains(&event.event.position) {
                                match target {
                            Some(target) => set_drop_target(window, cx, target),
                            None => reset_drop_target(window, cx),
                        }
                    }
                }
            })
                    .on_drop(move |drag: &DragConversationTreeItem, window, cx| {
                        cx.stop_propagation();
                        reset_drop_target(window, cx);
                        if root_drop_state(drag) != DropState::Valid {
                            return;
                        }
                        drag.move_to_root(cx);
                    })
                    .context_menu(root_context_menu),
            )
            .context_menu(root_context_menu)
    }
}

#[derive(Clone)]
enum DragConversationTreeKind {
    Folder {
        id: i32,
        name: SharedString,
        path: SharedString,
    },
    Conversation {
        id: i32,
        icon: SharedString,
        title: SharedString,
    },
}

#[derive(Clone)]
pub(crate) struct DragConversationTreeItem {
    kind: DragConversationTreeKind,
    source_parent_id: Option<i32>,
}

impl DragConversationTreeItem {
    pub(super) fn folder(folder: &Folder) -> Self {
        Self {
            kind: DragConversationTreeKind::Folder {
                id: folder.id,
                name: folder.name.clone().into(),
                path: folder.path.clone().into(),
            },
            source_parent_id: folder.parent_id,
        }
    }

    pub(super) fn conversation(conversation: &Conversation) -> Self {
        Self {
            kind: DragConversationTreeKind::Conversation {
                id: conversation.id,
                icon: conversation.icon.clone().into(),
                title: conversation.title.clone().into(),
            },
            source_parent_id: conversation.folder_id,
        }
    }

    pub(crate) fn conversation_id(&self) -> Option<i32> {
        match self.kind {
            DragConversationTreeKind::Conversation { id, .. } => Some(id),
            DragConversationTreeKind::Folder { .. } => None,
        }
    }

    pub(super) fn move_to_root(&self, cx: &mut App) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, |_this, cx| match &self.kind {
            DragConversationTreeKind::Folder { id, .. } => {
                cx.emit(ChatDataEvent::MoveFolder {
                    folder_id: *id,
                    target_parent_id: None,
                });
            }
            DragConversationTreeKind::Conversation { id, .. } => {
                cx.emit(ChatDataEvent::MoveConversation {
                    conversation_id: *id,
                    target_folder_id: None,
                });
            }
        });
    }

    pub(super) fn move_to_folder(&self, target_folder_id: i32, cx: &mut App) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, |_this, cx| match &self.kind {
            DragConversationTreeKind::Folder { id, .. } => {
                cx.emit(ChatDataEvent::MoveFolder {
                    folder_id: *id,
                    target_parent_id: Some(target_folder_id),
                });
            }
            DragConversationTreeKind::Conversation { id, .. } => {
                cx.emit(ChatDataEvent::MoveConversation {
                    conversation_id: *id,
                    target_folder_id: Some(target_folder_id),
                });
            }
        });
    }

    fn is_already_in_root(&self) -> bool {
        self.source_parent_id.is_none()
    }
}

pub(super) fn root_drop_state(drag: &DragConversationTreeItem) -> DropState {
    if drag.is_already_in_root() {
        DropState::Noop
    } else {
        DropState::Valid
    }
}

pub(super) fn folder_drop_state(
    drag: &DragConversationTreeItem,
    target_folder_id: i32,
    target_folder_path: &str,
) -> DropState {
    match &drag.kind {
        DragConversationTreeKind::Folder {
            id: drag_id,
            path: drag_path,
            ..
        } => {
            if *drag_id == target_folder_id || target_folder_path == drag_path.as_ref() {
                DropState::Noop
            } else if target_folder_path.starts_with(&format!("{drag_path}/")) {
                DropState::InvalidDescendant
            } else if drag.source_parent_id == Some(target_folder_id) {
                DropState::Noop
            } else {
                DropState::Valid
            }
        }
        DragConversationTreeKind::Conversation { .. } => {
            if drag.source_parent_id == Some(target_folder_id) {
                DropState::Noop
            } else {
                DropState::Valid
            }
        }
    }
}

pub(super) fn target_for_root(drag: &DragConversationTreeItem) -> Option<ActiveDropTarget> {
    match root_drop_state(drag) {
        DropState::Valid => Some(ActiveDropTarget::Root),
        DropState::InvalidDescendant | DropState::Noop => None,
    }
}

pub(super) fn target_for_folder(
    drag: &DragConversationTreeItem,
    target_folder_id: i32,
    target_folder_path: &str,
) -> Option<ActiveDropTarget> {
    match folder_drop_state(drag, target_folder_id, target_folder_path) {
        DropState::Valid => Some(ActiveDropTarget::folder(target_folder_id, false)),
        DropState::InvalidDescendant => Some(ActiveDropTarget::folder(target_folder_id, true)),
        DropState::Noop => None,
    }
}

pub(super) fn folder_block_drop_target(
    active_drop_target: Option<ActiveDropTarget>,
    folder_id: i32,
) -> Option<bool> {
    match active_drop_target {
        Some(ActiveDropTarget::Folder { id, invalid }) if id == folder_id => Some(invalid),
        _ => None,
    }
}

impl Render for DragConversationTreeItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (icon, label) = match &self.kind {
            DragConversationTreeKind::Folder { name, .. } => (SharedString::from(""), name.clone()),
            DragConversationTreeKind::Conversation { icon, title, .. } => {
                (icon.clone(), title.clone())
            }
        };

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
            .child(match &self.kind {
                DragConversationTreeKind::Folder { .. } => {
                    Icon::new(IconName::Folder).size_3().into_any_element()
                }
                DragConversationTreeKind::Conversation { .. } => Label::new(&icon)
                    .text_xs()
                    .line_height(rems(0.75))
                    .into_any_element(),
            })
            .child(Label::new(label).text_xs().line_height(rems(0.75)))
    }
}

fn root_context_menu(
    menu: PopupMenu,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    menu.check_side(Side::Left)
        .item(
            PopupMenuItem::new("Add Conversation")
                .icon(IconName::Plus)
                .on_click(|_, window, cx| add_conversation_dialog(None, window, cx)),
        )
        .item(
            PopupMenuItem::new("Add Folder")
                .icon(IconName::Plus)
                .on_click(|_, window, cx| add_folder_dialog(None, window, cx)),
        )
}

pub(super) fn set_drop_target(
    window: &mut Window,
    cx: &mut App,
    target: ActiveDropTarget,
) {
    window.dispatch_action(SetDropTarget(target).boxed_clone(), cx);
}

pub(super) fn clear_drop_target(
    window: &mut Window,
    cx: &mut App,
    target: ActiveDropTarget,
) {
    window.dispatch_action(ClearDropTarget(target).boxed_clone(), cx);
}

pub(super) fn reset_drop_target(window: &mut Window, cx: &mut App) {
    window.dispatch_action(ResetDropTarget.boxed_clone(), cx);
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveDropTarget, DragConversationTreeItem, DragConversationTreeKind, DropState,
        folder_block_drop_target, folder_drop_state, root_drop_state, target_for_folder,
        target_for_root,
    };
    use gpui::SharedString;

    fn folder_drag(id: i32, parent_id: Option<i32>, path: &str) -> DragConversationTreeItem {
        DragConversationTreeItem {
            kind: DragConversationTreeKind::Folder {
                id,
                name: SharedString::from(format!("Folder {id}")),
                path: SharedString::from(path.to_string()),
            },
            source_parent_id: parent_id,
        }
    }

    fn conversation_drag(id: i32, parent_id: Option<i32>) -> DragConversationTreeItem {
        DragConversationTreeItem {
            kind: DragConversationTreeKind::Conversation {
                id,
                icon: SharedString::from("🤖"),
                title: SharedString::from(format!("Conversation {id}")),
            },
            source_parent_id: parent_id,
        }
    }

    #[test]
    fn root_drop_is_noop_for_root_items() {
        let drag = conversation_drag(1, None);
        assert_eq!(root_drop_state(&drag), DropState::Noop);
        assert_eq!(target_for_root(&drag), None);
    }

    #[test]
    fn root_drop_accepts_nested_items() {
        let drag = conversation_drag(1, Some(2));
        assert_eq!(root_drop_state(&drag), DropState::Valid);
        assert_eq!(target_for_root(&drag), Some(ActiveDropTarget::Root));
    }

    #[test]
    fn folder_drop_is_noop_for_same_parent_conversation() {
        let drag = conversation_drag(1, Some(2));
        assert_eq!(folder_drop_state(&drag, 2, "/root/folder"), DropState::Noop);
        assert_eq!(target_for_folder(&drag, 2, "/root/folder"), None);
    }

    #[test]
    fn folder_drop_marks_descendant_folder_invalid() {
        let drag = folder_drag(2, Some(1), "/root/folder");
        assert_eq!(
            folder_drop_state(&drag, 3, "/root/folder/child"),
            DropState::InvalidDescendant
        );
        assert_eq!(
            target_for_folder(&drag, 3, "/root/folder/child"),
            Some(ActiveDropTarget::Folder {
                id: 3,
                invalid: true,
            })
        );
    }

    #[test]
    fn folder_drop_is_noop_for_self_target() {
        let drag = folder_drag(2, Some(1), "/root/folder");
        assert_eq!(folder_drop_state(&drag, 2, "/root/folder"), DropState::Noop);
        assert_eq!(target_for_folder(&drag, 2, "/root/folder"), None);
    }

    #[test]
    fn folder_drop_accepts_new_parent_for_conversation() {
        let drag = conversation_drag(1, Some(2));
        assert_eq!(folder_drop_state(&drag, 3, "/root/other"), DropState::Valid);
        assert_eq!(
            target_for_folder(&drag, 3, "/root/other"),
            Some(ActiveDropTarget::Folder {
                id: 3,
                invalid: false,
            })
        );
    }

    #[test]
    fn folder_drop_accepts_new_parent_for_folder() {
        let drag = folder_drag(2, Some(1), "/root/folder");
        assert_eq!(
            folder_drop_state(&drag, 5, "/root/target"),
            DropState::Valid
        );
        assert_eq!(
            target_for_folder(&drag, 5, "/root/target"),
            Some(ActiveDropTarget::Folder {
                id: 5,
                invalid: false,
            })
        );
    }

    #[test]
    fn folder_drop_is_noop_for_same_parent_folder() {
        let drag = folder_drag(2, Some(1), "/root/folder");
        assert_eq!(folder_drop_state(&drag, 1, "/root"), DropState::Noop);
        assert_eq!(target_for_folder(&drag, 1, "/root"), None);
    }

    #[test]
    fn folder_block_drop_target_matches_exact_folder_only() {
        assert_eq!(
            folder_block_drop_target(
                Some(ActiveDropTarget::Folder {
                    id: 2,
                    invalid: false
                }),
                2
            ),
            Some(false)
        );
        assert_eq!(
            folder_block_drop_target(
                Some(ActiveDropTarget::Folder {
                    id: 2,
                    invalid: false
                }),
                3
            ),
            None
        );
    }

    #[test]
    fn conversation_drag_exposes_conversation_id_only() {
        assert_eq!(conversation_drag(7, None).conversation_id(), Some(7));
        assert_eq!(folder_drag(3, None, "/root/folder").conversation_id(), None);
    }
}
