use std::path::{Path, PathBuf};

use ai_chat_core::{ProjectId, ProjectKind, ProjectMetadata, new_id};
use ai_chat_db::{NewProject, ProjectRecord};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};

use crate::{database, errors::AiChat2Result, foundation::I18n, state::AiChat2Config};

const SCRATCH_PROJECTS_DIR: &str = "scratch-projects";
const NO_PROJECT_SCRATCH_REASON: &str = "no-project";

#[derive(Clone)]
pub(crate) struct ProjectCatalogGlobal(Entity<ProjectCatalogStore>);

impl ProjectCatalogGlobal {
    pub(crate) fn entity(&self) -> Entity<ProjectCatalogStore> {
        self.0.clone()
    }
}

impl Global for ProjectCatalogGlobal {}

pub(crate) struct ProjectCatalogStore {
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCatalogEvent {
    Changed(ProjectCatalogChange),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCatalogChange {
    Added { project_id: ProjectId },
    Renamed { project_id: ProjectId },
    Removed { project_id: ProjectId },
    PinChanged { project_id: ProjectId, pinned: bool },
}

impl EventEmitter<ProjectCatalogEvent> for ProjectCatalogStore {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertExistingFolderProjectResult {
    pub(crate) project: ProjectRecord,
    pub(crate) was_existing: bool,
}

impl ProjectCatalogStore {
    fn new() -> Self {
        Self { revision: 0 }
    }

    #[cfg(test)]
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn insert_existing_folder_project(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<InsertExistingFolderProjectResult> {
        let project_path = path.display().to_string();
        let repository = database::repository(cx);

        if let Some(project) = repository.get_project_by_path(&project_path)? {
            if project.removed {
                let restored = repository.set_project_removed(&project.id, false)?;
                self.emit_changed(
                    ProjectCatalogChange::Added {
                        project_id: restored.id.clone(),
                    },
                    cx,
                );
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
        self.emit_changed(
            ProjectCatalogChange::Added {
                project_id: project.id.clone(),
            },
            cx,
        );

        Ok(InsertExistingFolderProjectResult {
            project,
            was_existing: false,
        })
    }

    pub(crate) fn insert_scratch_project(
        &mut self,
        path: PathBuf,
        display_name: String,
        scratch_reason: String,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProjectRecord> {
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
        self.emit_changed(
            ProjectCatalogChange::Added {
                project_id: project.id.clone(),
            },
            cx,
        );
        Ok(project)
    }

    pub(crate) fn rename_project(
        &mut self,
        project_id: &ProjectId,
        display_name: String,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProjectRecord> {
        let project = database::repository(cx).rename_project(project_id, display_name)?;
        self.emit_changed(
            ProjectCatalogChange::Renamed {
                project_id: project.id.clone(),
            },
            cx,
        );
        Ok(project)
    }

    pub(crate) fn set_project_pinned(
        &mut self,
        project_id: &ProjectId,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProjectRecord> {
        let repository = database::repository(cx);
        let project = repository.set_project_pinned(project_id, pinned)?;
        self.emit_changed(
            ProjectCatalogChange::PinChanged {
                project_id: project.id.clone(),
                pinned,
            },
            cx,
        );
        Ok(project)
    }

    pub(crate) fn set_project_removed(
        &mut self,
        project_id: &ProjectId,
        removed: bool,
        cx: &mut Context<Self>,
    ) -> ai_chat_db::Result<ProjectRecord> {
        let project = database::repository(cx).set_project_removed(project_id, removed)?;
        self.emit_changed(
            ProjectCatalogChange::Removed {
                project_id: project.id.clone(),
            },
            cx,
        );
        Ok(project)
    }

    fn emit_changed(&mut self, change: ProjectCatalogChange, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.emit(ProjectCatalogEvent::Changed(change));
        cx.notify();
    }
}

pub(crate) fn init(cx: &mut App) {
    let store = cx.new(|_| ProjectCatalogStore::new());
    cx.set_global(ProjectCatalogGlobal(store));
}

pub(crate) fn catalog(cx: &App) -> Entity<ProjectCatalogStore> {
    cx.global::<ProjectCatalogGlobal>().entity()
}

pub(crate) fn normal_projects(cx: &App) -> ai_chat_db::Result<Vec<ProjectRecord>> {
    database::repository(cx).list_sidebar_projects()
}

pub(crate) fn insert_existing_folder_project(
    cx: &mut App,
    path: PathBuf,
) -> ai_chat_db::Result<InsertExistingFolderProjectResult> {
    catalog(cx).update(cx, |catalog, cx| {
        catalog.insert_existing_folder_project(path, cx)
    })
}

pub(crate) fn create_anonymous_scratch_project(cx: &mut App) -> AiChat2Result<ProjectRecord> {
    let id = new_id();
    let path = cx
        .global::<AiChat2Config>()
        .data_dir()?
        .join(SCRATCH_PROJECTS_DIR)
        .join(&id);
    std::fs::create_dir_all(&path)?;
    let display_name = cx.global::<I18n>().t("anonymous-project-name");
    Ok(catalog(cx).update(cx, |catalog, cx| {
        catalog.insert_scratch_project(
            path,
            display_name,
            NO_PROJECT_SCRATCH_REASON.to_string(),
            cx,
        )
    })?)
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
    use ai_chat_core::ProjectKind;
    use std::path::Path;

    #[test]
    fn project_display_name_uses_path_last_component() {
        assert_eq!(
            project_display_name(Path::new("/tmp/ai-chat-project")),
            "ai-chat-project"
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

    #[test]
    fn project_catalog_revision_starts_at_zero() {
        assert_eq!(super::ProjectCatalogStore::new().revision(), 0);
    }
}
