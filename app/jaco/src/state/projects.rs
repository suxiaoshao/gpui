use std::path::{Path, PathBuf};

use gpui::{App, Global};
use gpui_store::{SharedStore, StoreState};
use jaco_core::{ProjectId, ProjectKind, ProjectMetadata, new_id};
use jaco_db::{NewProject, ProjectRecord};

use crate::{database, errors::JacoResult, foundation::I18n, state::config};

const SCRATCH_PROJECTS_DIR: &str = "scratch-projects";
const NO_PROJECT_SCRATCH_REASON: &str = "no-project";

#[derive(Clone)]
pub(crate) struct ProjectCatalogGlobal(SharedStore<ProjectCatalogSnapshot>);

impl ProjectCatalogGlobal {
    pub(crate) fn store(&self) -> SharedStore<ProjectCatalogSnapshot> {
        self.0.clone()
    }
}

impl Global for ProjectCatalogGlobal {}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProjectCatalogSnapshot {
    projects: Vec<ProjectRecord>,
}

impl StoreState for ProjectCatalogSnapshot {}

impl ProjectCatalogSnapshot {
    pub(crate) fn projects(&self) -> &Vec<ProjectRecord> {
        &self.projects
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertExistingFolderProjectResult {
    pub(crate) project: ProjectRecord,
    pub(crate) was_existing: bool,
}

fn refresh_snapshot(store: &SharedStore<ProjectCatalogSnapshot>, cx: &mut App) {
    let Ok(projects) = database::repository(cx).list_sidebar_projects() else {
        return;
    };
    store.update(cx, |snapshot| {
        snapshot.projects = projects;
    });
}

fn insert_existing_folder_project_impl(
    path: PathBuf,
    cx: &mut App,
) -> jaco_db::Result<InsertExistingFolderProjectResult> {
    let store = catalog(cx);
    let project_path = path.display().to_string();
    let repository = database::repository(cx);

    if let Some(project) = repository.get_project_by_path(&project_path)? {
        if project.removed {
            let restored = repository.set_project_removed(&project.id, false)?;
            refresh_snapshot(&store, cx);
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
    refresh_snapshot(&store, cx);

    Ok(InsertExistingFolderProjectResult {
        project,
        was_existing: false,
    })
}

fn insert_scratch_project(
    path: PathBuf,
    display_name: String,
    scratch_reason: String,
    cx: &mut App,
) -> jaco_db::Result<ProjectRecord> {
    let store = catalog(cx);
    let mut metadata = empty_project_metadata();
    metadata.scratch_reason = Some(scratch_reason);
    let project = database::repository(cx).insert_project(NewProject {
        path: path.display().to_string(),
        display_name,
        kind: ProjectKind::Scratch,
        pinned: false,
        removed: false,
        metadata,
    })?;
    refresh_snapshot(&store, cx);
    Ok(project)
}

pub(crate) fn rename_project(
    project_id: &ProjectId,
    display_name: String,
    cx: &mut App,
) -> jaco_db::Result<ProjectRecord> {
    let store = catalog(cx);
    let project = database::repository(cx).rename_project(project_id, display_name)?;
    refresh_snapshot(&store, cx);
    Ok(project)
}

pub(crate) fn set_project_pinned(
    project_id: &ProjectId,
    pinned: bool,
    cx: &mut App,
) -> jaco_db::Result<ProjectRecord> {
    let store = catalog(cx);
    let repository = database::repository(cx);
    let project = repository.set_project_pinned(project_id, pinned)?;
    refresh_snapshot(&store, cx);
    Ok(project)
}

pub(crate) fn set_project_removed(
    project_id: &ProjectId,
    removed: bool,
    cx: &mut App,
) -> jaco_db::Result<ProjectRecord> {
    let store = catalog(cx);
    let project = database::repository(cx).set_project_removed(project_id, removed)?;
    refresh_snapshot(&store, cx);
    Ok(project)
}

pub(crate) fn init(cx: &mut App) {
    let projects = database::repository(cx)
        .list_sidebar_projects()
        .unwrap_or_default();
    let store = SharedStore::new(cx, ProjectCatalogSnapshot { projects });
    cx.set_global(ProjectCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> SharedStore<ProjectCatalogSnapshot> {
    cx.global::<ProjectCatalogGlobal>().store()
}

pub(crate) fn insert_existing_folder_project(
    cx: &mut App,
    path: PathBuf,
) -> jaco_db::Result<InsertExistingFolderProjectResult> {
    insert_existing_folder_project_impl(path, cx)
}

pub(crate) fn create_anonymous_scratch_project(cx: &mut App) -> JacoResult<ProjectRecord> {
    let id = new_id();
    let path = config::data_dir(cx)?.join(SCRATCH_PROJECTS_DIR).join(&id);
    std::fs::create_dir_all(&path)?;
    let display_name = cx.global::<I18n>().t("anonymous-project-name");
    Ok(insert_scratch_project(
        path,
        display_name,
        NO_PROJECT_SCRATCH_REASON.to_string(),
        cx,
    )?)
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
