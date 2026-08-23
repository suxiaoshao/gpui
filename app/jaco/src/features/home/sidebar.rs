pub(crate) mod search;

mod actions;
mod menu;
mod row;

use crate::{
    features::settings::{TOGGLE_SETTINGS_KEY, ToggleSettings},
    foundation::{self, assets::IconName},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Collapsible, Disableable, Side, Sizable,
    button::Button,
    h_flex,
    label::Label,
    sidebar::{Sidebar, SidebarGroup, SidebarItem},
    v_flex,
};

use super::actions::{
    OPEN_CONVERSATION_SEARCH_KEY, OPEN_NEW_CONVERSATION_KEY, OpenConversationSearch,
    OpenNewConversation,
};
use super::workspace::{
    HomeRoute, HomeWorkspace, SidebarConversationNode, SidebarPinnedEntry, SidebarProjectNode,
    SidebarSnapshot,
};

pub(crate) struct HomeSidebar {
    workspace: Entity<HomeWorkspace>,
}

impl HomeSidebar {
    pub(crate) fn new(workspace: Entity<HomeWorkspace>, _: &mut Context<Self>) -> Self {
        Self { workspace }
    }
}

impl Render for HomeSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_label = sidebar_settings_label(cx.global::<foundation::I18n>());
        let workspace = self.workspace.clone();
        let route = workspace.read(cx).route().clone();
        let snapshot = workspace.read(cx).snapshot().clone();
        let project_status = workspace.read(cx).project_status();
        let conversation_status = workspace.read(cx).conversation_status(cx);
        let sections = sidebar_sections(
            snapshot,
            route,
            workspace,
            project_status,
            conversation_status,
            cx,
        );

        Sidebar::<SidebarSection>::new("jaco-main-sidebar")
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
    Status(ResourceStatusRow),
    Rows(SidebarGroup<SidebarRows>),
}

impl Collapsible for SidebarSection {
    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Actions(menu) => Self::Actions(menu.collapsed(collapsed)),
            Self::Status(status) => Self::Status(status),
            Self::Rows(group) => Self::Rows(group.collapsed(collapsed)),
        }
    }

    fn is_collapsed(&self) -> bool {
        match self {
            Self::Actions(menu) => menu.is_collapsed(),
            Self::Status(_) => false,
            Self::Rows(group) => group.is_collapsed(),
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
            Self::Status(status) => status.render(id, window, cx).into_any_element(),
            Self::Rows(group) => group.render(id, window, cx).into_any_element(),
        }
    }
}

#[derive(Clone, Copy)]
enum ResourceKind {
    Projects,
    Conversations,
}

#[derive(Clone)]
struct ResourceStatusRow {
    kind: ResourceKind,
    status: super::workspace::WorkspaceResourceStatus,
    workspace: Entity<HomeWorkspace>,
}

impl Collapsible for ResourceStatusRow {
    fn collapsed(self, _collapsed: bool) -> Self {
        self
    }

    fn is_collapsed(&self) -> bool {
        false
    }
}

impl SidebarItem for ResourceStatusRow {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let workspace = self.workspace.clone();
        let running = self.status.running;
        let subject = cx.global::<foundation::I18n>().t(match self.kind {
            ResourceKind::Projects => "sidebar-resource-projects",
            ResourceKind::Conversations => "sidebar-resource-conversations",
        });
        let detail = self.status.problem.unwrap_or_else(|| {
            cx.global::<foundation::I18n>().t(if self.status.has_data {
                "resource-status-stale"
            } else {
                "resource-status-loading"
            })
        });
        h_flex()
            .id(id)
            .w_full()
            .min_w_0()
            .gap_2()
            .rounded(cx.theme().radius)
            .bg(cx.theme().warning.opacity(0.08))
            .p_2()
            .child(
                Label::new(format!("{subject}: {detail}"))
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .flex_1(),
            )
            .child(
                Button::new(match self.kind {
                    ResourceKind::Projects => "sidebar-refresh-projects",
                    ResourceKind::Conversations => "sidebar-refresh-conversations",
                })
                .label(cx.global::<foundation::I18n>().t("resource-status-refresh"))
                .xsmall()
                .disabled(running)
                .on_click(move |_, _window, cx| {
                    workspace.update(cx, |workspace, cx| match self.kind {
                        ResourceKind::Projects => workspace.refresh_projects(cx),
                        ResourceKind::Conversations => workspace.refresh_conversations(cx),
                    });
                }),
            )
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
        node: SidebarProjectNode,
        route: HomeRoute,
        workspace: Entity<HomeWorkspace>,
    },
    Conversation {
        conversation: SidebarConversationNode,
        active: bool,
        workspace: Entity<HomeWorkspace>,
    },
    Empty(SharedString),
}

impl SidebarRow {
    fn project(
        node: SidebarProjectNode,
        route: HomeRoute,
        workspace: Entity<HomeWorkspace>,
    ) -> Self {
        Self::Project {
            node,
            route,
            workspace,
        }
    }

    fn conversation(
        conversation: SidebarConversationNode,
        active: bool,
        workspace: Entity<HomeWorkspace>,
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

fn sidebar_sections(
    snapshot: SidebarSnapshot,
    route: HomeRoute,
    workspace: Entity<HomeWorkspace>,
    project_status: super::workspace::WorkspaceResourceStatus,
    conversation_status: super::workspace::WorkspaceResourceStatus,
    cx: &mut App,
) -> Vec<SidebarSection> {
    let can_create_conversation = project_status.is_ready() && conversation_status.is_ready();
    let projects_are_ready = project_status.is_ready();
    let mut sections = vec![SidebarSection::Actions(top_actions(
        can_create_conversation,
        cx,
    ))];
    if !project_status.is_ready() {
        sections.push(SidebarSection::Status(ResourceStatusRow {
            kind: ResourceKind::Projects,
            status: project_status,
            workspace: workspace.clone(),
        }));
    }
    if !conversation_status.is_ready() {
        sections.push(SidebarSection::Status(ResourceStatusRow {
            kind: ResourceKind::Conversations,
            status: conversation_status,
            workspace: workspace.clone(),
        }));
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
        projects_are_ready,
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

fn top_actions(can_create_conversation: bool, cx: &mut App) -> SidebarActions {
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
        )
        .disabled(!can_create_conversation),
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
    route: HomeRoute,
    workspace: Entity<HomeWorkspace>,
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
            SidebarProjectNode {
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
    projects: Vec<SidebarProjectNode>,
    route: HomeRoute,
    workspace: Entity<HomeWorkspace>,
    ready: bool,
    cx: &mut App,
) -> Vec<SidebarSection> {
    let mut rows = Vec::new();

    if projects.is_empty() {
        if !ready {
            return Vec::new();
        }
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
    conversations: Vec<SidebarConversationNode>,
    route: HomeRoute,
    workspace: Entity<HomeWorkspace>,
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
    node: SidebarProjectNode,
    route: HomeRoute,
    workspace: Entity<HomeWorkspace>,
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
