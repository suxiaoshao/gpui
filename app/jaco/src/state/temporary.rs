use std::{collections::HashSet, fmt};

use gpui::{App, Task};
use gpui_operation::refresh;
use jaco_core::ProjectKind;

use crate::{
    database,
    state::{
        conversation_index::{self, ConversationIndexOperation},
        projects::{self, ProjectOperation},
        workspace::{self, SidebarConversationNode},
    },
};

pub(crate) type TemporaryConversationNode = SidebarConversationNode;
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
    conversation_index::catalog(cx).read(cx, |operation| match operation {
        ConversationIndexOperation::Ready(ready) => Ok(TemporaryConversationSnapshot {
            conversations: ready
                .data()
                .conversations()
                .iter()
                .filter(|conversation| scratch_projects.contains(&conversation.project_id))
                .cloned()
                .map(workspace::conversation_record_node)
                .collect(),
        }),
        _ => Err(jaco_db::DbError::Invariant(
            "conversation index is not ready".to_string(),
        )),
    })
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
                let conversations = repository
                    .list_no_project_conversations(&query)?
                    .into_iter()
                    .map(workspace::conversation_record_node)
                    .collect();
                Ok(TemporaryConversationSnapshot { conversations })
            })
            .await
            .map_err(TemporarySearchProblem)
    })
}
