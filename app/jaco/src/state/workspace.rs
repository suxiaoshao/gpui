use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gpui::{App, AppContext, Context, Entity, Global, SharedString, Subscription};
use jaco_core::{ConversationId, ConversationStatus, ProjectId, ProjectKind};
use jaco_db::{ConversationRecord, ProjectRecord};

use crate::{database, state::projects};

#[derive(Clone)]
pub(crate) struct WorkspaceStoreGlobal(Entity<JacoWorkspaceStore>);

impl WorkspaceStoreGlobal {
    pub(crate) fn entity(&self) -> Entity<JacoWorkspaceStore> {
        self.0.clone()
    }
}

impl Global for WorkspaceStoreGlobal {}

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
    pub(crate) updated_at: i128,
    pub(crate) pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarSearchResult {
    pub(crate) conversation: SidebarConversationNode,
    pub(crate) project: Option<SidebarProjectHeader>,
}

pub(crate) struct JacoWorkspaceStore {
    route: HomeRoute,
    snapshot: SidebarSnapshot,
    expanded_project_ids: HashSet<ProjectId>,
    project_catalog: Entity<projects::ProjectCatalogStore>,
    pending_new_conversation_project_id: Option<ProjectId>,
    last_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl JacoWorkspaceStore {
    fn new(project_catalog: Entity<projects::ProjectCatalogStore>, cx: &mut Context<Self>) -> Self {
        let mut store = Self {
            route: HomeRoute::NewConversation,
            snapshot: SidebarSnapshot::default(),
            expanded_project_ids: HashSet::new(),
            project_catalog: project_catalog.clone(),
            pending_new_conversation_project_id: None,
            last_error: None,
            _subscriptions: Vec::new(),
        };
        store._subscriptions.push(cx.subscribe(
            &project_catalog,
            |store, _catalog, _event: &projects::ProjectCatalogEvent, cx| {
                store.reload_sidebar(cx);
            },
        ));
        store.reload_sidebar(cx);
        store
    }

    pub(crate) fn route(&self) -> &HomeRoute {
        &self.route
    }

    pub(crate) fn snapshot(&self) -> &SidebarSnapshot {
        &self.snapshot
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn reload_sidebar(&mut self, cx: &mut Context<Self>) {
        match build_sidebar_snapshot(&self.expanded_project_ids, cx) {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
            }
        }
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
        self.reload_sidebar(cx);
    }

    pub(crate) fn pin_project(
        &mut self,
        project_id: &ProjectId,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> jaco_db::Result<ProjectRecord> {
        self.project_catalog.update(cx, |catalog, cx| {
            catalog.set_project_pinned(project_id, pinned, cx)
        })
    }

    pub(crate) fn rename_project(
        &mut self,
        project_id: &ProjectId,
        display_name: String,
        cx: &mut Context<Self>,
    ) -> jaco_db::Result<ProjectRecord> {
        self.project_catalog.update(cx, |catalog, cx| {
            catalog.rename_project(project_id, display_name, cx)
        })
    }

    pub(crate) fn remove_project(
        &mut self,
        project_id: &ProjectId,
        cx: &mut Context<Self>,
    ) -> jaco_db::Result<ProjectRecord> {
        if self.route_belongs_to_project(project_id, cx) {
            self.route = HomeRoute::NewConversation;
        }
        self.project_catalog.update(cx, |catalog, cx| {
            catalog.set_project_removed(project_id, true, cx)
        })
    }

    pub(crate) fn pin_conversation(
        &mut self,
        conversation_id: &ConversationId,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> jaco_db::Result<ConversationRecord> {
        let conversation =
            database::repository(cx).set_conversation_pinned(conversation_id, pinned)?;
        self.reload_sidebar(cx);
        Ok(conversation)
    }

    pub(crate) fn delete_conversation(
        &mut self,
        conversation_id: &ConversationId,
        cx: &mut Context<Self>,
    ) -> jaco_db::Result<ConversationRecord> {
        let conversation = database::repository(cx).soft_delete_conversation(conversation_id)?;
        if matches!(&self.route, HomeRoute::Conversation(id) if id == conversation_id) {
            self.route = HomeRoute::NewConversation;
        }
        self.reload_sidebar(cx);
        Ok(conversation)
    }

    pub(crate) fn search_conversations(
        &self,
        query: &str,
        limit: usize,
        cx: &App,
    ) -> jaco_db::Result<Vec<SidebarSearchResult>> {
        let repository = database::repository(cx);
        let project_by_id = visible_project_headers(cx)?;
        Ok(repository
            .search_sidebar_conversations(query, limit)?
            .into_iter()
            .map(|conversation| SidebarSearchResult {
                project: project_by_id.get(&conversation.project_id).cloned(),
                conversation: conversation_node(conversation),
            })
            .collect())
    }

    fn route_belongs_to_project(&self, project_id: &ProjectId, cx: &App) -> bool {
        let HomeRoute::Conversation(conversation_id) = &self.route else {
            return false;
        };

        database::repository(cx)
            .get_conversation(conversation_id)
            .ok()
            .flatten()
            .is_some_and(|conversation| &conversation.project_id == project_id)
    }
}

pub(crate) fn init(cx: &mut App) {
    let project_catalog = projects::catalog(cx);
    let store = cx.new(|cx| JacoWorkspaceStore::new(project_catalog, cx));
    cx.set_global(WorkspaceStoreGlobal(store));
}

pub(crate) fn workspace(cx: &App) -> Entity<JacoWorkspaceStore> {
    cx.global::<WorkspaceStoreGlobal>().entity()
}

fn build_sidebar_snapshot(
    expanded_project_ids: &HashSet<ProjectId>,
    cx: &App,
) -> jaco_db::Result<SidebarSnapshot> {
    let repository = database::repository(cx);
    let visible_projects = repository.list_visible_projects()?;
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

    let normal_project_ids = normal_projects
        .iter()
        .map(|project| project.id.clone())
        .collect::<HashSet<_>>();
    let scratch_project_ids = visible_projects
        .iter()
        .filter(|project| project.kind == ProjectKind::Scratch)
        .map(|project| project.id.clone())
        .collect::<HashSet<_>>();
    let mut conversations_by_project: HashMap<ProjectId, Vec<SidebarConversationNode>> =
        HashMap::new();
    let mut no_project_conversations = Vec::new();

    for conversation in repository.list_sidebar_conversations()? {
        let node = conversation_node(conversation);
        if normal_project_ids.contains(&node.project_id) {
            conversations_by_project
                .entry(node.project_id.clone())
                .or_default()
                .push(node);
        } else if scratch_project_ids.contains(&node.project_id) {
            no_project_conversations.push(node);
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
        sort_conversations_by_updated_at(&mut project.conversations);
    }
    sort_conversations_by_updated_at(&mut no_project_conversations);

    let mut pinned_conversations = projects
        .iter()
        .flat_map(|project| project.conversations.iter())
        .chain(no_project_conversations.iter())
        .filter(|conversation| conversation.pinned)
        .cloned()
        .collect::<Vec<_>>();
    sort_conversations_by_updated_at(&mut pinned_conversations);

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

    Ok(SidebarSnapshot {
        pinned,
        projects,
        no_project_conversations,
    })
}

fn visible_project_headers(cx: &App) -> jaco_db::Result<HashMap<ProjectId, SidebarProjectHeader>> {
    Ok(database::repository(cx)
        .list_visible_projects()?
        .into_iter()
        .map(|project| (project.id.clone(), project_header(&project)))
        .collect())
}

fn project_header(project: &ProjectRecord) -> SidebarProjectHeader {
    SidebarProjectHeader {
        id: project.id.clone(),
        path: PathBuf::from(&project.path),
        display_name: project.display_name.clone().into(),
        updated_at: project.updated_at.unix_timestamp_nanos(),
        pinned: project.pinned,
    }
}

pub(crate) fn conversation_node(conversation: ConversationRecord) -> SidebarConversationNode {
    debug_assert_eq!(conversation.status, ConversationStatus::Active);
    SidebarConversationNode {
        id: conversation.id,
        project_id: conversation.project_id,
        title: conversation.title.into(),
        updated_at: conversation.updated_at.unix_timestamp_nanos(),
        pinned: conversation.pinned,
    }
}

fn sort_conversations_by_updated_at(conversations: &mut [SidebarConversationNode]) {
    conversations.sort_by_key(|conversation| Reverse(conversation.updated_at));
}
