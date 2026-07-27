use super::*;

pub type ProjectRecord = ProjectSummary;

#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
}
