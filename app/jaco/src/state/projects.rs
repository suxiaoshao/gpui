use std::path::{Path, PathBuf};

use std::fmt;

use gpui::{App, Task};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::{Select, Store};
use jaco_core::{ProjectId, ProjectKind, ProjectMetadata, new_id};
use jaco_db::{NewProject, ProjectRecord};
use tokio::sync::oneshot;

use crate::{database, errors::JacoResult, foundation::I18n, state::config};

const SCRATCH_PROJECTS_DIR: &str = "scratch-projects";
const NO_PROJECT_SCRATCH_REASON: &str = "no-project";

pub(crate) type ProjectOperation = refresh::Operation<ProjectData, ProjectProblem, Task<()>>;
pub(crate) type ProjectStore = Store<ProjectOperation>;

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectNormalProjects;

impl Select<ProjectOperation> for SelectNormalProjects {
    type Output = Option<Vec<ProjectRecord>>;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        operation.data().map(|data| {
            data.projects()
                .iter()
                .filter(|project| project.kind == ProjectKind::Normal && !project.removed)
                .cloned()
                .collect()
        })
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProjectStatus;

impl Select<ProjectOperation> for SelectProjectStatus {
    type Output = (gpui_operation::refresh::Phase, Option<String>);

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        (
            operation.phase(),
            operation.problem().map(ToString::to_string),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalProjectCatalogSnapshot {
    projects: Option<Vec<ProjectRecord>>,
    phase: gpui_operation::refresh::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectNormalProjectCatalog;

impl Select<ProjectOperation> for SelectNormalProjectCatalog {
    type Output = NormalProjectCatalogSnapshot;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        NormalProjectCatalogSnapshot {
            projects: operation.data().map(|data| {
                data.projects()
                    .iter()
                    .filter(|project| project.kind == ProjectKind::Normal && !project.removed)
                    .cloned()
                    .collect()
            }),
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProjectData {
    projects: Vec<ProjectRecord>,
}

impl ProjectData {
    pub(crate) fn projects(&self) -> &Vec<ProjectRecord> {
        &self.projects
    }

    fn upsert(&mut self, project: ProjectRecord) {
        match self
            .projects
            .iter_mut()
            .find(|current| current.id == project.id)
        {
            Some(current) => *current = project,
            None => self.projects.push(project),
        }
        sort_projects(&mut self.projects);
    }
}

#[derive(Debug)]
pub(crate) struct ProjectProblem(jaco_db::DbError);

impl fmt::Display for ProjectProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ProjectProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub(crate) struct UpsertProject(pub(crate) ProjectRecord);

impl Transition<UpsertProject> for &mut ProjectData {
    type Output = ();

    fn transition(self, message: UpsertProject) {
        self.upsert(message.0);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertExistingFolderProjectResult {
    pub(crate) project: ProjectRecord,
    pub(crate) was_existing: bool,
}

fn sort_projects(projects: &mut [ProjectRecord]) {
    projects.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(crate) fn publish_project(project: ProjectRecord, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        let ProjectOperation::Ready(ready) = operation else {
            panic!("project commit requires an exact Ready operation");
        };
        ready.transition(UpsertProject(project));
    });
}

fn ensure_ready(cx: &App) -> jaco_db::Result<()> {
    catalog(cx).read(cx, |operation| {
        matches!(operation, ProjectOperation::Ready(_))
            .then_some(())
            .ok_or_else(|| jaco_db::DbError::Invariant("project resource is not ready".to_string()))
    })
}

fn insert_existing_folder_project_impl(
    path: PathBuf,
) -> impl FnOnce(&jaco_db::FreshRepository) -> jaco_db::Result<InsertExistingFolderProjectResult> {
    let project_path = path.display().to_string();
    move |repository| {
        if let Some(project) = repository.get_project_by_path(&project_path)? {
            if project.removed {
                let restored = repository.set_project_removed(&project.id, false)?;
                return Ok(InsertExistingFolderProjectResult {
                    project: restored,
                    was_existing: true,
                });
            }

            return Ok(InsertExistingFolderProjectResult {
                project,
                was_existing: true,
            });
        }

        let project = repository.insert_project(NewProject {
            path: project_path,
            display_name: project_display_name(&path),
            kind: ProjectKind::Normal,
            pinned: false,
            removed: false,
            metadata: empty_project_metadata(),
        })?;
        Ok(InsertExistingFolderProjectResult {
            project,
            was_existing: false,
        })
    }
}

pub(crate) fn rename_project(
    project_id: ProjectId,
    display_name: String,
    cx: &mut App,
) -> Task<jaco_db::Result<ProjectRecord>> {
    spawn_project_mutation(
        cx,
        move |repo| repo.rename_project(&project_id, display_name),
        |project, cx| publish_project(project.clone(), cx),
    )
}

pub(crate) fn set_project_pinned(
    project_id: ProjectId,
    pinned: bool,
    cx: &mut App,
) -> Task<jaco_db::Result<ProjectRecord>> {
    spawn_project_mutation(
        cx,
        move |repo| repo.set_project_pinned(&project_id, pinned),
        |project, cx| publish_project(project.clone(), cx),
    )
}

pub(crate) fn set_project_removed(
    project_id: ProjectId,
    removed: bool,
    cx: &mut App,
) -> Task<jaco_db::Result<ProjectRecord>> {
    spawn_project_mutation(
        cx,
        move |repo| repo.set_project_removed(&project_id, removed),
        |project, cx| publish_project(project.clone(), cx),
    )
}

pub(crate) fn init(cx: &mut App) {
    ProjectStore::install_global(cx, ProjectOperation::new());
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_projects().map(|mut projects| {
                    sort_projects(&mut projects);
                    ProjectData { projects }
                })
            })
            .await
            .map_err(ProjectProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if matches!(operation, ProjectOperation::Loading(_)) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| operation.transition(Load(task)));
}

pub(crate) fn catalog(cx: &impl gpui::AppContext) -> ProjectStore {
    ProjectStore::global(cx)
}

pub(crate) fn request_refresh(cx: &mut App) {
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    if catalog(cx).read(cx, ProjectOperation::is_running) {
        return;
    }
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_projects().map(|mut projects| {
                    sort_projects(&mut projects);
                    ProjectData { projects }
                })
            })
            .await
            .map_err(ProjectProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if operation.is_running() {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| match operation {
        ProjectOperation::Idle(_) => operation.transition(Load(task)),
        ProjectOperation::Ready(_) | ProjectOperation::Degraded(_) => {
            operation.transition(Refresh(task))
        }
        ProjectOperation::Unavailable(_) => operation.transition(Retry(task)),
        ProjectOperation::Loading(_)
        | ProjectOperation::Refreshing(_)
        | ProjectOperation::Retrying(_)
        | ProjectOperation::RefreshingDegraded(_) => {}
    });
}

pub(crate) fn insert_existing_folder_project(
    cx: &mut App,
    path: PathBuf,
) -> Task<jaco_db::Result<InsertExistingFolderProjectResult>> {
    spawn_project_mutation(
        cx,
        insert_existing_folder_project_impl(path),
        |result, cx| publish_project(result.project.clone(), cx),
    )
}

pub(crate) fn prepare_anonymous_scratch_project(
    cx: &App,
) -> JacoResult<(ProjectId, PathBuf, NewProject)> {
    let id = new_id();
    let path = config::data_dir(cx)?.join(SCRATCH_PROJECTS_DIR).join(&id);
    let mut metadata = empty_project_metadata();
    metadata.scratch_reason = Some(NO_PROJECT_SCRATCH_REASON.to_string());
    Ok((
        id,
        path.clone(),
        NewProject {
            path: path.display().to_string(),
            display_name: cx.global::<I18n>().t("anonymous-project-name").to_string(),
            kind: ProjectKind::Scratch,
            pinned: false,
            removed: false,
            metadata,
        },
    ))
}

fn spawn_project_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&jaco_db::FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    publish: impl FnOnce(&R, &mut App) + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static,
{
    if let Err(error) = ensure_ready(cx) {
        return Task::ready(Err(error));
    }
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(jaco_db::DbError::Invariant(
            "project mutation requires an exact Ready session".to_string(),
        )));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    let task_binding = binding.clone();
    let driver = cx.spawn(async move |cx| {
        let result = executor.execute(command).await;
        if let Ok(value) = &result {
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() == Some(&binding) {
                    publish(value, cx);
                }
            });
        }
        let _ = sender.send(result);
    });
    crate::app::session::retain_task(task_binding, driver, cx);
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(jaco_db::DbError::Invariant(
                "project mutation driver ended without a result".to_string(),
            ))
        })
    })
}

pub(crate) fn project_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
fn project_kind_is_normal(kind: ProjectKind) -> bool {
    kind == ProjectKind::Normal
}

fn empty_project_metadata() -> ProjectMetadata {
    ProjectMetadata {
        scratch_reason: None,
        git_root: None,
        last_active_conversation_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{empty_project_metadata, project_display_name, project_kind_is_normal};
    use jaco_core::ProjectKind;
    use std::path::Path;

    #[test]
    fn project_display_name_uses_path_last_component() {
        assert_eq!(
            project_display_name(Path::new("/tmp/jaco-project")),
            "jaco-project"
        );
    }

    #[test]
    fn project_display_name_falls_back_to_full_path() {
        let path = Path::new("/");

        assert_eq!(project_display_name(path), path.display().to_string());
    }

    #[test]
    fn project_kind_filter_accepts_only_normal_projects() {
        assert!(project_kind_is_normal(ProjectKind::Normal));
        assert!(!project_kind_is_normal(ProjectKind::Scratch));
    }

    #[test]
    fn empty_project_metadata_defaults() {
        let metadata = empty_project_metadata();

        assert_eq!(metadata.scratch_reason, None);
        assert_eq!(metadata.git_root, None);
        assert_eq!(metadata.last_active_conversation_id, None);
    }
}
