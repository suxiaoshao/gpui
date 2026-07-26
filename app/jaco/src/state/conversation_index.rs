use std::fmt;

use gpui::{App, Task};
use gpui_operation::{Complete, Load, Transition, refresh};
use gpui_store::Store;
use jaco_core::{ConversationId, ConversationStatus};
use jaco_db::{ConversationIndexDelta, ConversationRecord};

use crate::database;

pub(crate) type ConversationIndexOperation =
    refresh::Operation<ConversationIndexData, ConversationIndexProblem, Task<()>>;
pub(crate) type ConversationIndexStore = Store<ConversationIndexOperation>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ConversationIndexData {
    conversations: Vec<ConversationRecord>,
}

impl ConversationIndexData {
    pub(crate) fn conversations(&self) -> &[ConversationRecord] {
        &self.conversations
    }

    fn upsert(&mut self, conversation: ConversationRecord) {
        if conversation.status != ConversationStatus::Active {
            self.remove(&conversation.id);
            return;
        }
        match self
            .conversations
            .iter_mut()
            .find(|current| current.id == conversation.id)
        {
            Some(current) => *current = conversation,
            None => self.conversations.push(conversation),
        }
        sort_conversations(&mut self.conversations);
    }

    fn remove(&mut self, conversation_id: &ConversationId) {
        self.conversations
            .retain(|conversation| &conversation.id != conversation_id);
    }
}

#[derive(Debug)]
pub(crate) struct ConversationIndexProblem(jaco_db::DbError);

impl fmt::Display for ConversationIndexProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ConversationIndexProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub(crate) enum ConversationIndexMessage {
    Upsert(ConversationRecord),
    Remove(ConversationId),
    ApplyCommitted {
        conversation: ConversationRecord,
        delta: ConversationIndexDelta,
    },
}

impl Transition<ConversationIndexMessage> for &mut ConversationIndexData {
    type Output = ();

    fn transition(self, message: ConversationIndexMessage) {
        match message {
            ConversationIndexMessage::Upsert(conversation) => self.upsert(conversation),
            ConversationIndexMessage::Remove(conversation_id) => self.remove(&conversation_id),
            ConversationIndexMessage::ApplyCommitted {
                conversation,
                delta,
            } => self.apply_committed(conversation, delta),
        }
    }
}

impl ConversationIndexData {
    fn apply_committed(&mut self, conversation: ConversationRecord, delta: ConversationIndexDelta) {
        match delta {
            ConversationIndexDelta::InsertIfMissing(record) => {
                if !self
                    .conversations
                    .iter()
                    .any(|current| current.id == record.id)
                {
                    self.upsert(*record);
                }
            }
            ConversationIndexDelta::EntryAdvanced {
                id,
                last_entry_seq,
                updated_at,
            } => {
                let Some(current) = self
                    .conversations
                    .iter_mut()
                    .find(|current| current.id == id)
                else {
                    tracing::warn!(
                        conversation_id = %conversation.id,
                        "conversation index entry delta refers to a missing conversation"
                    );
                    return;
                };
                if (last_entry_seq, updated_at) >= (current.last_entry_seq, current.updated_at) {
                    current.last_entry_seq = last_entry_seq;
                    current.updated_at = updated_at;
                    sort_conversations(&mut self.conversations);
                }
            }
            ConversationIndexDelta::PresentationChanged {
                id,
                title,
                pinned,
                status,
                updated_at,
            } => {
                let Some(current) = self
                    .conversations
                    .iter_mut()
                    .find(|current| current.id == id)
                else {
                    tracing::warn!(
                        conversation_id = %conversation.id,
                        "conversation presentation delta refers to a missing conversation"
                    );
                    return;
                };
                if let Some(title) = title {
                    current.title = title;
                }
                if let Some(pinned) = pinned {
                    current.pinned = pinned;
                }
                if let Some(status) = status {
                    current.status = status;
                }
                current.updated_at = current.updated_at.max(updated_at);
                if current.status != ConversationStatus::Active {
                    self.remove(&id);
                } else {
                    sort_conversations(&mut self.conversations);
                }
            }
            ConversationIndexDelta::Remove { id } => self.remove(&id),
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    ConversationIndexStore::install_global(cx, ConversationIndexOperation::new());
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository
                    .list_sidebar_conversations()
                    .map(|mut conversations| {
                        sort_conversations(&mut conversations);
                        ConversationIndexData { conversations }
                    })
            })
            .await
            .map_err(ConversationIndexProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if matches!(operation, ConversationIndexOperation::Loading(_)) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| operation.transition(Load(task)));
}

pub(crate) fn catalog(cx: &impl gpui::AppContext) -> ConversationIndexStore {
    ConversationIndexStore::global(cx)
}

pub(crate) fn publish(conversation: ConversationRecord, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        let ConversationIndexOperation::Ready(ready) = operation else {
            panic!("conversation index commit requires an exact Ready operation");
        };
        ready.transition(ConversationIndexMessage::Upsert(conversation));
    });
}

pub(crate) fn is_ready(cx: &impl gpui::AppContext) -> bool {
    catalog(cx).read(cx, |operation| {
        matches!(operation, ConversationIndexOperation::Ready(_))
    })
}

pub(crate) fn publish_removed(conversation_id: ConversationId, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        let ConversationIndexOperation::Ready(ready) = operation else {
            panic!("conversation index commit requires an exact Ready operation");
        };
        ready.transition(ConversationIndexMessage::Remove(conversation_id));
    });
}

pub(crate) fn publish_committed(
    conversation: ConversationRecord,
    delta: ConversationIndexDelta,
    cx: &mut App,
) {
    catalog(cx).update(cx, |operation| {
        let ConversationIndexOperation::Ready(ready) = operation else {
            panic!("runtime conversation commit requires an exact Ready index");
        };
        ready.transition(ConversationIndexMessage::ApplyCommitted {
            conversation,
            delta,
        });
    });
}

fn sort_conversations(conversations: &mut [ConversationRecord]) {
    conversations.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaco_core::{
        ConversationMetadata, ConversationSettingsSnapshot, ProjectId, ToolApprovalMode,
        ToolApprovalPolicy, ToolPolicySnapshot, ToolSource,
    };
    use time::OffsetDateTime;

    fn record(id: &str, updated_at: i64) -> ConversationRecord {
        ConversationRecord {
            id: id.into(),
            project_id: ProjectId::from("project"),
            title: id.to_string(),
            status: ConversationStatus::Active,
            pinned: false,
            prompt_id: None,
            default_provider_id: None,
            default_model_id: None,
            last_entry_seq: 0,
            metadata: ConversationMetadata {
                summary: None,
                tags: Vec::new(),
            },
            settings_snapshot: ConversationSettingsSnapshot {
                prompt: None,
                provider_id: None,
                model_id: None,
                model_capabilities: None,
                tool_policy: ToolPolicySnapshot {
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    enabled_sources: vec![ToolSource::Local],
                    max_steps: 1,
                    approval_mode: ToolApprovalMode::RequestApproval,
                    permission_scope: None,
                },
            },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::from_unix_timestamp(updated_at).expect("timestamp"),
            archived_at: None,
            deleted_at: None,
        }
    }

    #[test]
    fn upsert_is_authoritative_and_stably_sorted() {
        let mut data = ConversationIndexData::default();
        data.upsert(record("b", 1));
        data.upsert(record("a", 1));
        data.upsert(record("c", 2));

        assert_eq!(
            data.conversations()
                .iter()
                .map(|conversation| conversation.id.as_str())
                .collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
    }

    #[test]
    fn non_active_upsert_removes_existing_entry() {
        let mut data = ConversationIndexData::default();
        data.upsert(record("conversation", 1));
        let mut deleted = record("conversation", 2);
        deleted.status = ConversationStatus::Deleted;

        data.upsert(deleted);

        assert!(data.conversations().is_empty());
    }
}
