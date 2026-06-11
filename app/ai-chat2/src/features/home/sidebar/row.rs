use crate::{
    foundation::assets::IconName,
    state::{self, HomeRoute},
};
use ai_chat_core::ConversationId;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    kbd::Kbd,
    label::Label,
    menu::DropdownMenu,
};
use std::rc::Rc;

use super::menu;
use crate::state::workspace::{SidebarConversationNode, SidebarProjectNode};

type ShortcutActionHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const ACTION_SUFFIX_WIDTH: Pixels = px(56.);
const SHORTCUT_SUFFIX_WIDTH: Pixels = px(56.);
const ACTION_HOVER_PADDING: Pixels = px(64.);

fn hover_action_overlay(group: impl Into<SharedString>, width: Pixels) -> Div {
    h_flex()
        .absolute()
        .top_0()
        .right_2()
        .bottom_0()
        .w(width)
        .opacity(0.)
        .items_center()
        .justify_end()
        .gap_1()
        .group_hover(group, |this| this.opacity(1.))
}

#[derive(Clone)]
pub(super) struct ShortcutSidebarAction {
    id: SharedString,
    label: SharedString,
    icon: IconName,
    keystroke: &'static str,
    handler: ShortcutActionHandler,
}

impl ShortcutSidebarAction {
    pub(super) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        icon: IconName,
        keystroke: &'static str,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            keystroke,
            handler: Rc::new(handler),
        }
    }

    pub(super) fn render(self, _cx: &mut App) -> AnyElement {
        ShortcutSidebarActionRow { action: self }.into_any_element()
    }
}

#[derive(IntoElement)]
pub(super) struct ShortcutSidebarActionRow {
    action: ShortcutSidebarAction,
}

impl RenderOnce for ShortcutSidebarActionRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = format!("sidebar-shortcut-action-group-{}", self.action.id);
        let handler = self.action.handler.clone();
        let keystroke = Keystroke::parse(self.action.keystroke).ok();

        h_flex()
            .id(self.action.id)
            .group(group.clone())
            .relative()
            .w_full()
            .min_w_0()
            .h_7()
            .p_2()
            .items_center()
            .gap_x_2()
            .overflow_hidden()
            .flex_shrink_0()
            .rounded(cx.theme().radius)
            .text_sm()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .cursor_pointer()
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent.opacity(0.8))
                    .text_color(cx.theme().sidebar_accent_foreground)
                    .pr(ACTION_HOVER_PADDING)
            })
            .on_click(move |event, window, cx| {
                handler(event, window, cx);
            })
            .child(Icon::new(self.action.icon).size_4().flex_none())
            .child(
                h_flex().flex_1().min_w_0().items_center().child(
                    Label::new(self.action.label)
                        .text_sm()
                        .truncate()
                        .flex_1()
                        .min_w_0(),
                ),
            )
            .when_some(keystroke, |this, keystroke| {
                this.child(
                    hover_action_overlay(group, SHORTCUT_SUFFIX_WIDTH).child(Kbd::new(keystroke)),
                )
            })
    }
}

#[derive(IntoElement)]
pub(super) struct ProjectSidebarRow {
    node: SidebarProjectNode,
    workspace: Entity<state::AiChat2WorkspaceStore>,
}

impl ProjectSidebarRow {
    pub(super) fn new(
        node: SidebarProjectNode,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    ) -> Self {
        Self { node, workspace }
    }
}

impl RenderOnce for ProjectSidebarRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let project = self.node.project.clone();
        let project_id = project.id.clone();
        let group = format!("sidebar-project-group-{project_id}");
        let workspace_for_toggle = self.workspace.clone();
        let workspace_for_new = self.workspace.clone();
        let new_project_id = project_id.clone();
        let more_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t("sidebar-project-more-tooltip");
        let new_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t("sidebar-project-new-conversation-tooltip");

        h_flex()
            .id(format!("sidebar-project-row-{project_id}"))
            .group(group.clone())
            .relative()
            .w_full()
            .min_w_0()
            .h_7()
            .p_2()
            .items_center()
            .gap_x_2()
            .overflow_hidden()
            .flex_shrink_0()
            .rounded(cx.theme().radius)
            .text_sm()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .cursor_pointer()
            .hover(|this| {
                this.bg(cx.theme().sidebar_accent.opacity(0.8))
                    .text_color(cx.theme().sidebar_accent_foreground)
                    .pr(ACTION_HOVER_PADDING)
            })
            .on_click(move |_, _window, cx| {
                workspace_for_toggle.update(cx, |workspace, cx| {
                    workspace.toggle_project(&project_id, cx);
                });
            })
            .child(
                Icon::new(if self.node.is_expanded {
                    IconName::FolderOpen
                } else {
                    IconName::Folder
                })
                .size_4()
                .flex_none(),
            )
            .child(
                h_flex().flex_1().items_center().min_w_0().child(
                    Label::new(project.display_name.clone())
                        .text_sm()
                        .truncate()
                        .flex_1()
                        .min_w_0(),
                ),
            )
            .child(
                hover_action_overlay(group.clone(), ACTION_SUFFIX_WIDTH)
                    .child(
                        Button::new(format!("sidebar-project-more-{new_project_id}"))
                            .icon(IconName::Ellipsis)
                            .ghost()
                            .xsmall()
                            .tooltip(more_tooltip)
                            .on_click(|_, _window, cx| cx.stop_propagation())
                            .dropdown_menu({
                                let project = project.clone();
                                move |menu, window, cx| {
                                    menu::project_popup_menu(menu, project.clone(), window, cx)
                                }
                            }),
                    )
                    .child(
                        Button::new(format!("sidebar-project-new-{new_project_id}"))
                            .icon(IconName::SquarePen)
                            .ghost()
                            .xsmall()
                            .tooltip(new_tooltip)
                            .on_click(move |_, _window, cx| {
                                cx.stop_propagation();
                                workspace_for_new.update(cx, |workspace, cx| {
                                    workspace.new_conversation_in_project(&new_project_id, cx);
                                });
                            }),
                    ),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct ConversationSidebarRow {
    conversation: SidebarConversationNode,
    active: bool,
    workspace: Entity<state::AiChat2WorkspaceStore>,
}

impl ConversationSidebarRow {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        active: bool,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    ) -> Self {
        Self {
            conversation,
            active,
            workspace,
        }
    }
}

impl RenderOnce for ConversationSidebarRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let conversation_id = self.conversation.id.clone();
        let group = format!("sidebar-conversation-group-{conversation_id}");
        let workspace_for_open = self.workspace.clone();
        let workspace_for_pin = self.workspace.clone();
        let pin_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t(if self.conversation.pinned {
                "sidebar-conversation-unpin-tooltip"
            } else {
                "sidebar-conversation-pin-tooltip"
            });
        let delete_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t("sidebar-conversation-delete-tooltip");
        let pin_conversation_id = conversation_id.clone();
        let delete_conversation_id = conversation_id.clone();
        let delete_conversation = self.conversation.clone();
        let is_pinned = self.conversation.pinned;

        h_flex()
            .id(format!("sidebar-conversation-row-{conversation_id}"))
            .group(group.clone())
            .relative()
            .w_full()
            .min_w_0()
            .h_7()
            .p_2()
            .items_center()
            .gap_x_2()
            .overflow_hidden()
            .flex_shrink_0()
            .rounded(cx.theme().radius)
            .text_sm()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .cursor_pointer()
            .when(self.active, |this| {
                this.font_medium()
                    .bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .hover({
                let active = self.active;
                move |this| {
                    let this = this.pr(ACTION_HOVER_PADDING);
                    if active {
                        this
                    } else {
                        this.bg(cx.theme().sidebar_accent.opacity(0.8))
                            .text_color(cx.theme().sidebar_accent_foreground)
                    }
                }
            })
            .on_click(move |_, _window, cx| {
                workspace_for_open.update(cx, |workspace, cx| {
                    workspace.open_conversation(conversation_id.clone(), cx);
                });
            })
            .child(
                h_flex().flex_1().items_center().min_w_0().child(
                    Label::new(self.conversation.title.clone())
                        .text_sm()
                        .truncate()
                        .flex_1()
                        .min_w_0(),
                ),
            )
            .child(
                hover_action_overlay(group.clone(), ACTION_SUFFIX_WIDTH)
                    .child(
                        Button::new(format!("sidebar-conversation-pin-{pin_conversation_id}"))
                            .icon(if self.conversation.pinned {
                                IconName::PinOff
                            } else {
                                IconName::Pin
                            })
                            .ghost()
                            .xsmall()
                            .tooltip(pin_tooltip)
                            .on_click(move |_, _window, cx| {
                                cx.stop_propagation();
                                let pinned = !is_pinned;
                                workspace_for_pin.update(cx, |workspace, cx| {
                                    let _ = workspace.pin_conversation(
                                        &pin_conversation_id,
                                        pinned,
                                        cx,
                                    );
                                });
                            }),
                    )
                    .child(
                        Button::new(format!(
                            "sidebar-conversation-delete-{delete_conversation_id}"
                        ))
                        .icon(IconName::Trash)
                        .ghost()
                        .xsmall()
                        .tooltip(delete_tooltip)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            menu::open_delete_conversation_confirm(
                                delete_conversation.clone(),
                                window,
                                cx,
                            );
                        }),
                    ),
            )
    }
}

pub(super) fn project_row(
    node: SidebarProjectNode,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    _cx: &mut App,
) -> AnyElement {
    ProjectSidebarRow::new(node, workspace).into_any_element()
}

pub(super) fn conversation_row(
    conversation: SidebarConversationNode,
    active: bool,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    _cx: &mut App,
) -> AnyElement {
    ConversationSidebarRow::new(conversation, active, workspace).into_any_element()
}

pub(super) fn route_matches_conversation(
    route: &HomeRoute,
    conversation_id: &ConversationId,
) -> bool {
    matches!(route, HomeRoute::Conversation(active_id) if active_id == conversation_id)
}
