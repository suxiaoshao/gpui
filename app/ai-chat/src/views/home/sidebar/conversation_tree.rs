use crate::{
    components::{
        add_conversation::add_conversation_dialog, add_folder::add_folder_dialog,
        delete_confirm::open_delete_confirm_dialog,
    },
    database::{Conversation, Folder},
    store::{ChatData, ChatDataEvent, ChatDataInner},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Collapsible, Icon, IconName, Side, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    v_flex,
};
use serde::Deserialize;
use std::ops::Deref;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropState {
    Valid,
    InvalidDescendant,
    Noop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
enum ActiveDropTarget {
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
                                    Some(target) => window
                                        .dispatch_action(SetDropTarget(target).boxed_clone(), cx),
                                    None => {
                                        window.dispatch_action(ResetDropTarget.boxed_clone(), cx)
                                    }
                                }
                            }
                        }
                    })
                    .on_drop(move |drag: &DragConversationTreeItem, window, cx| {
                        cx.stop_propagation();
                        window.dispatch_action(ResetDropTarget.boxed_clone(), cx);
                        if root_drop_state(drag) != DropState::Valid {
                            return;
                        }
                        let chat_data = cx.global::<ChatData>().deref().clone();
                        chat_data.update(cx, |_this, cx| match &drag.kind {
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
struct DragConversationTreeItem {
    kind: DragConversationTreeKind,
    source_parent_id: Option<i32>,
}

impl DragConversationTreeItem {
    fn folder(folder: &Folder) -> Self {
        Self {
            kind: DragConversationTreeKind::Folder {
                id: folder.id,
                name: folder.name.clone().into(),
                path: folder.path.clone().into(),
            },
            source_parent_id: folder.parent_id,
        }
    }

    fn conversation(conversation: &Conversation) -> Self {
        Self {
            kind: DragConversationTreeKind::Conversation {
                id: conversation.id,
                icon: conversation.icon.clone().into(),
                title: conversation.title.clone().into(),
            },
            source_parent_id: conversation.folder_id,
        }
    }

    fn is_already_in_root(&self) -> bool {
        self.source_parent_id.is_none()
    }
}

fn root_drop_state(drag: &DragConversationTreeItem) -> DropState {
    if drag.is_already_in_root() {
        DropState::Noop
    } else {
        DropState::Valid
    }
}

fn folder_drop_state(
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

fn target_for_root(drag: &DragConversationTreeItem) -> Option<ActiveDropTarget> {
    match root_drop_state(drag) {
        DropState::Valid => Some(ActiveDropTarget::Root),
        DropState::InvalidDescendant | DropState::Noop => None,
    }
}

fn target_for_folder(
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

fn folder_block_drop_target(
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

#[derive(IntoElement)]
struct FolderTreeItem {
    folder: Folder,
    collapsed: bool,
    depth: usize,
    active_conversation_id: Option<i32>,
    active_drop_target: Option<ActiveDropTarget>,
}

impl FolderTreeItem {
    fn new(
        folder: Folder,
        collapsed: bool,
        depth: usize,
        active_conversation_id: Option<i32>,
        active_drop_target: Option<ActiveDropTarget>,
    ) -> Self {
        Self {
            folder,
            collapsed,
            depth,
            active_conversation_id,
            active_drop_target,
        }
    }
}

impl RenderOnce for FolderTreeItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let folder = self.folder;
        let id = folder.id;
        let name = folder.name.clone();
        let path = folder.path.clone();
        let active_drop_target = self.active_drop_target;
        let block_drop_target = folder_block_drop_target(active_drop_target, id);
        let root_drop_target = active_drop_target == Some(ActiveDropTarget::Root);
        let open_state = window.use_keyed_state(
            ("conversation-tree-folder-open", id as usize),
            cx,
            |_, _| true,
        );
        let is_open = !self.collapsed && *open_state.read(cx);
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
                            .on_click({
                                let open_state = open_state.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    open_state.update(cx, |is_open, cx| {
                                        *is_open = !*is_open;
                                        cx.notify();
                                    });
                                }
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
                    .on_click({
                        let open_state = open_state.clone();
                        move |_event, _window, cx| {
                            open_state.update(cx, |is_open, cx| {
                                *is_open = !*is_open;
                                cx.notify();
                            });
                        }
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
                            Some(target) => {
                                window.dispatch_action(SetDropTarget(target).boxed_clone(), cx)
                            }
                            None => window.dispatch_action(ResetDropTarget.boxed_clone(), cx),
                        }
                    } else if let Some(target) = target {
                        window.dispatch_action(ClearDropTarget(target).boxed_clone(), cx);
                    }
                }
            })
            .on_drop({
                let path = path.clone();
                move |drag: &DragConversationTreeItem, window, cx| {
                    cx.stop_propagation();
                    window.dispatch_action(ResetDropTarget.boxed_clone(), cx);
                    if folder_drop_state(drag, id, &path) != DropState::Valid {
                        return;
                    }
                    match &drag.kind {
                        DragConversationTreeKind::Folder { id: drag_id, .. } => {
                            let chat_data = cx.global::<ChatData>().deref().clone();
                            chat_data.update(cx, |_this, cx| {
                                cx.emit(ChatDataEvent::MoveFolder {
                                    folder_id: *drag_id,
                                    target_parent_id: Some(id),
                                });
                            });
                        }
                        DragConversationTreeKind::Conversation {
                            id: conversation_id,
                            ..
                        } => {
                            let chat_data = cx.global::<ChatData>().deref().clone();
                            chat_data.update(cx, |_this, cx| {
                                cx.emit(ChatDataEvent::MoveConversation {
                                    conversation_id: *conversation_id,
                                    target_folder_id: Some(id),
                                });
                            });
                        }
                    }
                }
            })
            .when(is_open, |this| {
                this.child(
                    v_flex().gap_1().children(
                        folder
                            .folders
                            .into_iter()
                            .map(|child| {
                                FolderTreeItem::new(
                                    child,
                                    self.collapsed,
                                    self.depth + 1,
                                    self.active_conversation_id,
                                    active_drop_target,
                                )
                                .into_any_element()
                            })
                            .chain(folder.conversations.into_iter().map(|conversation| {
                                ConversationTreeItem::new(
                                    conversation,
                                    self.collapsed,
                                    self.depth + 1,
                                    self.active_conversation_id,
                                    Some((id, path.clone().into())),
                                    root_drop_target,
                                )
                                .into_any_element()
                            })),
                    ),
                )
            })
    }
}

#[derive(IntoElement)]
struct ConversationTreeItem {
    conversation: Conversation,
    collapsed: bool,
    depth: usize,
    active_conversation_id: Option<i32>,
    target_folder: Option<(i32, SharedString)>,
    root_drop_target: bool,
}

impl ConversationTreeItem {
    fn new(
        conversation: Conversation,
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
                            Some(target) => {
                                window.dispatch_action(SetDropTarget(target).boxed_clone(), cx)
                            }
                            None => window.dispatch_action(ResetDropTarget.boxed_clone(), cx),
                        }
                    } else if is_root_conversation && let Some(target) = target {
                        window.dispatch_action(ClearDropTarget(target).boxed_clone(), cx);
                    }
                }
            })
            .on_drop({
                let target_folder = target_folder.clone();
                move |drag: &DragConversationTreeItem, window, cx| {
                    cx.stop_propagation();
                    window.dispatch_action(ResetDropTarget.boxed_clone(), cx);
                    if let Some((folder_id, folder_path)) = target_folder.as_ref() {
                        if folder_drop_state(drag, *folder_id, folder_path) != DropState::Valid {
                            return;
                        }
                        match &drag.kind {
                            DragConversationTreeKind::Folder { id: drag_id, .. } => {
                                let chat_data = cx.global::<ChatData>().deref().clone();
                                chat_data.update(cx, |_this, cx| {
                                    cx.emit(ChatDataEvent::MoveFolder {
                                        folder_id: *drag_id,
                                        target_parent_id: Some(*folder_id),
                                    });
                                });
                            }
                            DragConversationTreeKind::Conversation {
                                id: conversation_id,
                                ..
                            } => {
                                let chat_data = cx.global::<ChatData>().deref().clone();
                                chat_data.update(cx, |_this, cx| {
                                    cx.emit(ChatDataEvent::MoveConversation {
                                        conversation_id: *conversation_id,
                                        target_folder_id: Some(*folder_id),
                                    });
                                });
                            }
                        }
                    } else if root_drop_state(drag) == DropState::Valid {
                        let chat_data = cx.global::<ChatData>().deref().clone();
                        chat_data.update(cx, |_this, cx| match &drag.kind {
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

fn folder_popup_menu(
    menu: PopupMenu,
    folder: &Folder,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let id = folder.id;
    let name = folder.name.clone();
    menu.check_side(Side::Left)
        .item(
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

fn conversation_popup_menu(
    menu: PopupMenu,
    conversation: &Conversation,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let id = conversation.id;
    let title = conversation.title.clone();
    menu.check_side(Side::Left).item(
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
}
