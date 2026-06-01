use std::path::{Path, PathBuf};

use ai_chat_core::{ProjectKind, ProjectMetadata};
use ai_chat_db::{NewProject, ProjectRecord};
use gpui::App;

use crate::database;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertExistingFolderProjectResult {
    pub(crate) project: ProjectRecord,
    pub(crate) was_existing: bool,
}

pub(crate) fn normal_projects(cx: &App) -> ai_chat_db::Result<Vec<ProjectRecord>> {
    let projects = database::repository(cx).list_projects()?;
    Ok(projects
        .into_iter()
        .filter(|project| project_kind_is_normal(project.kind))
        .collect())
}

pub(crate) fn insert_existing_folder_project(
    cx: &App,
    path: PathBuf,
) -> ai_chat_db::Result<InsertExistingFolderProjectResult> {
    let project_path = path.display().to_string();
    let repository = database::repository(cx);

    if let Some(project) = repository.get_project_by_path(&project_path)? {
        return Ok(InsertExistingFolderProjectResult {
            project,
            was_existing: true,
        });
    }

    let project = repository.insert_project(NewProject {
        path: project_path,
        display_name: project_display_name(&path),
        kind: ProjectKind::Normal,
        metadata: empty_project_metadata(),
    })?;

    Ok(InsertExistingFolderProjectResult {
        project,
        was_existing: false,
    })
}

pub(crate) fn project_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

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
    use super::{project_display_name, project_kind_is_normal};
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
}
