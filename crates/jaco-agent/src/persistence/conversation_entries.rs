use super::{PersistenceContext, lock, mutex_clone, mutex_replace};
use crate::{
    AgentRuntimeError, AgentRuntimeEvent, AgentStep, Result, artifacts::GeneratedImageCandidate,
};
use jaco_core::*;
use jaco_db::{
    AppendedConversationEntryBatch, ConversationCommit, ConversationEntryRecord, NewAttachment,
    NewConversationEntry, NewConversationEntryBatchItem, ToolInvocationRecord,
    UpdateToolInvocationStatus,
};
use rig::completion::AssistantContent;

fn generated_invalid_error() -> RunErrorPayload {
    RunErrorPayload {
        code: "generated_artifact_invalid".to_string(),
        message: "generated image response was invalid".to_string(),
        retryable: false,
        provider: Some("openrouter".to_string()),
        raw: None,
    }
}

fn generated_persistence_error() -> RunErrorPayload {
    RunErrorPayload {
        code: "generated_artifact_persistence_failed".to_string(),
        message: "generated image persistence failed".to_string(),
        retryable: true,
        provider: Some("openrouter".to_string()),
        raw: None,
    }
}

impl PersistenceContext {
    pub(crate) async fn persist_generated_completion(
        &self,
        provider_step_id: &str,
        choice: &[AssistantContent],
    ) -> Result<()> {
        let image_count = choice
            .iter()
            .filter(|content| matches!(content, AssistantContent::Image(_)))
            .count();
        let has_tool_call = choice
            .iter()
            .any(|content| matches!(content, AssistantContent::ToolCall(_)));
        if image_count > 0 && has_tool_call {
            let payload = generated_invalid_error();
            self.set_pending_run_failure(payload);
            self.persist_generated_fallback(provider_step_id, choice)
                .await?;
            return Err(AgentRuntimeError::Invariant(
                "generated image response mixed images and tool calls".to_string(),
            ));
        }

        let mut candidates = Vec::with_capacity(image_count);
        for content in choice {
            if let AssistantContent::Image(image) = content {
                let ordinal = candidates.len() + 1;
                match GeneratedImageCandidate::from_rig_image(ordinal, image) {
                    Ok(candidate) => candidates.push(candidate),
                    Err(error) => {
                        self.set_pending_run_failure(error.run_error());
                        self.persist_generated_fallback(provider_step_id, choice)
                            .await?;
                        return Err(AgentRuntimeError::Invariant(
                            "generated image response was invalid".to_string(),
                        ));
                    }
                }
            }
        }

        if candidates.is_empty() {
            let batch = self.generated_batch(provider_step_id, choice, &[])?;
            if !batch.is_empty()
                && let Err(error) = self.commit_generated_batch(provider_step_id, batch).await
            {
                self.set_pending_run_failure(generated_persistence_error());
                return Err(error);
            }
            return Ok(());
        }

        let store = self.artifact_store().ok_or_else(|| {
            AgentRuntimeError::Invariant(
                "generated image run has no managed artifact store".to_string(),
            )
        })?;
        let mut prepared = match store
            .prepare(candidates, &self.provider_id, &self.cancellation_token)
            .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                if self.cancellation_token.is_cancelled() {
                    return Err(AgentRuntimeError::Canceled);
                }
                self.set_pending_run_failure(error.run_error());
                self.persist_generated_fallback(provider_step_id, choice)
                    .await?;
                return Err(AgentRuntimeError::Invariant(
                    "generated artifact processing failed".to_string(),
                ));
            }
        };
        if self.cancellation_token.is_cancelled() {
            prepared.rollback().await;
            return Err(AgentRuntimeError::Canceled);
        }

        let attachments = prepared
            .images
            .iter()
            .map(|image| image.attachment.clone())
            .collect::<Vec<_>>();
        let batch = self.generated_batch(provider_step_id, choice, &attachments)?;
        match self.commit_generated_batch(provider_step_id, batch).await {
            Ok(()) => {
                prepared.disarm();
                Ok(())
            }
            Err(error) => {
                self.set_pending_run_failure(generated_persistence_error());
                let ids = prepared
                    .images
                    .iter()
                    .map(|image| image.attachment.id.clone())
                    .collect::<std::collections::HashSet<_>>();
                match self
                    .persistence
                    .conversation_timeline(self.conversation_id.clone())
                    .await
                {
                    Ok(Some(timeline)) => {
                        let present = timeline
                            .attachments
                            .into_iter()
                            .filter(|attachment| ids.contains(&attachment.id))
                            .map(|attachment| attachment.id)
                            .collect::<std::collections::HashSet<_>>();
                        if present.is_empty() {
                            prepared.rollback().await;
                            let _ = self
                                .persist_generated_fallback(provider_step_id, choice)
                                .await;
                        } else {
                            prepared.preserve_only(&present).await;
                        }
                    }
                    Ok(None) => {
                        prepared.rollback().await;
                        let _ = self
                            .persist_generated_fallback(provider_step_id, choice)
                            .await;
                    }
                    Err(_) => prepared.disarm(),
                }
                Err(error)
            }
        }
    }

    async fn persist_generated_fallback(
        &self,
        provider_step_id: &str,
        choice: &[AssistantContent],
    ) -> Result<()> {
        let batch = self.generated_batch(provider_step_id, choice, &[])?;
        if batch.is_empty() {
            return Ok(());
        }
        self.commit_generated_batch(provider_step_id, batch).await
    }

    fn generated_batch(
        &self,
        provider_step_id: &str,
        choice: &[AssistantContent],
        attachments: &[NewAttachment],
    ) -> Result<Vec<NewConversationEntryBatchItem>> {
        let mut batch = Vec::new();
        let mut content = Vec::new();
        let mut message_attachments = Vec::new();
        let mut attachments = attachments.iter();

        let flush_message =
            |batch: &mut Vec<NewConversationEntryBatchItem>,
             content: &mut Vec<ContentPart>,
             message_attachments: &mut Vec<NewAttachment>| {
                if content.is_empty() {
                    return;
                }
                batch.push(NewConversationEntryBatchItem {
                    entry: NewConversationEntry {
                        conversation_id: self.conversation_id.clone(),
                        status: ConversationEntryStatus::Completed,
                        agent_run_id: Some(self.agent_run_id.clone()),
                        provider_step_id: Some(provider_step_id.to_string()),
                        tool_invocation_id: None,
                        provider_item_id: None,
                        payload: ConversationEntryPayload::Message {
                            role: TranscriptRole::Assistant,
                            content: std::mem::take(content),
                        },
                    },
                    attachments: std::mem::take(message_attachments),
                });
            };

        for item in choice {
            match item {
                AssistantContent::Text(text) if !text.text.is_empty() => {
                    content.push(ContentPart::Text {
                        text: text.text.clone(),
                    });
                }
                AssistantContent::Image(_) => {
                    if let Some(attachment) = attachments.next() {
                        content.push(ContentPart::Image {
                            attachment_id: attachment.id.clone(),
                        });
                        message_attachments.push(attachment.clone());
                    }
                }
                AssistantContent::Reasoning(reasoning) => {
                    flush_message(&mut batch, &mut content, &mut message_attachments);
                    batch.push(NewConversationEntryBatchItem {
                        entry: NewConversationEntry {
                            conversation_id: self.conversation_id.clone(),
                            status: ConversationEntryStatus::Completed,
                            agent_run_id: Some(self.agent_run_id.clone()),
                            provider_step_id: Some(provider_step_id.to_string()),
                            tool_invocation_id: None,
                            provider_item_id: reasoning.id.clone(),
                            payload: ConversationEntryPayload::Reasoning {
                                text: reasoning.display_text(),
                                summary: None,
                            },
                        },
                        attachments: Vec::new(),
                    });
                }
                AssistantContent::ToolCall(_) => {
                    flush_message(&mut batch, &mut content, &mut message_attachments);
                }
                AssistantContent::Text(_) => {}
            }
        }
        flush_message(&mut batch, &mut content, &mut message_attachments);
        if attachments.next().is_some() {
            return Err(AgentRuntimeError::Invariant(
                "generated attachment projection did not consume every attachment".to_string(),
            ));
        }
        Ok(batch)
    }

    async fn commit_generated_batch(
        &self,
        provider_step_id: &str,
        batch: Vec<NewConversationEntryBatchItem>,
    ) -> Result<()> {
        let commit = self
            .persistence
            .append_conversation_entries_with_attachments(batch)
            .await?;
        self.publish_generated_commit(provider_step_id, &commit);
        Ok(())
    }

    fn publish_generated_commit(
        &self,
        provider_step_id: &str,
        commit: &ConversationCommit<AppendedConversationEntryBatch>,
    ) {
        let attachments = commit
            .value
            .attachments
            .iter()
            .map(|attachment| (attachment.id.as_str(), attachment))
            .collect::<std::collections::HashMap<_, _>>();
        let mut changes = Vec::new();
        for entry in &commit.value.entries {
            if let ConversationEntryPayload::Message { content, .. } = &entry.payload {
                for part in content {
                    if let ContentPart::Image { attachment_id } = part
                        && let Some(attachment) = attachments.get(attachment_id.as_str())
                    {
                        changes.push(ConversationChange::AttachmentUpserted {
                            attachment: Box::new((*attachment).clone()),
                        });
                    }
                }
            }
            changes.push(ConversationChange::EntryAppended {
                entry: Box::new(entry.clone()),
            });
        }
        self.emit_conversation_commit_with_changes(commit, changes);
        self.record_persisted_entries(&commit.value.entries);
        for entry in &commit.value.entries {
            self.push_event(AgentRunEvent::ProviderStepEvent {
                provider_step_id: provider_step_id.to_string(),
                event: ProviderStepEvent::OutputItemCompleted {
                    provider_item_id: entry.provider_item_id.clone(),
                    item: entry.payload.clone(),
                },
            });
        }
        if let Some(entry) = commit.value.entries.iter().rev().find(|entry| {
            matches!(
                entry.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        }) {
            self.set_final_entry_id(Some(entry.id.clone()));
        }
    }
    pub(crate) fn record_persisted_entries(&self, entries: &[ConversationEntryRecord]) {
        for entry in entries {
            self.add_input_item_id(entry.id.clone());
            self.push_step(AgentStep::ConversationEntry(entry.id.clone()));
        }
    }

    pub(super) async fn append_entries_and_update_tool_invocation_full(
        &self,
        entries: Vec<NewConversationEntry>,
        invocation: &ToolInvocationRecord,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> Result<(Vec<ConversationEntryRecord>, ToolInvocationRecord)> {
        let commit = self
            .persistence
            .append_entries_and_update_tool_invocation(
                entries,
                invocation.id.clone(),
                update,
                approval,
            )
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            commit
                .value
                .0
                .iter()
                .cloned()
                .map(|entry| jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(entry),
                })
                .chain(std::iter::once(
                    jaco_core::ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(commit.value.1.clone()),
                    },
                ))
                .collect(),
        );
        let (entries, invocation) = commit.value;
        self.record_persisted_entries(&entries);
        Ok((entries, invocation))
    }

    pub(super) async fn append_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: None,
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(crate) async fn append_running_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Running,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: None,
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(crate) async fn update_item_payload(
        &self,
        item_id: &str,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .update_conversation_entry_payload(item_id.to_string(), status, payload)
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryUpdated {
                entry: Box::new(commit.value.clone()),
                kind: EntryChangeKind::TextAppended,
            }],
        );
        Ok(commit.value)
    }

    pub(crate) fn set_final_entry_id(&self, item_id: Option<ConversationEntryId>) {
        mutex_replace(&self.final_entry_id, item_id);
    }

    pub(crate) fn current_provider_step_id(&self) -> Option<ProviderStepId> {
        mutex_clone(&self.last_provider_step_id)
    }

    pub(crate) fn push_current_provider_step_event(&self, event: ProviderStepEvent) {
        let Some(provider_step_id) = self.current_provider_step_id() else {
            return;
        };
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id,
            event,
        });
    }

    pub(super) async fn append_tool_item(
        &self,
        tool_invocation_id: ToolInvocationId,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: Some(tool_invocation_id),
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(super) fn add_input_item_id(&self, item_id: ConversationEntryId) {
        let mut guard = lock(&self.input_item_ids);
        guard.push(item_id);
    }

    pub(super) fn push_event(&self, event: AgentRunEvent) {
        lock(&self.events).push(event);
    }

    pub(crate) fn push_step(&self, step: AgentStep) {
        lock(&self.steps).push(step);
    }

    pub(super) fn emit_runtime(&self, event: AgentRuntimeEvent) {
        if let Some(observer) = &self.observer {
            observer.emit(event);
        }
    }

    pub(super) fn emit_conversation_timeline_changes(
        &self,
        changes: Vec<jaco_core::ConversationChange>,
    ) {
        if changes.is_empty() {
            return;
        }
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes,
        });
    }

    pub(super) fn emit_conversation_commit_with_changes<T>(
        &self,
        commit: &jaco_db::ConversationCommit<T>,
        changes: Vec<jaco_core::ConversationChange>,
    ) {
        self.emit_runtime(AgentRuntimeEvent::ConversationCommitted {
            conversation: Box::new(commit.conversation.clone()),
            changes,
        });
    }

    pub(super) fn emit_tool_entries_commit(
        &self,
        commit: &jaco_db::ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>,
    ) {
        self.emit_conversation_commit_with_changes(
            commit,
            commit
                .value
                .0
                .iter()
                .cloned()
                .map(|entry| jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(entry),
                })
                .chain(std::iter::once(
                    jaco_core::ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(commit.value.1.clone()),
                    },
                ))
                .collect(),
        );
    }

    pub(super) fn emit_tool_entry_commit(
        &self,
        commit: &jaco_db::ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>,
    ) {
        self.emit_conversation_commit_with_changes(
            commit,
            vec![
                jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(commit.value.0.clone()),
                },
                jaco_core::ConversationChange::ToolInvocationChanged {
                    invocation: Box::new(commit.value.1.clone()),
                },
            ],
        );
    }
}
