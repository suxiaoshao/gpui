use super::{conversation_item::ConversationTreeItem, folder_item::FolderTreeItem};
use crate::{
    components::{add_conversation::add_conversation_dialog, add_folder::add_folder_dialog},
    database::{Conversation, Folder},
    state::{ChatData, ChatDataEvent, ChatDataInner},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Collapsible, Icon, IconName, Side, h_flex,
    label::Label,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    v_flex,
};
use serde::Deserialize;
use std::{collections::BTreeSet, ops::Deref};

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

    pub(super) fn is_root(self) -> bool {
        matches!(self, Self::Root)
    }
}

const DROP_TARGET_KEY: (&str, usize) = ("conversation-tree-drop-target", 0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SidebarConversationNode {
    pub(super) id: i32,
    pub(super) folder_id: Option<i32>,
    pub(super) title: SharedString,
    pub(super) icon: SharedString,
}

impl From<&Conversation> for SidebarConversationNode {
    fn from(conversation: &Conversation) -> Self {
        Self {
            id: conversation.id,
            folder_id: conversation.folder_id,
            title: conversation.title.clone().into(),
            icon: conversation.icon.clone().into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SidebarFolderNode {
    pub(super) id: i32,
    pub(super) parent_id: Option<i32>,
    pub(super) name: SharedString,
    pub(super) path: SharedString,
    pub(super) folders: Vec<SidebarFolderNode>,
    pub(super) conversations: Vec<SidebarConversationNode>,
}

impl From<&Folder> for SidebarFolderNode {
    fn from(folder: &Folder) -> Self {
        Self {
            id: folder.id,
            parent_id: folder.parent_id,
            name: folder.name.clone().into(),
            path: folder.path.clone().into(),
            folders: project_folders(&folder.folders),
            conversations: project_conversations(&folder.conversations),
        }
    }
}

fn project_folders(folders: &[Folder]) -> Vec<SidebarFolderNode> {
    folders.iter().map(Into::into).collect()
}

fn project_conversations(conversations: &[Conversation]) -> Vec<SidebarConversationNode> {
    conversations.iter().map(Into::into).collect()
}

#[derive(IntoElement)]
pub(super) struct ConversationTree {
    collapsed: bool,
    folders: Vec<SidebarFolderNode>,
    conversations: Vec<SidebarConversationNode>,
    active_conversation_id: Option<i32>,
    open_folder_ids: BTreeSet<i32>,
    root_label: SharedString,
}

impl ConversationTree {
    pub(super) fn empty_with_label(root_label: SharedString) -> Self {
        Self {
            collapsed: false,
            folders: Vec::new(),
            conversations: Vec::new(),
            active_conversation_id: None,
            open_folder_ids: BTreeSet::new(),
            root_label,
        }
    }

    pub(super) fn new(
        data: &ChatDataInner,
        active_conversation_id: Option<i32>,
        open_folder_ids: BTreeSet<i32>,
        root_label: SharedString,
    ) -> Self {
        Self {
            collapsed: false,
            folders: project_folders(&data.folders),
            conversations: project_conversations(&data.conversations),
            active_conversation_id,
            open_folder_ids,
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
        let collapsed = self.collapsed;
        let active_conversation_id = self.active_conversation_id;
        let open_folder_ids = self.open_folder_ids;
        let folders = self.folders;
        let conversations = self.conversations;
        let root_label = self.root_label.clone();
        let drop_target =
            window.use_keyed_state(DROP_TARGET_KEY, cx, |_, _| Option::<ActiveDropTarget>::None);
        let active_drop_target = *drop_target.read(cx);
        let root_is_highlighted = active_drop_target.is_some_and(ActiveDropTarget::is_root);
        v_flex()
            .id("conversation-tree")
            .focusable()
            .gap_1()
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
            .children(folders.into_iter().map(|folder| {
                FolderTreeItem::new(
                    folder,
                    collapsed,
                    0,
                    active_conversation_id,
                    &open_folder_ids,
                    active_drop_target,
                )
                .into_any_element()
            }))
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .when(!conversations.is_empty(), |this| {
                        this.child(v_flex().gap_1().children(conversations.into_iter().map(
                            move |conversation| {
                                ConversationTreeItem::new(
                                    conversation,
                                    collapsed,
                                    0,
                                    active_conversation_id,
                                    root_is_highlighted,
                                )
                                .into_any_element()
                            },
                        )))
                    })
                    .child(
                        div()
                            .min_h(px(52.))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(root_label),
                    )
                    .on_drag_move::<DragConversationTreeItem>({
                        move |event, window, cx| {
                            let target = target_for_root(event.drag(cx));
                            if event.bounds.contains(&event.event.position) {
                                match target {
                                    Some(target) => set_drop_target(window, cx, target),
                                    None => reset_drop_target(window, cx),
                                }
                            } else if let Some(target) = target {
                                clear_drop_target(window, cx, target);
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
    pub(super) fn folder(folder: &SidebarFolderNode) -> Self {
        Self {
            kind: DragConversationTreeKind::Folder {
                id: folder.id,
                name: folder.name.clone(),
                path: folder.path.clone(),
            },
            source_parent_id: folder.parent_id,
        }
    }

    pub(super) fn conversation(conversation: &SidebarConversationNode) -> Self {
        Self {
            kind: DragConversationTreeKind::Conversation {
                id: conversation.id,
                icon: conversation.icon.clone(),
                title: conversation.title.clone(),
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

pub(super) fn target_for_conversation_group(
    drag: &DragConversationTreeItem,
    target_folder: Option<(i32, &str)>,
) -> Option<ActiveDropTarget> {
    match target_folder {
        Some((folder_id, folder_path)) => target_for_folder(drag, folder_id, folder_path),
        None => target_for_root(drag),
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

pub(super) fn set_drop_target(window: &mut Window, cx: &mut App, target: ActiveDropTarget) {
    window.dispatch_action(SetDropTarget(target).boxed_clone(), cx);
}

pub(super) fn clear_drop_target(window: &mut Window, cx: &mut App, target: ActiveDropTarget) {
    window.dispatch_action(ClearDropTarget(target).boxed_clone(), cx);
}

pub(super) fn reset_drop_target(window: &mut Window, cx: &mut App) {
    window.dispatch_action(ResetDropTarget.boxed_clone(), cx);
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveDropTarget, DragConversationTreeItem, DragConversationTreeKind, DropState,
        folder_block_drop_target, folder_drop_state, project_conversations, project_folders,
        root_drop_state, target_for_conversation_group, target_for_folder, target_for_root,
    };
    use crate::database::{Content, Conversation, Folder, Message, Role, Status};
    use gpui::SharedString;
    use time::OffsetDateTime;

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn message(id: i32, conversation_id: i32) -> Message {
        Message {
            id,
            conversation_id,
            conversation_path: format!("/conversation-{conversation_id}"),
            provider: "OpenAI".to_string(),
            role: Role::User,
            content: Content::new(format!("message-{id}")),
            send_content: serde_json::json!({ "message_id": id }),
            status: Status::Normal,
            error: None,
            created_time: now(),
            updated_time: now(),
            start_time: now(),
            end_time: now(),
        }
    }

    fn conversation(
        id: i32,
        folder_id: Option<i32>,
        title: &str,
        icon: &str,
        messages: Vec<Message>,
    ) -> Conversation {
        Conversation {
            id,
            path: format!("/{title}"),
            folder_id,
            title: title.to_string(),
            icon: icon.to_string(),
            created_time: now(),
            updated_time: now(),
            info: Some(format!("info-{id}")),
            messages,
        }
    }

    fn folder(
        id: i32,
        parent_id: Option<i32>,
        name: &str,
        path: &str,
        conversations: Vec<Conversation>,
        folders: Vec<Folder>,
    ) -> Folder {
        Folder {
            id,
            name: name.to_string(),
            path: path.to_string(),
            parent_id,
            created_time: now(),
            updated_time: now(),
            conversations,
            folders,
        }
    }

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

    #[test]
    fn conversation_group_target_uses_root_for_root_group_gaps() {
        let drag = conversation_drag(7, Some(2));
        assert_eq!(
            target_for_conversation_group(&drag, None),
            Some(ActiveDropTarget::Root)
        );
    }

    #[test]
    fn conversation_group_target_uses_folder_for_folder_group_gaps() {
        let drag = conversation_drag(7, Some(2));
        assert_eq!(
            target_for_conversation_group(&drag, Some((3, "/root/folder"))),
            Some(ActiveDropTarget::Folder {
                id: 3,
                invalid: false
            })
        );
    }

    #[test]
    fn project_conversations_keeps_only_sidebar_fields() {
        let projected = project_conversations(&[conversation(
            7,
            Some(3),
            "Alpha",
            "A",
            vec![message(1, 7), message(2, 7)],
        )]);

        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].id, 7);
        assert_eq!(projected[0].folder_id, Some(3));
        assert_eq!(projected[0].title.as_ref(), "Alpha");
        assert_eq!(projected[0].icon.as_ref(), "A");
    }

    #[test]
    fn project_folders_preserves_nested_sidebar_tree_shape() {
        let projected = project_folders(&[folder(
            1,
            None,
            "root",
            "/root",
            vec![conversation(8, Some(1), "Leaf", "L", vec![message(2, 8)])],
            vec![folder(
                2,
                Some(1),
                "child",
                "/root/child",
                vec![conversation(9, Some(2), "Nested", "N", vec![message(3, 9)])],
                Vec::new(),
            )],
        )]);

        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].id, 1);
        assert_eq!(projected[0].name.as_ref(), "root");
        assert_eq!(projected[0].path.as_ref(), "/root");
        assert_eq!(projected[0].conversations.len(), 1);
        assert_eq!(projected[0].conversations[0].title.as_ref(), "Leaf");
        assert_eq!(projected[0].folders.len(), 1);
        assert_eq!(projected[0].folders[0].id, 2);
        assert_eq!(projected[0].folders[0].parent_id, Some(1));
        assert_eq!(projected[0].folders[0].path.as_ref(), "/root/child");
        assert_eq!(
            projected[0].folders[0].conversations[0].title.as_ref(),
            "Nested"
        );
    }
}
