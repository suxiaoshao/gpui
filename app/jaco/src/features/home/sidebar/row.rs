use crate::{
    features::conversation::runtime::{ConversationRuntimeStore, ConversationSidebarStatus},
    foundation::{assets::IconName, conversation_format::sidebar_relative_recency_label},
    state,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, ElementExt as _, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    hover_card::HoverCard,
    kbd::Kbd,
    label::Label,
    menu::{ContextMenuExt, DropdownMenu},
    spinner::Spinner,
    tooltip::Tooltip,
    v_flex,
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
const CONVERSATION_HOVER_CARD_GAP: Pixels = px(4.);
const CONVERSATION_DIRECT_ACTIONS: [ConversationSidebarAction; 2] = [
    ConversationSidebarAction::TogglePinned,
    ConversationSidebarAction::Archive,
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct ConversationHoverCardPlacement {
    anchor: Anchor,
    popover_offset: Point<Pixels>,
}

fn conversation_hover_card_placement(
    sidebar_width: Pixels,
    trigger_bounds: Bounds<Pixels>,
) -> ConversationHoverCardPlacement {
    ConversationHoverCardPlacement {
        anchor: Anchor::TopLeft,
        // HoverCard lays its top-anchored popover out after the trigger. Translate the entire
        // popover root so its top-left corner lands at the sidebar edge and trigger top.
        popover_offset: point(
            sidebar_width + CONVERSATION_HOVER_CARD_GAP - trigger_bounds.origin.x,
            -trigger_bounds.size.height,
        ),
    }
}

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
    runtime: Entity<ConversationRuntimeStore>,
}

impl ConversationSidebarRow {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        active: bool,
        workspace: Entity<HomeWorkspace>,
        runtime: Entity<ConversationRuntimeStore>,
    ) -> Self {
        Self {
            conversation,
            active,
            workspace,
            runtime,
        }
    }
}

impl RenderOnce for ConversationSidebarRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
        let status = self.runtime.read(cx).sidebar_status(&conversation_id);
        let status_slot = conversation_status_slot(status, group.clone(), &conversation_id, cx);
        let sidebar_width = cx
            .global::<state::LayoutStateStore>()
            .entity()
            .read(cx)
            .sidebar_width();
        let hover_background = cx.theme().tokens.sidebar_accent.background.opacity(0.8);
        let hover_foreground = cx.theme().sidebar_accent_foreground;
        let trigger = h_flex()
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
            .when(status != ConversationSidebarStatus::Idle, |this| {
                this.pr(ACTION_HOVER_PADDING)
            })
            .hover({
                let active = self.active;
                move |this| {
                    let this = this.pr(ACTION_HOVER_PADDING);
                    if active {
                        this
                    } else {
                        this.bg(hover_background).text_color(hover_foreground)
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
            .children(status_slot)
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
            .into_any_element();

        conversation_hover_card(&self.conversation, trigger, sidebar_width, window, cx)
    }
}

fn conversation_hover_card(
    conversation: &SidebarConversationNode,
    trigger: AnyElement,
    sidebar_width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(project_display_name) = conversation.project_display_name.clone() else {
        return trigger;
    };

    let id = format!("sidebar-conversation-hover-card-{}", conversation.id);
    let bounds_state = window.use_keyed_state(format!("{id}-trigger-bounds"), cx, |_, _| {
        Bounds::<Pixels>::default()
    });
    let trigger_bounds = *bounds_state.read(cx);
    let placement = conversation_hover_card_placement(sidebar_width, trigger_bounds);
    let bounds_state_for_prepaint = bounds_state.clone();
    let trigger = div()
        .w_full()
        .child(trigger)
        .on_prepaint(move |bounds, _, cx| {
            bounds_state_for_prepaint.update(cx, |current, cx| {
                if *current != bounds {
                    *current = bounds;
                    cx.notify();
                }
            });
        });
    let title = conversation.title.clone();
    let recency_at = conversation.recency_at;

    HoverCard::new(id)
        .anchor(placement.anchor)
        .left(placement.popover_offset.x)
        .top(placement.popover_offset.y)
        .trigger(trigger)
        .content(move |_, _, cx| {
            let relative_recency = sidebar_relative_recency_label(
                recency_at,
                time::OffsetDateTime::now_utc(),
                cx.global::<crate::foundation::I18n>(),
            );

            v_flex()
                .w(px(320.))
                .gap_2()
                .child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .child(
                            Label::new(title.clone())
                                .text_sm()
                                .whitespace_normal()
                                .flex_1()
                                .min_w_0(),
                        )
                        .child(
                            Label::new(relative_recency)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_nowrap()
                                .flex_shrink_0(),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::Folder).size_4().flex_none())
                        .child(
                            Label::new(project_display_name.clone())
                                .text_sm()
                                .whitespace_normal()
                                .flex_1()
                                .min_w_0(),
                        ),
                )
        })
        .into_any_element()
}

fn conversation_status_slot(
    status: ConversationSidebarStatus,
    group: String,
    conversation_id: &ConversationId,
    cx: &mut App,
) -> Option<AnyElement> {
    let (tooltip, content) = match status {
        ConversationSidebarStatus::Idle => return None,
        ConversationSidebarStatus::Running => (
            cx.global::<crate::foundation::I18n>()
                .t("sidebar-conversation-status-running"),
            Spinner::new().small().into_any_element(),
        ),
        ConversationSidebarStatus::AwaitingApproval => (
            cx.global::<crate::foundation::I18n>()
                .t("sidebar-conversation-status-awaiting-approval"),
            Icon::new(IconName::ShieldAlert)
                .size_4()
                .text_color(cx.theme().warning)
                .into_any_element(),
        ),
        ConversationSidebarStatus::Failed => (
            cx.global::<crate::foundation::I18n>()
                .t("sidebar-conversation-status-failed"),
            Icon::new(IconName::CircleAlert)
                .size_4()
                .text_color(cx.theme().danger)
                .into_any_element(),
        ),
    };

    Some(
        h_flex()
            .id(format!("sidebar-conversation-status-{conversation_id}"))
            .absolute()
            .top_0()
            .right_2()
            .bottom_0()
            .w(ACTION_SUFFIX_WIDTH)
            .items_center()
            .justify_end()
            .group_hover(group, |this| this.opacity(0.))
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .child(content)
            .into_any_element(),
    )
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
    runtime: Entity<ConversationRuntimeStore>,
    _cx: &mut App,
) -> AnyElement {
    ConversationSidebarRow::new(conversation, active, workspace, runtime).into_any_element()
}

pub(super) fn route_matches_conversation(
    route: &HomeRoute,
    conversation_id: &ConversationId,
) -> bool {
    matches!(route, HomeRoute::Conversation(active_id) if active_id == conversation_id)
}

#[cfg(test)]
mod tests {
    use super::{
        CONVERSATION_DIRECT_ACTIONS, ConversationSidebarAction, conversation_hover_card_placement,
    };
    use gpui::{
        Anchor, Bounds, Context, Modifiers, Pixels, Render, TestAppContext, VisualTestContext,
        Window, div, point, prelude::*, px, size,
    };
    use gpui_component::hover_card::HoverCard;
    use std::time::Duration;

    struct HoverCardAnchorTestView;

    impl Render for HoverCardAnchorTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                div()
                    .absolute()
                    .left(px(36.))
                    .top(px(100.))
                    .w(px(252.))
                    .child(
                        HoverCard::new("hover-card-anchor-test")
                            .anchor(Anchor::TopLeft)
                            .appearance(false)
                            .open_delay(Duration::ZERO)
                            .close_delay(Duration::from_millis(300))
                            .left(px(268.))
                            .top(px(-28.))
                            .trigger(
                                div()
                                    .debug_selector(|| "HOVER_CARD_TRIGGER".into())
                                    .w_full()
                                    .h(px(28.)),
                            )
                            .content(|_, _, _| {
                                div()
                                    .debug_selector(|| "HOVER_CARD_CONTENT".into())
                                    .w(px(320.))
                                    .h(px(100.))
                            }),
                    ),
            )
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    fn debug_bounds(cx: &mut VisualTestContext, selector: &'static str) -> Option<Bounds<Pixels>> {
        cx.debug_bounds(selector)
    }

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

    #[test]
    fn conversation_hover_card_uses_sidebar_edge_and_trigger_top() {
        let trigger_bounds = Bounds {
            origin: point(px(36.), px(100.)),
            size: size(px(252.), px(28.)),
        };
        let placement = conversation_hover_card_placement(px(300.), trigger_bounds);

        assert_eq!(placement.anchor, Anchor::TopLeft);
        assert_eq!(placement.popover_offset, point(px(268.), px(-28.)));
    }

    #[gpui::test]
    fn hover_card_moves_its_root_and_preserves_content_hover(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (_, cx) = cx.add_window_view(|_, _| HoverCardAnchorTestView);
        draw(cx);

        cx.simulate_mouse_move(point(px(50.), px(110.)), None, Modifiers::default());
        draw(cx);

        let content_bounds = debug_bounds(cx, "HOVER_CARD_CONTENT").expect("hover card opens");
        assert_eq!(content_bounds.origin, point(px(304.), px(100.)));

        cx.simulate_mouse_move(point(px(310.), px(110.)), None, Modifiers::default());
        cx.background_executor
            .advance_clock(Duration::from_millis(301));
        draw(cx);
        assert!(debug_bounds(cx, "HOVER_CARD_CONTENT").is_some());
    }
}
