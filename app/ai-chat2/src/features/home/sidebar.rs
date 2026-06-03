pub(crate) mod search;

mod menu;
mod row;

use crate::{
    features::settings::{TOGGLE_SETTINGS_KEY, ToggleSettings},
    foundation::{self, assets::IconName},
    state::{self, workspace::SidebarPinnedEntry},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Collapsible, Side,
    label::Label,
    sidebar::{Sidebar, SidebarGroup, SidebarItem},
    v_flex,
};

use super::actions::{
    OPEN_CONVERSATION_SEARCH_KEY, OPEN_NEW_CONVERSATION_KEY, OpenConversationSearch,
    OpenNewConversation,
};

pub(crate) struct HomeSidebar;

impl HomeSidebar {
    pub(crate) fn new(_: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for HomeSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_label = sidebar_settings_label(cx.global::<foundation::I18n>());
        let workspace = state::workspace::workspace(cx);
        let route = workspace.read(cx).route().clone();
        let snapshot = workspace.read(cx).snapshot().clone();
        let last_error = workspace.read(cx).last_error().map(ToOwned::to_owned);

        let sections = sidebar_sections(snapshot, route, workspace, last_error, cx);

        Sidebar::<SidebarSection>::new("ai-chat2-main-sidebar")
            .side(Side::Left)
            .w_full()
            .border_r_0()
            .collapsible(false)
            .collapsed(false)
            .children(sections)
            .footer(settings_action(settings_label).render(cx))
    }
}

#[derive(Clone)]
enum SidebarSection {
    Actions(SidebarActions),
    Rows(SidebarGroup<SidebarRows>),
    Message(SidebarMessage),
}

impl Collapsible for SidebarSection {
    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Actions(menu) => Self::Actions(menu.collapsed(collapsed)),
            Self::Rows(group) => Self::Rows(group.collapsed(collapsed)),
            Self::Message(message) => Self::Message(message.collapsed(collapsed)),
        }
    }

    fn is_collapsed(&self) -> bool {
        match self {
            Self::Actions(menu) => menu.is_collapsed(),
            Self::Rows(group) => group.is_collapsed(),
            Self::Message(message) => message.is_collapsed(),
        }
    }
}

impl SidebarItem for SidebarSection {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Actions(menu) => menu.render(id, window, cx).into_any_element(),
            Self::Rows(group) => group.render(id, window, cx).into_any_element(),
            Self::Message(message) => message.render(id, window, cx).into_any_element(),
        }
    }
}

#[derive(Clone)]
struct SidebarActions {
    rows: Vec<row::ShortcutSidebarAction>,
    collapsed: bool,
}

impl SidebarActions {
    fn new(rows: Vec<row::ShortcutSidebarAction>) -> Self {
        Self {
            rows,
            collapsed: false,
        }
    }
}

impl Collapsible for SidebarActions {
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl SidebarItem for SidebarActions {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        v_flex().id(id).gap_2().when(!self.collapsed, |this| {
            this.children(
                self.rows
                    .into_iter()
                    .map(|row| row.render(cx))
                    .collect::<Vec<_>>(),
            )
        })
    }
}

#[derive(Clone)]
struct SidebarRows {
    rows: Vec<SidebarRow>,
    collapsed: bool,
}

impl SidebarRows {
    fn new(rows: Vec<SidebarRow>) -> Self {
        Self {
            rows,
            collapsed: false,
        }
    }
}

impl Collapsible for SidebarRows {
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl SidebarItem for SidebarRows {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        v_flex().id(id).gap_1().when(!self.collapsed, |this| {
            this.children(
                self.rows
                    .into_iter()
                    .map(|row| row.render(cx))
                    .collect::<Vec<_>>(),
            )
        })
    }
}

#[derive(Clone)]
enum SidebarRow {
    Project {
        node: state::workspace::SidebarProjectNode,
        route: state::HomeRoute,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    },
    Conversation {
        conversation: state::workspace::SidebarConversationNode,
        active: bool,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    },
    Empty(SharedString),
}

impl SidebarRow {
    fn project(
        node: state::workspace::SidebarProjectNode,
        route: state::HomeRoute,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    ) -> Self {
        Self::Project {
            node,
            route,
            workspace,
        }
    }

    fn conversation(
        conversation: state::workspace::SidebarConversationNode,
        active: bool,
        workspace: Entity<state::AiChat2WorkspaceStore>,
    ) -> Self {
        Self::Conversation {
            conversation,
            active,
            workspace,
        }
    }

    fn render(self, cx: &mut App) -> AnyElement {
        match self {
            Self::Project {
                node,
                route,
                workspace,
            } => project_tree_row(node, route, workspace, cx),
            Self::Conversation {
                conversation,
                active,
                workspace,
            } => row::conversation_row(conversation, active, workspace, cx),
            Self::Empty(label) => empty_row(label, cx),
        }
    }
}

#[derive(Clone)]
struct SidebarMessage {
    message: SharedString,
    collapsed: bool,
}

impl SidebarMessage {
    fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            collapsed: false,
        }
    }
}

impl Collapsible for SidebarMessage {
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl SidebarItem for SidebarMessage {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        div().id(id).w_full().when(!self.collapsed, |this| {
            this.child(
                Label::new(self.message)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .px_2()
                    .py_1(),
            )
        })
    }
}

fn sidebar_sections(
    snapshot: state::workspace::SidebarSnapshot,
    route: state::HomeRoute,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    last_error: Option<String>,
    cx: &mut App,
) -> Vec<SidebarSection> {
    let mut sections = vec![SidebarSection::Actions(top_actions(cx))];

    if let Some(error) = last_error {
        sections.push(SidebarSection::Message(SidebarMessage::new(error)));
    }

    sections.extend(render_pinned_section(
        snapshot.pinned,
        route.clone(),
        workspace.clone(),
        cx,
    ));
    sections.extend(render_projects_section(
        snapshot.projects,
        route.clone(),
        workspace.clone(),
        cx,
    ));
    sections.extend(render_no_project_section(
        snapshot.no_project_conversations,
        route,
        workspace,
        cx,
    ));

    sections
}

fn top_actions(cx: &mut App) -> SidebarActions {
    let i18n = cx.global::<foundation::I18n>();

    SidebarActions::new(vec![
        row::ShortcutSidebarAction::new(
            "sidebar-action-new-conversation",
            i18n.t("sidebar-new-conversation"),
            IconName::SquarePen,
            OPEN_NEW_CONVERSATION_KEY,
            |_, window, cx| {
                window.dispatch_action(OpenNewConversation.boxed_clone(), cx);
            },
        ),
        row::ShortcutSidebarAction::new(
            "sidebar-action-search",
            i18n.t("sidebar-search"),
            IconName::Search,
            OPEN_CONVERSATION_SEARCH_KEY,
            |_, window, cx| {
                window.dispatch_action(OpenConversationSearch.boxed_clone(), cx);
            },
        ),
    ])
}

fn settings_action(label: impl Into<SharedString>) -> row::ShortcutSidebarAction {
    row::ShortcutSidebarAction::new(
        "sidebar-action-settings",
        label,
        IconName::Settings,
        TOGGLE_SETTINGS_KEY,
        |_, window, cx| {
            window.dispatch_action(ToggleSettings.boxed_clone(), cx);
        },
    )
}

fn render_pinned_section(
    pinned: Vec<SidebarPinnedEntry>,
    route: state::HomeRoute,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    cx: &mut App,
) -> Vec<SidebarSection> {
    if pinned.is_empty() {
        return Vec::new();
    }

    let rows = pinned.into_iter().map(|entry| match entry {
        SidebarPinnedEntry::Conversation(conversation) => {
            let active = row::route_matches_conversation(&route, &conversation.id);
            SidebarRow::conversation(conversation, active, workspace.clone())
        }
        SidebarPinnedEntry::Project(project) => SidebarRow::project(
            state::workspace::SidebarProjectNode {
                project,
                is_expanded: false,
                conversations: Vec::new(),
            },
            route.clone(),
            workspace.clone(),
        ),
    });

    let label = cx.global::<foundation::I18n>().t("sidebar-section-pinned");
    vec![SidebarSection::Rows(
        SidebarGroup::new(label).child(SidebarRows::new(rows.collect())),
    )]
}

fn render_projects_section(
    projects: Vec<state::workspace::SidebarProjectNode>,
    route: state::HomeRoute,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    cx: &mut App,
) -> Vec<SidebarSection> {
    let mut rows = Vec::new();

    if projects.is_empty() {
        rows.push(SidebarRow::Empty(
            cx.global::<foundation::I18n>()
                .t("sidebar-empty-projects")
                .into(),
        ));
        let label = cx
            .global::<foundation::I18n>()
            .t("sidebar-section-projects");
        return vec![SidebarSection::Rows(
            SidebarGroup::new(label).child(SidebarRows::new(rows)),
        )];
    }

    rows.extend(
        projects
            .into_iter()
            .map(|project| SidebarRow::project(project, route.clone(), workspace.clone())),
    );

    let label = cx
        .global::<foundation::I18n>()
        .t("sidebar-section-projects");
    vec![SidebarSection::Rows(
        SidebarGroup::new(label).child(SidebarRows::new(rows)),
    )]
}

fn render_no_project_section(
    conversations: Vec<state::workspace::SidebarConversationNode>,
    route: state::HomeRoute,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    cx: &mut App,
) -> Vec<SidebarSection> {
    if conversations.is_empty() {
        return Vec::new();
    }

    let rows = conversations.into_iter().map(|conversation| {
        let active = row::route_matches_conversation(&route, &conversation.id);
        SidebarRow::conversation(conversation, active, workspace.clone())
    });

    let label = cx
        .global::<foundation::I18n>()
        .t("sidebar-section-no-project-conversations");
    vec![SidebarSection::Rows(
        SidebarGroup::new(label).child(SidebarRows::new(rows.collect())),
    )]
}

fn project_tree_row(
    node: state::workspace::SidebarProjectNode,
    route: state::HomeRoute,
    workspace: Entity<state::AiChat2WorkspaceStore>,
    cx: &mut App,
) -> AnyElement {
    let project_id = node.project.id.clone();
    let is_expanded = node.is_expanded;
    let conversations = node.conversations.clone();

    v_flex()
        .w_full()
        .child(row::project_row(node, workspace.clone(), cx))
        .when(is_expanded, |this| {
            let children = if conversations.is_empty() {
                vec![empty_row(
                    cx.global::<foundation::I18n>()
                        .t("sidebar-empty-conversations"),
                    cx,
                )]
            } else {
                conversations
                    .into_iter()
                    .map(|conversation| {
                        let active = row::route_matches_conversation(&route, &conversation.id);
                        row::conversation_row(conversation, active, workspace.clone(), cx)
                    })
                    .collect::<Vec<_>>()
            };

            this.child(
                v_flex()
                    .id(format!("sidebar-project-submenu-{project_id}"))
                    .border_l_1()
                    .border_color(cx.theme().sidebar_border)
                    .gap_1()
                    .ml_3p5()
                    .pl_2p5()
                    .py_0p5()
                    .children(children),
            )
        })
        .into_any_element()
}

fn empty_row(label: impl Into<SharedString>, cx: &mut App) -> AnyElement {
    div()
        .w_full()
        .h_7()
        .p_2()
        .child(
            Label::new(label.into())
                .text_sm()
                .truncate()
                .text_color(cx.theme().sidebar_foreground.opacity(0.7)),
        )
        .into_any_element()
}

fn sidebar_settings_label(i18n: &foundation::I18n) -> String {
    i18n.t("app-menu-settings")
}

#[cfg(test)]
mod tests {
    use super::sidebar_settings_label;
    use crate::foundation::I18n;

    #[test]
    fn sidebar_settings_label_uses_existing_i18n_key() {
        assert_eq!(
            sidebar_settings_label(&I18n::english_for_test()),
            "Settings"
        );
        assert_eq!(
            sidebar_settings_label(&I18n::for_locale_tag("zh-CN")),
            "设置"
        );
    }
}
