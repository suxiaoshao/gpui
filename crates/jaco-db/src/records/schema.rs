use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaMetadataRecord {
    pub schema_version: i32,
    pub created_app_version: Option<String>,
    pub last_opened_app_version: Option<String>,
    pub payload: SchemaMetadataPayload,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
