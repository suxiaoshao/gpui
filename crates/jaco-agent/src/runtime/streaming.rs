use crate::{Result, persistence::PersistenceContext};
use jaco_core::*;
use std::time::{Duration, Instant};

const STREAM_FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const STREAM_FLUSH_CHARS: usize = 24;

pub(super) struct StreamingOutputAccumulator {
    context: PersistenceContext,
    text: StreamingItemAccumulator,
    reasoning: StreamingItemAccumulator,
}

impl StreamingOutputAccumulator {
    pub(super) fn new(context: PersistenceContext) -> Self {
        Self {
            context,
            text: StreamingItemAccumulator::new(StreamingOutputKind::Text),
            reasoning: StreamingItemAccumulator::new(StreamingOutputKind::Reasoning),
        }
    }

    pub(super) fn append_text(&mut self, delta: &str) -> Result<()> {
        self.text.append(&self.context, delta)
    }

    pub(super) fn append_reasoning(&mut self, delta: &str) -> Result<()> {
        self.reasoning.append(&self.context, delta)
    }

    pub(super) fn replace_reasoning(&mut self, text: String) -> Result<()> {
        self.reasoning.replace(&self.context, text)
    }

    pub(super) fn finish(
        &mut self,
        status: ConversationEntryStatus,
        final_text: Option<&str>,
    ) -> Result<()> {
        if let Some(final_text) = final_text {
            self.text.replace_final_content(final_text.to_string())?;
        }
        self.text.finish(&self.context, status)?;
        self.reasoning.finish(&self.context, status)?;
        Ok(())
    }
}

struct StreamingItemAccumulator {
    kind: StreamingOutputKind,
    item_id: Option<ConversationEntryId>,
    content: String,
    pending_delta: String,
    pending_chars: usize,
    last_flush_at: Option<Instant>,
}

impl StreamingItemAccumulator {
    fn new(kind: StreamingOutputKind) -> Self {
        Self {
            kind,
            item_id: None,
            content: String::new(),
            pending_delta: String::new(),
            pending_chars: 0,
            last_flush_at: None,
        }
    }

    fn append(&mut self, context: &PersistenceContext, delta: &str) -> Result<()> {
        if delta.is_empty() {
            return Ok(());
        }
        self.content.push_str(delta);
        if self.item_id.is_none() {
            self.start(context)?;
            return Ok(());
        }

        self.pending_delta.push_str(delta);
        self.pending_chars += delta.chars().count();
        if self.should_flush() {
            self.flush(context, ConversationEntryStatus::Running, false)?;
        }
        Ok(())
    }

    fn replace(&mut self, context: &PersistenceContext, content: String) -> Result<()> {
        self.content = content;
        self.pending_delta.clear();
        self.pending_chars = 0;
        if self.content.is_empty() {
            return Ok(());
        }
        if self.item_id.is_none() {
            self.start(context)?;
        } else {
            self.flush(context, ConversationEntryStatus::Running, true)?;
        }
        Ok(())
    }

    fn replace_final_content(&mut self, content: String) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }
        if self.content != content {
            self.content = content;
            self.pending_delta.clear();
            self.pending_chars = 0;
        }
        Ok(())
    }

    fn finish(
        &mut self,
        context: &PersistenceContext,
        status: ConversationEntryStatus,
    ) -> Result<()> {
        if self.item_id.is_none() {
            if self.content.is_empty() {
                return Ok(());
            }
            self.start(context)?;
        }
        self.flush(context, status, true)?;
        if matches!(
            status,
            ConversationEntryStatus::Completed | ConversationEntryStatus::Canceled
        ) && self.kind == StreamingOutputKind::Text
        {
            context.set_final_entry_id(self.item_id.clone());
        }
        context.push_current_provider_step_event(ProviderStepEvent::OutputItemCompleted {
            provider_item_id: None,
            item: self.payload(),
        });
        Ok(())
    }

    fn start(&mut self, context: &PersistenceContext) -> Result<()> {
        let payload = self.payload();
        let item = context.append_running_item(payload.clone())?;
        self.item_id = Some(item.id);
        self.pending_delta.clear();
        self.pending_chars = 0;
        self.last_flush_at = Some(Instant::now());
        context.push_current_provider_step_event(ProviderStepEvent::OutputItemStarted {
            provider_item_id: None,
            item: payload,
        });
        Ok(())
    }

    fn should_flush(&self) -> bool {
        self.pending_chars >= STREAM_FLUSH_CHARS
            || self
                .last_flush_at
                .is_some_and(|last_flush_at| last_flush_at.elapsed() >= STREAM_FLUSH_INTERVAL)
    }

    fn flush(
        &mut self,
        context: &PersistenceContext,
        status: ConversationEntryStatus,
        force: bool,
    ) -> Result<()> {
        let Some(item_id) = self.item_id.as_deref() else {
            return Ok(());
        };
        if !force && self.pending_delta.is_empty() {
            return Ok(());
        }

        let delta = std::mem::take(&mut self.pending_delta);
        self.pending_chars = 0;
        context.update_item_payload(item_id, status, self.payload())?;
        if !delta.is_empty() {
            let event = match self.kind {
                StreamingOutputKind::Text => ProviderStepEvent::TextDelta {
                    provider_item_id: None,
                    text: delta,
                },
                StreamingOutputKind::Reasoning => ProviderStepEvent::ReasoningDelta {
                    provider_item_id: None,
                    text: delta,
                },
            };
            context.push_current_provider_step_event(event);
        }
        self.last_flush_at = Some(Instant::now());
        Ok(())
    }

    fn payload(&self) -> ConversationEntryPayload {
        match self.kind {
            StreamingOutputKind::Text => ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: self.content.clone(),
                }],
            },
            StreamingOutputKind::Reasoning => ConversationEntryPayload::Reasoning {
                text: self.content.clone(),
                summary: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamingOutputKind {
    Text,
    Reasoning,
}
