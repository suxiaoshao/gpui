use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct PromptRecord {
    pub id: PromptId,
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewPrompt {
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdatePrompt {
    pub name: String,
    pub content: PromptContent,
    pub enabled: bool,
    pub sort_order: i32,
}
