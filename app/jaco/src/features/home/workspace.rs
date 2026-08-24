use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gpui::{App, AppContext, Context, Entity, SharedString, Subscription, Task};
use gpui_operation::refresh::Phase;
use gpui_store::{Select, StoreSelection};
use jaco_core::{ConversationId, ConversationStatus, ConversationSummary, ProjectId, ProjectKind};
use jaco_db::ProjectRecord;
use time::OffsetDateTime;

use crate::{
    database, features::conversation::registry::ConversationCatalogModel, state::projects,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceProjectInput {
    id: ProjectId,
    kind: ProjectKind,
    path: PathBuf,
    display_name: String,
    pinned: bool,
    updated_at: i128,
}

#[derive(Clone, Copy, Default)]
struct SelectWorkspaceProjects;

impl Select<projects::ProjectOperation> for SelectWorkspaceProjects {
    type Output = WorkspaceProjectsSnapshot;

    fn select(&self, operation: &projects::ProjectOperation) -> Self::Output {
        WorkspaceProjectsSnapshot {
            projects: operation.data().map(|data| {
                data.projects()
                    .iter()
                    .filter(|project| !project.removed)
                    .map(|project| WorkspaceProjectInput {
                        id: project.id.clone(),
                        kind: project.kind,
                        path: PathBuf::from(&project.path),
                        display_name: project.display_name.clone(),
                        pinned: project.pinned,
                        updated_at: project.updated_at.unix_timestamp_nanos(),
                    })
                    .collect()
            }),
            status: WorkspaceResourceStatus::from_operation(operation),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceProjectsSnapshot {
    projects: Option<Vec<WorkspaceProjectInput>>,
    status: WorkspaceResourceStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceResourceStatus {
    pub(crate) phase: Phase,
    pub(crate) problem: Option<String>,
    pub(crate) has_data: bool,
    pub(crate) running: bool,
}

impl WorkspaceResourceStatus {
    fn from_operation<Data, Problem: std::error::Error, Task>(
        operation: &gpui_operation::refresh::Operation<Data, Problem, Task>,
    ) -> Self {
        Self {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
            has_data: operation.data().is_some(),
            running: operation.is_running(),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.phase == Phase::Ready
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceConversationInput {
    id: ConversationId,
    project_id: ProjectId,
    title: String,
    pinned: bool,
    status: ConversationStatus,
    recency_at: OffsetDateTime,
    deleted_at: Option<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HomeRoute {
    NewConversation,
    Conversation(ConversationId),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SidebarSnapshot {
    pub(crate) pinned: Vec<SidebarPinnedEntry>,
    pub(crate) projects: Vec<SidebarProjectNode>,
    pub(crate) no_project_conversations: Vec<SidebarConversationNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SidebarPinnedEntry {
    Conversation(SidebarConversationNode),
    Project(SidebarProjectHeader),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarProjectNode {
    pub(crate) project: SidebarProjectHeader,
    pub(crate) is_expanded: bool,
    pub(crate) conversations: Vec<SidebarConversationNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarProjectHeader {
    pub(crate) id: ProjectId,
    pub(crate) path: PathBuf,
    pub(crate) display_name: SharedString,
    pub(crate) updated_at: i128,
    pub(crate) pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarConversationNode {
    pub(crate) id: ConversationId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: SharedString,
    pub(crate) recency_at: OffsetDateTime,
    pub(crate) project_display_name: Option<SharedString>,
    pub(crate) pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarSearchResult {
    pub(crate) conversation: SidebarConversationNode,
    pub(crate) project: Option<SidebarProjectHeader>,
}

pub(crate) struct SidebarSearchLoad {
    pub(crate) results: Vec<SidebarSearchResult>,
    pub(crate) stale_problem: Option<String>,
}

pub(crate) struct HomeWorkspace {
    route: HomeRoute,
    snapshot: SidebarSnapshot,
    expanded_project_ids: HashSet<ProjectId>,
    pending_new_conversation_project_id: Option<ProjectId>,
    projects: StoreSelection<WorkspaceProjectsSnapshot>,
    conversations: Entity<ConversationCatalogModel>,
    _subscriptions: Vec<Subscription>,
}

impl HomeWorkspace {
    fn new(
        project_catalog: projects::ProjectStore,
        conversation_catalog: Entity<ConversationCatalogModel>,
        cx: &mut Context<Self>,
    ) -> Self {
        let projects = project_catalog.select(cx, SelectWorkspaceProjects);
        let mut store = Self {
            route: HomeRoute::NewConversation,
            snapshot: SidebarSnapshot::default(),
            expanded_project_ids: HashSet::new(),
            pending_new_conversation_project_id: None,
            projects,
            conversations: conversation_catalog.clone(),
            _subscriptions: Vec::new(),
        };
        store._subscriptions.push(project_catalog.observe_select(
            cx,
            SelectWorkspaceProjects,
            |store, _, cx| store.rebuild_sidebar(cx),
        ));
        store
            ._subscriptions
            .push(cx.observe(&conversation_catalog, |store, _, cx| {
                store.rebuild_sidebar(cx)
            }));
        store.rebuild_sidebar(cx);
        store
    }

    pub(crate) fn route(&self) -> &HomeRoute {
        &self.route
    }

    pub(crate) fn snapshot(&self) -> &SidebarSnapshot {
        &self.snapshot
    }

    pub(crate) fn project_status(&self) -> WorkspaceResourceStatus {
        self.projects.read(|projects| projects.status.clone())
    }

    pub(crate) fn conversation_status(&self, cx: &App) -> WorkspaceResourceStatus {
        WorkspaceResourceStatus::from_operation(self.conversations.read(cx).operation())
    }

    pub(crate) fn project_mutations_ready(&self) -> bool {
        self.project_status().is_ready()
    }

    pub(crate) fn conversation_mutations_ready(&self, cx: &App) -> bool {
        self.conversation_status(cx).is_ready()
    }

    pub(crate) fn contains_project(&self, project_id: &ProjectId) -> bool {
        self.projects.read(|projects| {
            projects
                .projects
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|project| &project.id == project_id)
        })
    }

    pub(crate) fn contains_conversation(&self, conversation_id: &ConversationId, cx: &App) -> bool {
        self.conversations
            .read(cx)
            .operation()
            .data()
            .is_some_and(|conversations| {
                conversations.iter().any(|conversation| {
                    &conversation.id == conversation_id
                        && conversation.status == ConversationStatus::Active
                })
            })
    }

    pub(crate) fn project_path(&self, project_id: &ProjectId) -> Option<PathBuf> {
        self.projects.read(|projects| {
            projects
                .projects
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|project| &project.id == project_id)
                .map(|project| project.path.clone())
        })
    }

    pub(crate) fn active_conversation_count(&self, project_id: &ProjectId, cx: &App) -> usize {
        self.conversations
            .read(cx)
            .operation()
            .data()
            .map(|conversations| {
                conversations
                    .iter()
                    .filter(|conversation| {
                        &conversation.project_id == project_id
                            && conversation.status == ConversationStatus::Active
                    })
                    .count()
            })
            .unwrap_or_default()
    }

    pub(crate) fn refresh_projects(&self, cx: &mut App) {
        projects::request_refresh(cx);
    }

    pub(crate) fn refresh_conversations(&self, cx: &mut App) {
        self.conversations
            .update(cx, |catalog, cx| catalog.refresh(cx));
    }

    fn rebuild_sidebar(&mut self, cx: &mut Context<Self>) {
        let conversations = self
            .conversations
            .read(cx)
            .operation()
            .data()
            .map(|conversations| {
                conversations
                    .iter()
                    .map(workspace_conversation_input)
                    .collect::<Vec<_>>()
            });
        self.snapshot = self.projects.read(|projects| {
            build_sidebar_snapshot(
                &self.expanded_project_ids,
                projects.projects.as_deref().unwrap_or_default(),
                conversations.as_deref().unwrap_or_default(),
            )
        });
        cx.notify();
    }

    pub(crate) fn open_new_conversation(&mut self, cx: &mut Context<Self>) {
        self.pending_new_conversation_project_id = None;
        self.route = HomeRoute::NewConversation;
        cx.notify();
    }

    pub(crate) fn new_conversation_in_project(
        &mut self,
        project_id: &ProjectId,
        cx: &mut Context<Self>,
    ) {
        self.pending_new_conversation_project_id = Some(project_id.clone());
        self.route = HomeRoute::NewConversation;
        cx.notify();
    }

    pub(crate) fn take_pending_new_conversation_project_id(&mut self) -> Option<ProjectId> {
        self.pending_new_conversation_project_id.take()
    }

    pub(crate) fn open_conversation(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        self.route = HomeRoute::Conversation(conversation_id);
        cx.notify();
    }

    pub(crate) fn toggle_project(&mut self, project_id: &ProjectId, cx: &mut Context<Self>) {
        if !self.expanded_project_ids.insert(project_id.clone()) {
            self.expanded_project_ids.remove(project_id);
        }
        self.rebuild_sidebar(cx);
    }

    pub(crate) fn pin_project(
        &mut self,
        project_id: ProjectId,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ProjectRecord>> {
        projects::set_project_pinned(project_id, pinned, cx)
    }

    pub(crate) fn rename_project(
        &mut self,
        project_id: ProjectId,
        display_name: String,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ProjectRecord>> {
        projects::rename_project(project_id, display_name, cx)
    }

    pub(crate) fn remove_project(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ProjectRecord>> {
        if self.route_belongs_to_project(&project_id, cx) {
            self.route = HomeRoute::NewConversation;
        }
        projects::set_project_removed(project_id, true, cx)
    }

    pub(crate) fn pin_conversation(
        &mut self,
        conversation_id: ConversationId,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ConversationSummary>> {
        crate::features::conversation::set_conversation_pinned(conversation_id, pinned, cx)
    }

    pub(crate) fn rename_conversation(
        &mut self,
        conversation_id: ConversationId,
        title: String,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ConversationSummary>> {
        crate::features::conversation::rename_conversation(conversation_id, title, cx)
    }

    pub(crate) fn archive_conversation(
        &mut self,
        conversation_id: ConversationId,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ConversationSummary>> {
        let task = crate::features::conversation::archive_conversation(
            conversation_id.clone(),
            project_id,
            cx,
        );
        cx.spawn(async move |workspace, cx| {
            let result = task.await;
            if result.is_ok() {
                let _ = workspace.update(cx, |workspace, cx| {
                    if reset_route_after_archived(
                        &mut workspace.route,
                        std::iter::once(&conversation_id),
                    ) {
                        cx.notify();
                    }
                });
            }
            result
        })
    }

    pub(crate) fn archive_project_conversations(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<Vec<ConversationSummary>>> {
        let task = crate::features::conversation::archive_project_conversations(project_id, cx);
        cx.spawn(async move |workspace, cx| {
            task.await.inspect(|summaries| {
                let _ = workspace.update(cx, |workspace, cx| {
                    if reset_route_after_archived(
                        &mut workspace.route,
                        summaries.iter().map(|summary| &summary.id),
                    ) {
                        cx.notify();
                    }
                });
            })
        })
    }

    pub(crate) fn search_conversations(
        &self,
        query: String,
        limit: usize,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<SidebarSearchLoad>> {
        let project_by_id = self.visible_project_headers();
        if query.is_empty() {
            let status = self.conversation_status(cx);
            if !status.has_data {
                return Task::ready(Err(jaco_db::DbError::Invariant(
                    status
                        .problem
                        .unwrap_or_else(|| "conversation catalog is loading".to_string()),
                )));
            }
            let results =
                self.snapshot
                    .projects
                    .iter()
                    .flat_map(|project| {
                        project.conversations.iter().cloned().map(|conversation| {
                            SidebarSearchResult {
                                project: Some(project.project.clone()),
                                conversation,
                            }
                        })
                    })
                    .chain(self.snapshot.no_project_conversations.iter().cloned().map(
                        |conversation| SidebarSearchResult {
                            project: None,
                            conversation,
                        },
                    ))
                    .take(limit)
                    .collect();
            return Task::ready(Ok(SidebarSearchLoad {
                results,
                stale_problem: (!status.is_ready()).then(|| {
                    status
                        .problem
                        .unwrap_or_else(|| "conversation catalog is stale".to_string())
                }),
            }));
        }
        let executor = match database::ready_executor(cx) {
            Ok(executor) => executor,
            Err(error) => return Task::ready(Err(error)),
        };
        cx.spawn(async move |_, _| {
            executor
                .execute(move |repo| {
                    jaco_conversation::ConversationService::new(repo)
                        .search_catalog(&query, limit)
                        .map_err(|error| match error {
                            jaco_conversation::ConversationError::Database(error) => error,
                        })
                })
                .await
                .map(|conversations| SidebarSearchLoad {
                    results: conversations
                        .into_iter()
                        .map(|conversation| {
                            let project = project_by_id.get(&conversation.project_id);
                            SidebarSearchResult {
                                project: project.map(|(header, _)| header.clone()),
                                conversation: SidebarConversationNode {
                                    id: conversation.id,
                                    project_id: conversation.project_id,
                                    title: conversation.title.into(),
                                    recency_at: conversation.recency_at,
                                    project_display_name: project
                                        .filter(|(_, kind)| *kind == ProjectKind::Normal)
                                        .map(|(header, _)| header.display_name.clone()),
                                    pinned: conversation.pinned,
                                },
                            }
                        })
                        .collect(),
                    stale_problem: None,
                })
        })
    }

    fn route_belongs_to_project(&self, project_id: &ProjectId, _cx: &App) -> bool {
        let HomeRoute::Conversation(conversation_id) = &self.route else {
            return false;
        };

        self.conversations
            .read(_cx)
            .operation()
            .data()
            .into_iter()
            .flatten()
            .find(|conversation| &conversation.id == conversation_id)
            .is_some_and(|conversation| &conversation.project_id == project_id)
    }

    fn visible_project_headers(&self) -> HashMap<ProjectId, (SidebarProjectHeader, ProjectKind)> {
        self.projects.read(|projects| {
            projects
                .projects
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|project| (project.id.clone(), (project_header(project), project.kind)))
                .collect()
        })
    }
}

fn reset_route_after_archived<'a>(
    route: &mut HomeRoute,
    archived_ids: impl IntoIterator<Item = &'a ConversationId>,
) -> bool {
    let HomeRoute::Conversation(current_id) = route else {
        return false;
    };
    let should_reset = archived_ids
        .into_iter()
        .any(|archived_id| archived_id == current_id);
    if should_reset {
        *route = HomeRoute::NewConversation;
    }
    should_reset
}

pub(crate) fn create(cx: &mut App) -> Entity<HomeWorkspace> {
    let project_catalog = projects::catalog(cx);
    let conversation_catalog = crate::features::conversation::resources::ready_conversations(cx)
        .expect("home workspace requires ready conversation resources")
        .read(cx)
        .catalog();
    cx.new(|cx| HomeWorkspace::new(project_catalog, conversation_catalog, cx))
}

fn workspace_conversation_input(conversation: &ConversationSummary) -> WorkspaceConversationInput {
    WorkspaceConversationInput {
        id: conversation.id.clone(),
        project_id: conversation.project_id.clone(),
        title: conversation.title.clone(),
        pinned: conversation.pinned,
        status: conversation.status,
        recency_at: conversation.recency_at,
        deleted_at: conversation
            .deleted_at
            .map(|deleted_at| deleted_at.unix_timestamp_nanos()),
    }
}

fn build_sidebar_snapshot(
    expanded_project_ids: &HashSet<ProjectId>,
    visible_projects: &[WorkspaceProjectInput],
    sidebar_conversations: &[WorkspaceConversationInput],
) -> SidebarSnapshot {
    let mut normal_projects = visible_projects
        .iter()
        .filter(|project| project.kind == ProjectKind::Normal)
        .cloned()
        .collect::<Vec<_>>();
    normal_projects.sort_by(|left, right| {
        left.display_name
            .to_lowercase()
            .cmp(&right.display_name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });

    let normal_project_names = normal_projects
        .iter()
        .map(|project| {
            (
                project.id.clone(),
                SharedString::from(project.display_name.clone()),
            )
        })
        .collect::<HashMap<_, _>>();
    let scratch_project_ids = visible_projects
        .iter()
        .filter(|project| project.kind == ProjectKind::Scratch)
        .map(|project| project.id.clone())
        .collect::<HashSet<_>>();
    let mut conversations_by_project: HashMap<ProjectId, Vec<SidebarConversationNode>> =
        HashMap::new();
    let mut no_project_conversations = Vec::new();

    for conversation in sidebar_conversations
        .iter()
        .filter(|conversation| conversation.status == ConversationStatus::Active)
    {
        if let Some(project_display_name) = normal_project_names.get(&conversation.project_id) {
            let node = conversation_node(conversation, Some(project_display_name.clone()));
            conversations_by_project
                .entry(node.project_id.clone())
                .or_default()
                .push(node);
        } else if scratch_project_ids.contains(&conversation.project_id) {
            no_project_conversations.push(conversation_node(conversation, None));
        }
    }

    let mut projects = normal_projects
        .iter()
        .map(|project| SidebarProjectNode {
            project: project_header(project),
            is_expanded: expanded_project_ids.contains(&project.id),
            conversations: conversations_by_project
                .remove(&project.id)
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    for project in &mut projects {
        sort_conversations_by_recency(&mut project.conversations);
    }
    sort_conversations_by_recency(&mut no_project_conversations);

    let mut pinned_conversations = projects
        .iter()
        .flat_map(|project| project.conversations.iter())
        .chain(no_project_conversations.iter())
        .filter(|conversation| conversation.pinned)
        .cloned()
        .collect::<Vec<_>>();
    sort_conversations_by_recency(&mut pinned_conversations);

    let mut pinned_projects = projects
        .iter()
        .map(|project| project.project.clone())
        .filter(|project| project.pinned)
        .collect::<Vec<_>>();
    pinned_projects.sort_by_key(|project| Reverse(project.updated_at));

    let pinned = pinned_conversations
        .into_iter()
        .map(SidebarPinnedEntry::Conversation)
        .chain(pinned_projects.into_iter().map(SidebarPinnedEntry::Project))
        .collect();

    SidebarSnapshot {
        pinned,
        projects,
        no_project_conversations,
    }
}

fn project_header(project: &WorkspaceProjectInput) -> SidebarProjectHeader {
    SidebarProjectHeader {
        id: project.id.clone(),
        path: project.path.clone(),
        display_name: project.display_name.clone().into(),
        updated_at: project.updated_at,
        pinned: project.pinned,
    }
}

fn conversation_node(
    conversation: &WorkspaceConversationInput,
    project_display_name: Option<SharedString>,
) -> SidebarConversationNode {
    debug_assert_eq!(conversation.status, ConversationStatus::Active);
    SidebarConversationNode {
        id: conversation.id.clone(),
        project_id: conversation.project_id.clone(),
        title: conversation.title.clone().into(),
        recency_at: conversation.recency_at,
        project_display_name,
        pinned: conversation.pinned,
    }
}

fn sort_conversations_by_recency(conversations: &mut [SidebarConversationNode]) {
    conversations.sort_by(|left, right| {
        right
            .recency_at
            .cmp(&left.recency_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, kind: ProjectKind) -> WorkspaceProjectInput {
        WorkspaceProjectInput {
            id: id.to_string(),
            kind,
            path: PathBuf::from(format!("/tmp/{id}")),
            display_name: id.to_string(),
            pinned: false,
            updated_at: 1,
        }
    }

    fn conversation(
        id: &str,
        project_id: &str,
        title: &str,
        pinned: bool,
    ) -> WorkspaceConversationInput {
        WorkspaceConversationInput {
            id: id.to_string(),
            project_id: project_id.to_string(),
            title: title.to_string(),
            pinned,
            status: ConversationStatus::Active,
            recency_at: OffsetDateTime::UNIX_EPOCH,
            deleted_at: None,
        }
    }

    #[test]
    fn renamed_summary_rebuilds_project_pinned_and_scratch_projections() {
        let projects = [
            project("project-1", ProjectKind::Normal),
            project("scratch-1", ProjectKind::Scratch),
        ];
        let mut conversations = vec![
            conversation("conversation-1", "project-1", "Old", true),
            conversation("conversation-2", "scratch-1", "Scratch Old", false),
        ];
        let expanded = HashSet::from(["project-1".to_string()]);

        conversations[0].title = "Renamed".to_string();
        conversations[1].title = "Scratch Renamed".to_string();
        let snapshot = build_sidebar_snapshot(&expanded, &projects, &conversations);

        assert_eq!(snapshot.projects[0].conversations[0].title, "Renamed");
        assert_eq!(
            snapshot.projects[0].conversations[0].project_display_name,
            Some("project-1".into())
        );
        assert_eq!(
            snapshot.no_project_conversations[0].title,
            "Scratch Renamed"
        );
        assert_eq!(
            snapshot.no_project_conversations[0].project_display_name,
            None
        );
        assert!(matches!(
            &snapshot.pinned[0],
            SidebarPinnedEntry::Conversation(conversation)
                if conversation.title == "Renamed"
                    && conversation.project_display_name.as_deref() == Some("project-1")
        ));
    }

    #[test]
    fn conversations_sort_by_recency_then_id_in_every_projection() {
        let projects = [project("project-1", ProjectKind::Normal)];
        let mut conversations = vec![
            conversation("conversation-b", "project-1", "B", true),
            conversation("conversation-a", "project-1", "A", true),
            conversation("conversation-new", "project-1", "New", true),
        ];
        conversations[2].recency_at = OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(1);

        let snapshot = build_sidebar_snapshot(&HashSet::new(), &projects, &conversations);
        let project_ids = snapshot.projects[0]
            .conversations
            .iter()
            .map(|conversation| conversation.id.as_str())
            .collect::<Vec<_>>();
        let pinned_ids = snapshot
            .pinned
            .iter()
            .filter_map(|entry| match entry {
                SidebarPinnedEntry::Conversation(conversation) => Some(conversation.id.as_str()),
                SidebarPinnedEntry::Project(_) => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            project_ids,
            ["conversation-new", "conversation-a", "conversation-b"]
        );
        assert_eq!(pinned_ids, project_ids);
    }

    #[test]
    fn empty_or_unrelated_archive_keeps_route_unchanged() {
        let mut route = HomeRoute::Conversation("conversation-1".to_string());
        assert!(!reset_route_after_archived(&mut route, std::iter::empty()));
        assert_eq!(route, HomeRoute::Conversation("conversation-1".to_string()));

        assert!(!reset_route_after_archived(
            &mut route,
            [&"conversation-2".to_string()].into_iter(),
        ));
        assert_eq!(route, HomeRoute::Conversation("conversation-1".to_string()));

        assert!(reset_route_after_archived(
            &mut route,
            [&"conversation-1".to_string()].into_iter(),
        ));
        assert_eq!(route, HomeRoute::NewConversation);
    }
}
