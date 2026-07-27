use std::{collections::HashSet, fmt};

use gpui::SharedString;
use gpui::{App, Task};
use gpui_operation::refresh;
use jaco_core::{ConversationId, ConversationSummary, ProjectId, ProjectKind};

use crate::{
    database,
    features::conversation::registry::ConversationCatalogOperation,
    state::projects::{self, ProjectOperation},
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TemporaryConversationNode {
    pub(crate) id: ConversationId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: SharedString,
    pub(crate) updated_at: i128,
    pub(crate) pinned: bool,
}
pub(crate) type TemporarySearchOperation =
    refresh::Operation<TemporaryConversationSnapshot, TemporarySearchProblem, Task<()>>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TemporaryConversationSnapshot {
    pub(crate) conversations: Vec<TemporaryConversationNode>,
}

#[derive(Debug)]
pub(crate) struct TemporarySearchProblem(jaco_db::DbError);

impl fmt::Display for TemporarySearchProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for TemporarySearchProblem {}

impl From<jaco_db::DbError> for TemporarySearchProblem {
    fn from(error: jaco_db::DbError) -> Self {
        Self(error)
    }
}

pub(crate) fn empty_snapshot(cx: &App) -> jaco_db::Result<TemporaryConversationSnapshot> {
    let scratch_projects = projects::catalog(cx).read(cx, |operation| match operation {
        ProjectOperation::Ready(ready) => Ok(ready
            .data()
            .projects()
            .iter()
            .filter(|project| project.kind == ProjectKind::Scratch && !project.removed)
            .map(|project| project.id.clone())
            .collect::<HashSet<_>>()),
        _ => Err(jaco_db::DbError::Invariant(
            "project resource is not ready".to_string(),
        )),
    })?;
    let registry = crate::app::session::ready_conversations(cx).ok_or_else(|| {
        jaco_db::DbError::Invariant("conversation session is not ready".to_string())
    })?;
    let catalog = registry.read(cx).catalog();
    match catalog.read(cx).operation() {
        ConversationCatalogOperation::Ready(ready) => Ok(TemporaryConversationSnapshot {
            conversations: ready
                .data()
                .iter()
                .filter(|conversation| scratch_projects.contains(&conversation.project_id))
                .cloned()
                .map(conversation_node)
                .collect(),
        }),
        _ => Err(jaco_db::DbError::Invariant(
            "conversation index is not ready".to_string(),
        )),
    }
}

pub(crate) fn search(
    query: String,
    cx: &mut App,
) -> Task<Result<TemporaryConversationSnapshot, TemporarySearchProblem>> {
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(TemporarySearchProblem(error))),
    };
    cx.spawn(async move |_| {
        executor
            .execute(move |repository| {
                let conversations = jaco_conversation::ConversationService::new(repository)
                    .load_scratch_catalog(&query)
                    .map_err(|error| match error {
                        jaco_conversation::ConversationError::Database(error) => error,
                    })?
                    .into_iter()
                    .map(conversation_node)
                    .collect();
                Ok(TemporaryConversationSnapshot { conversations })
            })
            .await
            .map_err(TemporarySearchProblem)
    })
}

fn conversation_node(conversation: ConversationSummary) -> TemporaryConversationNode {
    TemporaryConversationNode {
        id: conversation.id,
        project_id: conversation.project_id,
        title: conversation.title.into(),
        updated_at: conversation.updated_at.unix_timestamp_nanos(),
        pinned: conversation.pinned,
    }
}
