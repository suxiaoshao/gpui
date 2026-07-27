use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub last_opened_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    pub path: String,
    pub display_name: String,
    pub kind: ProjectKind,
    pub pinned: bool,
    pub removed: bool,
    pub metadata: ProjectMetadata,
}
