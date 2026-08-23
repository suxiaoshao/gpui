use crate::foundation::assets::IconName;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    kbd::Kbd,
    label::Label,
    menu::{ContextMenuExt, DropdownMenu},
};
use jaco_core::ConversationId;
use std::rc::Rc;

use super::super::workspace::{
    HomeRoute, HomeWorkspace, SidebarConversationNode, SidebarProjectNode,
};
use super::actions::{
    ConversationSidebarAction, ConversationSidebarActions, ProjectSidebarAction,
    ProjectSidebarActions,
};
use super::menu;

type ShortcutActionHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const ACTION_SUFFIX_WIDTH: Pixels = px(56.);
const SHORTCUT_SUFFIX_WIDTH: Pixels = px(56.);
const ACTION_HOVER_PADDING: Pixels = px(64.);
const CONVERSATION_DIRECT_ACTIONS: [ConversationSidebarAction; 2] = [
    ConversationSidebarAction::TogglePinned,
    ConversationSidebarAction::Archive,
];

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

fn action_overlay(width: Pixels) -> Div {
    h_flex()
        .absolute()
        .top_0()
        .right_2()
        .bottom_0()
        .w(width)
        .items_center()
        .justify_end()
        .gap_1()
}

fn reveal_action_button(button: Button, group: impl Into<SharedString>) -> Button {
    button
        .opacity(0.)
        .group_hover(group, |this| this.opacity(1.))
        .focus_visible(|this| this.opacity(1.))
}

#[derive(Clone)]
pub(super) struct ShortcutSidebarAction {
    id: SharedString,
    label: SharedString,
    icon: IconName,
    keystroke: &'static str,
    enabled: bool,
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
            enabled: true,
            handler: Rc::new(handler),
        }
    }

    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
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
        let enabled = self.action.enabled;
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
            .when(enabled, |this| {
                this.cursor_pointer().hover(|this| {
                    this.bg(cx.theme().tokens.sidebar_accent.background.opacity(0.8))
                        .text_color(cx.theme().sidebar_accent_foreground)
                        .pr(ACTION_HOVER_PADDING)
                })
            })
            .when(!enabled, |this| this.opacity(0.5))
            .when(enabled, |this| {
                this.on_click(move |event, window, cx| {
                    handler(event, window, cx);
                })
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
    workspace: Entity<HomeWorkspace>,
}

impl ProjectSidebarRow {
    pub(super) fn new(node: SidebarProjectNode, workspace: Entity<HomeWorkspace>) -> Self {
        Self { node, workspace }
    }
}

impl RenderOnce for ProjectSidebarRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let project = self.node.project.clone();
        let project_id = project.id.clone();
        let group = format!("sidebar-project-group-{project_id}");
        let workspace_for_toggle = self.workspace.clone();
        let project_id_for_toggle = project_id.clone();
        let project_actions =
            ProjectSidebarActions::new(project.clone(), self.workspace.clone(), cx);
        let can_create_conversation =
            project_actions.availability(ProjectSidebarAction::NewConversation);
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
                this.bg(cx.theme().tokens.sidebar_accent.background.opacity(0.8))
                    .text_color(cx.theme().sidebar_accent_foreground)
                    .pr(ACTION_HOVER_PADDING)
            })
            .on_click(move |_, _window, cx| {
                workspace_for_toggle.update(cx, |workspace, cx| {
                    workspace.toggle_project(&project_id_for_toggle, cx);
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
                action_overlay(ACTION_SUFFIX_WIDTH)
                    .child(
                        reveal_action_button(
                            Button::new(format!("sidebar-project-more-{project_id}"))
                                .icon(IconName::Ellipsis)
                                .ghost()
                                .xsmall()
                                .tooltip(more_tooltip)
                                .on_click(|_, _window, cx| cx.stop_propagation()),
                            group.clone(),
                        )
                        .dropdown_menu({
                            let actions = project_actions.clone();
                            move |menu, window, cx| {
                                menu::project_popup_menu(menu, actions.clone(), window, cx)
                            }
                        }),
                    )
                    .child(reveal_action_button(
                        Button::new(format!("sidebar-project-new-{project_id}"))
                            .icon(IconName::SquarePen)
                            .ghost()
                            .xsmall()
                            .disabled(!can_create_conversation)
                            .tooltip(new_tooltip)
                            .on_click({
                                let actions = project_actions.clone();
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    actions.invoke(
                                        ProjectSidebarAction::NewConversation,
                                        window,
                                        cx,
                                    );
                                }
                            }),
                        group.clone(),
                    )),
            )
            .context_menu({
                let actions = project_actions.clone();
                move |menu, window, cx| menu::project_popup_menu(menu, actions.clone(), window, cx)
            })
    }
}

#[derive(IntoElement)]
pub(super) struct ConversationSidebarRow {
    conversation: SidebarConversationNode,
    active: bool,
    workspace: Entity<HomeWorkspace>,
}

impl ConversationSidebarRow {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        active: bool,
        workspace: Entity<HomeWorkspace>,
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
        let conversation_id_for_open = conversation_id.clone();
        let conversation_actions =
            ConversationSidebarActions::new(self.conversation.clone(), self.workspace.clone(), cx);
        let pin_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t(if self.conversation.pinned {
                "sidebar-conversation-unpin"
            } else {
                "sidebar-conversation-pin"
            });
        let archive_tooltip = cx
            .global::<crate::foundation::I18n>()
            .t("sidebar-conversation-archive");
        let [toggle_pinned_action, archive_action] = CONVERSATION_DIRECT_ACTIONS;
        let can_toggle_pinned = conversation_actions.availability(toggle_pinned_action);
        let can_archive = conversation_actions.availability(archive_action);
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
                    .bg(cx.theme().tokens.sidebar_accent.background)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .hover({
                let active = self.active;
                move |this| {
                    let this = this.pr(ACTION_HOVER_PADDING);
                    if active {
                        this
                    } else {
                        this.bg(cx.theme().tokens.sidebar_accent.background.opacity(0.8))
                            .text_color(cx.theme().sidebar_accent_foreground)
                    }
                }
            })
            .on_click(move |_, _window, cx| {
                workspace_for_open.update(cx, |workspace, cx| {
                    workspace.open_conversation(conversation_id_for_open.clone(), cx);
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
                action_overlay(ACTION_SUFFIX_WIDTH)
                    .child(reveal_action_button(
                        Button::new(format!("sidebar-conversation-pin-{conversation_id}"))
                            .icon(if is_pinned {
                                IconName::PinOff
                            } else {
                                IconName::Pin
                            })
                            .ghost()
                            .xsmall()
                            .disabled(!can_toggle_pinned)
                            .tooltip(pin_tooltip)
                            .on_click({
                                let actions = conversation_actions.clone();
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    actions.invoke(toggle_pinned_action, window, cx);
                                }
                            }),
                        group.clone(),
                    ))
                    .child(reveal_action_button(
                        Button::new(format!("sidebar-conversation-archive-{conversation_id}"))
                            .icon(IconName::Archive)
                            .ghost()
                            .xsmall()
                            .disabled(!can_archive)
                            .tooltip(archive_tooltip)
                            .on_click({
                                let actions = conversation_actions.clone();
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    actions.invoke(archive_action, window, cx);
                                }
                            }),
                        group.clone(),
                    )),
            )
            .context_menu({
                let actions = conversation_actions.clone();
                move |menu, window, cx| {
                    menu::conversation_popup_menu(menu, actions.clone(), window, cx)
                }
            })
    }
}

pub(super) fn project_row(
    node: SidebarProjectNode,
    workspace: Entity<HomeWorkspace>,
    _cx: &mut App,
) -> AnyElement {
    ProjectSidebarRow::new(node, workspace).into_any_element()
}

pub(super) fn conversation_row(
    conversation: SidebarConversationNode,
    active: bool,
    workspace: Entity<HomeWorkspace>,
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

#[cfg(test)]
mod tests {
    use super::{CONVERSATION_DIRECT_ACTIONS, ConversationSidebarAction};

    #[test]
    fn conversation_row_exposes_pin_and_archive_as_direct_actions() {
        assert_eq!(
            CONVERSATION_DIRECT_ACTIONS,
            [
                ConversationSidebarAction::TogglePinned,
                ConversationSidebarAction::Archive,
            ]
        );
    }
}
