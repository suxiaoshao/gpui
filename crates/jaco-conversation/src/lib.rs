use jaco_core::{Conversation, ConversationId, ConversationSummary};
use jaco_db::{ConversationTimelineRecords, FreshRepository};

pub type Result<T> = std::result::Result<T, ConversationError>;

#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error(transparent)]
    Database(#[from] jaco_db::DbError),
}

pub struct ConversationService<'a> {
    repository: &'a FreshRepository,
}

impl<'a> ConversationService<'a> {
    pub fn new(repository: &'a FreshRepository) -> Self {
        Self { repository }
    }

    pub fn load_catalog(&self) -> Result<Vec<ConversationSummary>> {
        self.repository
            .list_sidebar_conversations()
            .map_err(Into::into)
    }

    pub fn load(&self, id: &ConversationId) -> Result<Option<Conversation>> {
        self.repository
            .conversation_timeline_records(id)
            .map(|records| records.map(conversation_from_records))
            .map_err(Into::into)
    }

    pub fn search_catalog(&self, query: &str, limit: usize) -> Result<Vec<ConversationSummary>> {
        self.repository
            .search_sidebar_conversations(query, limit)
            .map_err(Into::into)
    }

    pub fn load_scratch_catalog(&self, query: &str) -> Result<Vec<ConversationSummary>> {
        self.repository
            .list_no_project_conversations(query)
            .map_err(Into::into)
    }

    pub fn set_pinned(&self, id: &ConversationId, pinned: bool) -> Result<ConversationSummary> {
        self.repository
            .set_conversation_pinned(id, pinned)
            .map_err(Into::into)
    }

    pub fn delete(&self, id: &ConversationId) -> Result<ConversationSummary> {
        self.repository
            .soft_delete_conversation(id)
            .map_err(Into::into)
    }
}

fn conversation_from_records(records: ConversationTimelineRecords) -> Conversation {
    Conversation {
        summary: records.conversation,
        project: records.project,
        entries: records.items,
        attachments: records.attachments,
        runs: records.runs,
        provider_steps: records.provider_steps,
        tool_invocations: records.tool_invocations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_db::FreshStore;

    #[test]
    fn empty_store_has_ready_empty_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3")).unwrap();
        let repository = store.repository();
        let service = ConversationService::new(&repository);

        assert_eq!(service.load_catalog().unwrap(), Vec::new());
    }
}
