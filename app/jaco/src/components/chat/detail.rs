mod attachments;
mod copy_button;
mod message;
mod request_usage;
mod timeline;
mod tool_blocks;
mod tool_invocation;

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::Path,
    rc::Rc,
    sync::Arc,
};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::Button,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    spinner::Spinner,
    text::TextViewState,
    v_flex,
};
use jaco_core::{
    AgentRunId, ConversationEffect, ConversationEntryId, ConversationId, ToolInvocationId,
};

use crate::{
    components::chat::form::{AgentRunControlStatus, AgentRunStatusSource},
    components::chat::input::{
        ChatFormSkillCompletionPlacement, ChatInput, ChatInputController, ChatInputEvent,
        ChatInputSubmit,
    },
    components::chat::runtime_status::ConversationRuntimeStatus,
    features::conversation,
    foundation::{I18n, conversation_format as format},
};
use conversation::model::{ConversationModel, ConversationModelEvent, ConversationOperation};

pub(crate) struct ConversationDetailPage {
    conversation_id: ConversationId,
    conversation: Entity<ConversationModel>,
    chat_form: Entity<ChatInputController>,
    timeline: ListState,
    timeline_rows: timeline::ConversationTimelineRows,
    message_text_states: Vec<MessageTextState>,
    expanded_agent_runs: HashMap<AgentRunId, bool>,
    expanded_tool_invocations: HashMap<ToolInvocationId, bool>,
    tool_invocation_previews:
        HashMap<ToolInvocationId, tool_invocation::ToolInvocationPreviewCacheEntry>,
    runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
    pending_submission: Option<conversation::runtime::ConversationSubmissionTicket>,
    owned_run: Option<conversation::runtime::ConversationSubmissionTicket>,
    #[cfg(test)]
    last_tool_invocation_remeasure: Option<Range<usize>>,
    _subscriptions: Vec<Subscription>,
}

struct MessageTextState {
    id: ConversationEntryId,
    state: Entity<TextViewState>,
    source: String,
    _subscription: Subscription,
}

struct ConversationAgentRunStatus {
    runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
    conversation_id: ConversationId,
}

impl AgentRunStatusSource for ConversationAgentRunStatus {
    fn status(&self, cx: &App) -> AgentRunControlStatus {
        match self.runtime.read(cx).run_status(&self.conversation_id) {
            conversation::runtime::ConversationRunStatus::Idle => AgentRunControlStatus::Idle,
            conversation::runtime::ConversationRunStatus::Submitting => {
                AgentRunControlStatus::Submitting
            }
            conversation::runtime::ConversationRunStatus::Running => AgentRunControlStatus::Running,
            conversation::runtime::ConversationRunStatus::Stopping => {
                AgentRunControlStatus::Stopping
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MessageTextUpdate<'a> {
    Unchanged,
    Append(&'a str),
    Replace,
}

fn message_text_update<'a>(previous: &str, next: &'a str) -> MessageTextUpdate<'a> {
    if previous == next {
        return MessageTextUpdate::Unchanged;
    }

    if let Some(delta) = next.strip_prefix(previous)
        && !delta.is_empty()
    {
        return MessageTextUpdate::Append(delta);
    }

    MessageTextUpdate::Replace
}

impl ConversationDetailPage {
    pub(crate) fn new(
        conversation: Entity<ConversationModel>,
        runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_focus(conversation, runtime, true, window, cx)
    }

    pub(crate) fn new_without_focus(
        conversation: Entity<ConversationModel>,
        runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_focus(conversation, runtime, false, window, cx)
    }

    fn new_with_focus(
        conversation: Entity<ConversationModel>,
        runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
        focus_composer: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let conversation_id = conversation.read(cx).id().clone();
        let run_status = Rc::new(ConversationAgentRunStatus {
            runtime: runtime.clone(),
            conversation_id: conversation_id.clone(),
        });
        let chat_form = cx.new(|cx| {
            let mut chat_form = if focus_composer {
                ChatInputController::new(window, cx)
            } else {
                ChatInputController::new_without_focus(window, cx)
            };
            chat_form.set_agent_run_status(run_status, cx);
            chat_form
        });
        let timeline = ListState::new(0, ListAlignment::Top, px(2048.)).measure_all();
        let timeline_rows = timeline::ConversationTimelineRows::new(Vec::new());
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |page, _chat_form, event: &ChatInputEvent, window, cx| match event {
                ChatInputEvent::SendRequested(submit) => {
                    page.submit_message((**submit).clone(), window, cx);
                }
                ChatInputEvent::StopRequested => {
                    page.stop_agent_run(cx);
                }
                ChatInputEvent::AddRequested | ChatInputEvent::AddProjectRequested => {}
            },
        );
        let runtime_subscription = cx.subscribe_in(
            &runtime,
            window,
            |page, runtime, event: &conversation::runtime::ConversationRuntimeEvent, window, cx| {
                page.handle_runtime_event(runtime, event, window, cx);
            },
        );
        let runtime_observation = cx.observe(&runtime, |page, _, cx| {
            page.sync_submission_problem(cx);
            cx.notify();
        });

        let model_event_subscription = cx.subscribe(
            &conversation,
            |page, _model, event: &ConversationModelEvent, cx| {
                page.handle_conversation_model_event(event, cx);
            },
        );
        let model_observation = cx.observe(&conversation, |page, _, cx| {
            page.sync_submission_problem(cx);
            cx.notify();
        });
        let mut page = Self {
            conversation_id,
            conversation,
            chat_form,
            timeline,
            timeline_rows,
            message_text_states: Vec::new(),
            expanded_agent_runs: HashMap::new(),
            expanded_tool_invocations: HashMap::new(),
            tool_invocation_previews: HashMap::new(),
            runtime,
            pending_submission: None,
            owned_run: None,
            #[cfg(test)]
            last_tool_invocation_remeasure: None,
            _subscriptions: vec![
                chat_form_subscription,
                runtime_subscription,
                runtime_observation,
                model_event_subscription,
                model_observation,
            ],
        };
        page.refresh_chat_form_context(cx);
        page.sync_message_text_states(cx);
        page.sync_timeline(cx, None);
        page.timeline.scroll_to_end();
        page.sync_submission_problem(cx);
        page
    }

    pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.focus_composer(window, cx));
    }

    fn submit_message(
        &mut self,
        submit: ChatInputSubmit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project_id) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|conversation| conversation.summary.project_id.clone())
        else {
            return;
        };
        let request = conversation::SendConversationMessageRequest {
            conversation_id: self.conversation_id.clone(),
            project_id,
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.attachments.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
        };
        match self
            .runtime
            .update(cx, |runtime, cx| runtime.submit_message(request, cx))
        {
            Ok(ticket) => self.pending_submission = Some(ticket),
            Err(conversation::runtime::ConversationSubmissionError::Busy) => {}
            Err(conversation::runtime::ConversationSubmissionError::Unavailable(error)) => {
                let title = cx.global::<I18n>().t("conversation-send-failed");
                push_conversation_notification(window, cx, title, error, NotificationType::Error);
            }
        }
    }

    fn handle_runtime_event(
        &mut self,
        runtime: &Entity<conversation::runtime::ConversationRuntimeStore>,
        event: &conversation::runtime::ConversationRuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let event_conversation_id = match event {
            conversation::runtime::ConversationRuntimeEvent::SubmissionCommitted {
                ticket, ..
            }
            | conversation::runtime::ConversationRuntimeEvent::SubmissionFailed {
                ticket, ..
            }
            | conversation::runtime::ConversationRuntimeEvent::RunLaunchFailed { ticket, .. } => {
                ticket.conversation_id()
            }
            conversation::runtime::ConversationRuntimeEvent::RunStarted { ticket }
            | conversation::runtime::ConversationRuntimeEvent::RunFinished { ticket } => {
                ticket.conversation_id()
            }
            conversation::runtime::ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                conversation_id,
                ..
            } => conversation_id,
        };
        if event_conversation_id != &self.conversation_id {
            return;
        }

        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_primary_action(cx);
        });
        cx.notify();
        match event {
            conversation::runtime::ConversationRuntimeEvent::SubmissionCommitted {
                ticket,
                kind: conversation::runtime::ConversationSubmissionKind::Message,
            } if self.pending_submission.as_ref() == Some(ticket) => {
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.clear_after_submit(window, cx);
                });
                self.timeline.set_follow_mode(FollowMode::Tail);
                self.timeline.scroll_to_end();
            }
            conversation::runtime::ConversationRuntimeEvent::SubmissionFailed {
                ticket,
                kind: conversation::runtime::ConversationSubmissionKind::Message,
                error,
            } if self.pending_submission.as_ref() == Some(ticket) => {
                self.pending_submission = None;
                let title = cx.global::<I18n>().t("conversation-send-failed");
                push_conversation_notification(
                    window,
                    cx,
                    title,
                    error.clone(),
                    NotificationType::Error,
                );
            }
            conversation::runtime::ConversationRuntimeEvent::RunLaunchFailed { ticket, error }
                if self.pending_submission.as_ref() == Some(ticket) =>
            {
                self.pending_submission = None;
                let title = cx.global::<I18n>().t("conversation-run-failed");
                push_conversation_notification(
                    window,
                    cx,
                    title,
                    error.clone(),
                    NotificationType::Error,
                );
            }
            conversation::runtime::ConversationRuntimeEvent::RunStarted { ticket }
                if self.pending_submission.as_ref() == Some(ticket) =>
            {
                self.owned_run = self.pending_submission.take();
            }
            conversation::runtime::ConversationRuntimeEvent::RunFinished { ticket }
                if take_matching_ticket(&mut self.owned_run, ticket) =>
            {
                runtime.update(cx, |runtime, cx| {
                    if let Some(error) = runtime.take_last_error(&self.conversation_id) {
                        let title = cx.global::<I18n>().t("conversation-run-failed");
                        push_conversation_notification(
                            window,
                            cx,
                            title,
                            error,
                            NotificationType::Error,
                        );
                    }
                });
            }
            _ => {}
        }

        match event {
            conversation::runtime::ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                agent_run_id,
                tool_invocation_id,
                ..
            } => self.update_tool_approval_availability(agent_run_id, tool_invocation_id, cx),
            conversation::runtime::ConversationRuntimeEvent::RunStarted { .. }
            | conversation::runtime::ConversationRuntimeEvent::RunFinished { .. } => {
                self.refresh_tool_approval_availability(cx);
            }
            conversation::runtime::ConversationRuntimeEvent::SubmissionCommitted { .. }
            | conversation::runtime::ConversationRuntimeEvent::SubmissionFailed { .. }
            | conversation::runtime::ConversationRuntimeEvent::RunLaunchFailed { .. } => {}
        }
    }

    fn handle_conversation_model_event(
        &mut self,
        event: &ConversationModelEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ConversationModelEvent::Reloaded => {
                self.refresh_chat_form_context(cx);
                self.sync_tool_invocation_ui_state(cx);
                self.sync_message_text_states(cx);
                self.sync_timeline(cx, None);
                // A reload can change the height of a row in the unchanged key prefix while
                // also adding or removing a later row. `splice` only invalidates the changed
                // suffix, so explicitly invalidate every retained row after the rebuild.
                self.timeline.remeasure();
            }
            ConversationModelEvent::Changed(effects) => {
                for effect in effects {
                    self.apply_conversation_effect(effect, cx);
                }
            }
        }
        self.sync_submission_problem(cx);
        cx.notify();
    }

    fn apply_conversation_effect(&mut self, effect: &ConversationEffect, cx: &mut Context<Self>) {
        match effect {
            ConversationEffect::SummaryChanged => {}
            ConversationEffect::EntryInserted { entry_id } => {
                self.sync_message_text_state(entry_id, cx);
                self.sync_timeline(cx, None);
            }
            ConversationEffect::EntryChanged { entry_id, .. } => {
                self.sync_message_text_state(entry_id, cx);
                self.update_timeline_entry(entry_id, cx);
            }
            ConversationEffect::AttachmentChanged { attachment_id } => {
                self.sync_attachment_rows(attachment_id, cx);
            }
            ConversationEffect::RunChanged { run_id } => self.update_timeline_run(run_id, cx),
            ConversationEffect::ProviderStepChanged { .. } => {}
            ConversationEffect::ToolInvocationChanged { tool_invocation_id } => {
                self.update_timeline_tool_invocation(tool_invocation_id, cx);
            }
            ConversationEffect::AgentMessageRequestUsageChanged { agent_run_id } => {
                self.update_timeline_request_usage(agent_run_id, cx);
            }
            ConversationEffect::ConversationContextRequestUsageChanged { .. } => {
                self.sync_chat_form_context_usage(cx);
            }
            ConversationEffect::Deleted => {
                self.message_text_states.clear();
                self.expanded_tool_invocations.clear();
                self.tool_invocation_previews.clear();
                self.sync_chat_form_context_usage(cx);
                self.sync_timeline(cx, None);
            }
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.conversation
            .update(cx, |conversation, cx| conversation.refresh(cx));
    }

    fn refresh_chat_form_context(&mut self, cx: &mut Context<Self>) {
        let (project_path, latest_context_request_usage) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                (
                    Some(snapshot.project.path.clone()),
                    snapshot.latest_context_request_usage.clone(),
                )
            })
            .unwrap_or((None, None));
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(project_path.as_deref().map(Path::new), cx);
            chat_form.set_latest_context_request_usage(latest_context_request_usage, cx);
        });
    }

    fn sync_chat_form_context_usage(&mut self, cx: &mut Context<Self>) {
        let latest_context_request_usage = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| snapshot.latest_context_request_usage.clone());
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.set_latest_context_request_usage(latest_context_request_usage, cx);
        });
    }

    fn sync_timeline(
        &mut self,
        cx: &mut Context<Self>,
        remeasure_hint: Option<message::TimelineRowKey>,
    ) {
        let page = cx.entity().downgrade();
        let callbacks = timeline::callbacks(
            {
                let page = page.clone();
                move |agent_run_id, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.toggle_agent_run(agent_run_id.clone(), window, cx);
                    });
                }
            },
            {
                let page = page.clone();
                move |tool_invocation_id, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.toggle_tool_invocation(tool_invocation_id.clone(), window, cx);
                    });
                }
            },
            copy_to_clipboard,
            {
                let page = page.clone();
                move |tool_invocation_id, approved, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.decide_tool_approval(tool_invocation_id.clone(), approved, window, cx);
                    });
                }
            },
        );
        let rows = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                let active_agent_run_id = self
                    .runtime
                    .read(cx)
                    .active_agent_run_id(&self.conversation_id);
                let approval_decidable = {
                    let runtime = self.runtime.read(cx);
                    snapshot
                        .tool_invocations
                        .iter()
                        .filter(|invocation| {
                            runtime.can_decide_tool_invocation(
                                &self.conversation_id,
                                &invocation.agent_run_id,
                                &invocation.id,
                            )
                        })
                        .map(|invocation| invocation.id.clone())
                        .collect::<HashSet<_>>()
                };
                timeline::build_rows(
                    snapshot,
                    active_agent_run_id.as_ref(),
                    &self.expanded_agent_runs,
                    &self.expanded_tool_invocations,
                    &self.tool_invocation_previews,
                    &approval_decidable,
                    &self.message_text_state_map(),
                    callbacks,
                )
            })
            .unwrap_or_default();
        let previous_keys = self.timeline_rows.set_rows(rows);
        sync_timeline_list(
            &self.timeline,
            &previous_keys,
            self.timeline_rows.keys(),
            remeasure_hint.as_ref(),
        );
    }

    fn sync_message_text_states(&mut self, cx: &mut Context<Self>) {
        let sources = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                snapshot
                    .entries
                    .iter()
                    .filter_map(|item| {
                        if tool_invocation::is_tool_lifecycle_entry(item) {
                            return None;
                        }
                        let source = format::item_markdown(item);
                        (!source.is_empty()).then(|| (item.id.clone(), source))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let next_ids = sources
            .iter()
            .map(|(item_id, _)| item_id.clone())
            .collect::<HashSet<_>>();

        for (item_id, source) in sources {
            self.ensure_message_text_state(item_id, &source, cx);
        }

        self.message_text_states
            .retain(|entry| next_ids.contains(&entry.id));
    }

    fn sync_message_text_state(&mut self, item_id: &ConversationEntryId, cx: &mut Context<Self>) {
        let source = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|conversation| {
                conversation
                    .entries
                    .iter()
                    .find(|entry| &entry.id == item_id)
            })
            .and_then(|entry| {
                (!tool_invocation::is_tool_lifecycle_entry(entry))
                    .then(|| format::item_markdown(entry))
            });
        match source {
            Some(source) if !source.is_empty() => {
                self.ensure_message_text_state(item_id.clone(), &source, cx);
            }
            Some(_) | None => {
                self.message_text_states
                    .retain(|state| &state.id != item_id);
            }
        }
    }

    fn update_timeline_entry(&mut self, item_id: &ConversationEntryId, cx: &mut Context<Self>) {
        let entry_update = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|conversation| {
                conversation
                    .entries
                    .iter()
                    .find(|entry| &entry.id == item_id)
                    .map(|entry| {
                        if tool_invocation::is_tool_lifecycle_entry(entry) {
                            None
                        } else {
                            Some((entry.clone(), conversation.attachments.clone()))
                        }
                    })
            });
        let (entry, attachments) = match entry_update {
            Some(Some(update)) => update,
            Some(None) | None => {
                self.sync_timeline(cx, None);
                return;
            }
        };
        let text_state = self
            .message_text_states
            .iter()
            .find(|state| &state.id == item_id)
            .map(|state| state.state.clone());
        let Some(key) = self
            .timeline_rows
            .update_entry(entry, &attachments, text_state)
        else {
            self.sync_timeline(cx, None);
            return;
        };
        self.remeasure_timeline_row(&key);
    }

    fn update_timeline_run(&mut self, run_id: &AgentRunId, cx: &mut Context<Self>) {
        let Some(run) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|conversation| {
                conversation
                    .runs
                    .iter()
                    .find(|run| &run.id == run_id)
                    .cloned()
            })
        else {
            self.sync_timeline(cx, None);
            return;
        };
        let Some(key) = self.timeline_rows.update_run(run) else {
            self.sync_timeline(cx, None);
            return;
        };
        self.remeasure_timeline_row(&key);
    }

    fn update_timeline_request_usage(&mut self, agent_run_id: &AgentRunId, cx: &mut Context<Self>) {
        let request_usage = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|conversation| {
                conversation
                    .agent_message_request_usages
                    .iter()
                    .find(|request_usage| &request_usage.agent_run_id == agent_run_id)
                    .cloned()
            });
        let Some(request_usage) = request_usage else {
            self.sync_timeline(cx, None);
            return;
        };
        let Some(key) = self
            .timeline_rows
            .update_agent_request_usage(agent_run_id, request_usage)
        else {
            self.sync_timeline(cx, None);
            return;
        };
        self.remeasure_timeline_row(&key);
    }

    fn sync_attachment_rows(
        &mut self,
        attachment_id: &jaco_core::AttachmentId,
        cx: &mut Context<Self>,
    ) {
        let item_ids = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|conversation| {
                conversation
                    .entries
                    .iter()
                    .filter(|entry| entry_references_attachment(entry, attachment_id))
                    .map(|entry| entry.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for item_id in item_ids {
            self.update_timeline_entry(&item_id, cx);
        }
    }

    fn remeasure_timeline_row(&mut self, key: &message::TimelineRowKey) {
        if let Some(range) = timeline_row_remeasure_range(self.timeline_rows.keys(), key) {
            #[cfg(test)]
            {
                self.last_tool_invocation_remeasure = Some(range.clone());
            }
            self.timeline.remeasure_items(range);
        }
    }

    fn ensure_message_text_state(
        &mut self,
        item_id: ConversationEntryId,
        source: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self
            .message_text_states
            .iter_mut()
            .find(|entry| entry.id == item_id)
        {
            match message_text_update(&entry.source, source) {
                MessageTextUpdate::Unchanged => {}
                MessageTextUpdate::Append(delta) => {
                    let delta = delta.to_owned();
                    entry
                        .state
                        .update(cx, |state, cx| state.push_str(&delta, cx));
                    entry.source.clear();
                    entry.source.push_str(source);
                }
                MessageTextUpdate::Replace => {
                    entry
                        .state
                        .update(cx, |state, cx| state.set_text(source, cx));
                    entry.source.clear();
                    entry.source.push_str(source);
                }
            }
            return;
        }

        let state = cx.new(|cx| TextViewState::markdown(source, cx));
        let observed_item_id = item_id.clone();
        let subscription = cx.observe(&state, move |page, _, cx| {
            if let Some(row_ix) = page.timeline_rows.row_index_for_item(&observed_item_id) {
                page.timeline.remeasure_items(row_ix..row_ix + 1);
                cx.notify();
            }
        });

        self.message_text_states.push(MessageTextState {
            id: item_id,
            state,
            source: source.to_owned(),
            _subscription: subscription,
        });
    }

    fn message_text_state_map(&self) -> HashMap<ConversationEntryId, Entity<TextViewState>> {
        self.message_text_states
            .iter()
            .map(|entry| (entry.id.clone(), entry.state.clone()))
            .collect()
    }

    fn sync_submission_problem(&mut self, cx: &mut Context<Self>) {
        let conversation_problem = {
            let operation = self.conversation.read(cx).operation();
            (!matches!(
                operation,
                ConversationOperation::Ready(ready) if ready.data().is_some()
            ))
            .then(|| {
                operation
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        cx.global::<I18n>().t(if operation.data().is_some() {
                            "resource-status-stale"
                        } else {
                            "resource-status-loading"
                        })
                    })
            })
        };
        let runtime_problem = {
            let runtime = self.runtime.read(cx);
            let operation = runtime.recovery();
            (!matches!(operation, gpui_operation::refresh::Operation::Ready(_))).then(|| {
                operation
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| cx.global::<I18n>().t("conversation-runtime-recovering"))
            })
        };
        let problem = conversation_problem.or(runtime_problem).map(Into::into);
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.set_submission_problem(problem, cx);
        });
    }

    fn stop_agent_run(&mut self, cx: &mut Context<Self>) {
        self.runtime.update(cx, |runtime, cx| {
            runtime.stop_run(&self.conversation_id, cx);
        });
    }

    fn decide_tool_approval(
        &mut self,
        tool_invocation_id: ToolInvocationId,
        approved: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.runtime.update(cx, |runtime, cx| {
            if approved {
                runtime.approve_tool_invocation(
                    self.conversation_id.clone(),
                    tool_invocation_id,
                    window,
                    cx,
                );
            } else {
                runtime.deny_tool_invocation(self.conversation_id.clone(), tool_invocation_id, cx);
            }
        });
    }

    fn toggle_tool_invocation(
        &mut self,
        id: ToolInvocationId,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let expanded = !self
            .expanded_tool_invocations
            .get(&id)
            .copied()
            .unwrap_or(false);
        let row_key = self.timeline_rows.row_key_for_tool_invocation(&id);
        self.timeline.set_follow_mode(FollowMode::Normal);
        self.expanded_tool_invocations.insert(id.clone(), expanded);
        if expanded {
            self.ensure_tool_invocation_preview(&id, cx);
        }
        self.sync_timeline(cx, row_key);
        cx.notify();
    }

    fn ensure_tool_invocation_preview(
        &mut self,
        id: &ToolInvocationId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(revision) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .find(|invocation| &invocation.id == id)
                    .map(|invocation| invocation.updated_at)
            })
        else {
            self.tool_invocation_previews.remove(id);
            return false;
        };

        if self
            .tool_invocation_previews
            .get(id)
            .is_some_and(|cached| cached.revision == revision)
        {
            return false;
        }

        let Some(preview) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .find(|invocation| &invocation.id == id)
                    .map(tool_invocation::build_tool_invocation_preview)
            })
            .map(Arc::new)
        else {
            self.tool_invocation_previews.remove(id);
            return false;
        };
        self.tool_invocation_previews.insert(
            id.clone(),
            tool_invocation::ToolInvocationPreviewCacheEntry { revision, preview },
        );
        true
    }

    fn update_timeline_tool_invocation(&mut self, id: &ToolInvocationId, cx: &mut Context<Self>) {
        let Some((revision, agent_run_id)) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .find(|invocation| &invocation.id == id)
                    .map(|invocation| (invocation.updated_at, invocation.agent_run_id.clone()))
            })
        else {
            self.expanded_tool_invocations.remove(id);
            self.tool_invocation_previews.remove(id);
            self.sync_timeline(cx, None);
            return;
        };

        if self
            .tool_invocation_previews
            .get(id)
            .is_some_and(|cached| cached.revision != revision)
        {
            self.tool_invocation_previews.remove(id);
        }
        let expanded = self
            .expanded_tool_invocations
            .get(id)
            .copied()
            .unwrap_or(false);
        if expanded {
            self.ensure_tool_invocation_preview(id, cx);
        }
        let preview = expanded.then(|| {
            self.tool_invocation_previews
                .get(id)
                .filter(|cached| cached.revision == revision)
                .map(|cached| cached.preview.clone())
        });
        let broker_decidable = self.runtime.read(cx).can_decide_tool_invocation(
            &self.conversation_id,
            &agent_run_id,
            id,
        );
        let Some(detail) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .find(|invocation| &invocation.id == id)
                    .map(|invocation| {
                        tool_invocation::project_tool_invocation_detail(
                            invocation,
                            expanded,
                            preview.flatten(),
                            broker_decidable,
                        )
                    })
            })
        else {
            self.sync_timeline(cx, None);
            return;
        };
        let Some(row_key) = self.timeline_rows.update_tool_invocation(detail) else {
            self.sync_timeline(cx, None);
            return;
        };
        self.remeasure_timeline_row(&row_key);
        cx.notify();
    }

    fn sync_tool_invocation_ui_state(&mut self, cx: &mut Context<Self>) {
        let current_ids = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .map(|invocation| invocation.id.clone())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let expanded_ids = reconcile_tool_invocation_ui_state(
            &mut self.expanded_tool_invocations,
            &mut self.tool_invocation_previews,
            &current_ids,
        );
        for id in expanded_ids {
            self.ensure_tool_invocation_preview(&id, cx);
        }
    }

    fn refresh_tool_approval_availability(&mut self, cx: &mut Context<Self>) {
        let invocation_ids = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                snapshot
                    .tool_invocations
                    .iter()
                    .map(|invocation| invocation.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for id in invocation_ids {
            self.update_timeline_tool_invocation(&id, cx);
        }
    }

    fn update_tool_approval_availability(
        &mut self,
        agent_run_id: &AgentRunId,
        id: &ToolInvocationId,
        cx: &mut Context<Self>,
    ) {
        let matches_run = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .is_some_and(|snapshot| {
                snapshot.tool_invocations.iter().any(|invocation| {
                    &invocation.id == id && &invocation.agent_run_id == agent_run_id
                })
            });
        if matches_run {
            self.update_timeline_tool_invocation(id, cx);
        } else {
            self.sync_timeline(cx, None);
        }
    }

    fn toggle_agent_run(
        &mut self,
        agent_run_id: AgentRunId,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .expanded_agent_runs
            .get(&agent_run_id)
            .copied()
            .unwrap_or_else(|| self.default_agent_run_expanded(&agent_run_id, cx));
        self.timeline.set_follow_mode(FollowMode::Normal);
        self.expanded_agent_runs
            .insert(agent_run_id.clone(), !current);
        self.sync_timeline(cx, Some(message::TimelineRowKey::Agent(agent_run_id)));
        cx.notify();
    }

    fn default_agent_run_expanded(
        &self,
        agent_run_id: &AgentRunId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(snapshot) = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
        else {
            return true;
        };
        let Some(run) = snapshot.runs.iter().find(|run| &run.id == agent_run_id) else {
            return true;
        };
        if !format::is_terminal_run(run) {
            return true;
        }
        false
    }

    fn render_missing(&self, cx: &mut Context<Self>) -> AnyElement {
        let operation = self.conversation.read(cx).operation();
        let (title, subtitle) = match operation.data() {
            None if operation.problem().is_some() => (
                cx.global::<I18n>().t("conversation-load-failed"),
                operation
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ),
            Some(None) => (
                cx.global::<I18n>().t("conversation-missing-title"),
                cx.global::<I18n>().t("conversation-missing-subtitle"),
            ),
            Some(Some(_)) => return div().into_any_element(),
            None => (
                cx.global::<I18n>().t("resource-status-loading"),
                String::new(),
            ),
        };

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .px_8()
            .py_12()
            .child(
                Label::new(title)
                    .text_size(px(24.))
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .truncate(),
            )
            .child(
                Label::new(subtitle)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .children(operation.is_running().then(|| Spinner::new().small()))
            .child(
                Button::new("conversation-retry-load")
                    .label(cx.global::<I18n>().t("resource-status-refresh"))
                    .disabled(operation.is_running())
                    .on_click(cx.listener(|page, _, _window, cx| {
                        page.reload(cx);
                    })),
            )
            .into_any_element()
    }

    fn render_stale_status(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let operation = self.conversation.read(cx).operation();
        let problem = operation.problem().map(ToString::to_string);
        let running = operation.is_running();
        (!matches!(operation, ConversationOperation::Ready(_))).then(|| {
            v_flex()
                .w_full()
                .gap_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary.opacity(0.45))
                .p_3()
                .child(
                    Label::new(
                        problem.unwrap_or_else(|| cx.global::<I18n>().t("resource-status-stale")),
                    )
                    .text_sm()
                    .text_color(cx.theme().warning),
                )
                .child(
                    Button::new("conversation-refresh")
                        .label(cx.global::<I18n>().t("resource-status-refresh"))
                        .disabled(running)
                        .loading(running)
                        .on_click(cx.listener(|page, _, _window, cx| {
                            page.reload(cx);
                        })),
                )
                .into_any_element()
        })
    }

    fn render_runtime_status(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        ConversationRuntimeStatus::from_runtime(&self.runtime, cx)
            .map(IntoElement::into_any_element)
    }
}

impl Render for ConversationDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .is_none()
        {
            return self.render_missing(cx);
        }

        let timeline = self.timeline.clone();
        let page = cx.entity().downgrade();

        v_flex()
            .id("jaco-conversation-page")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .bg(cx.theme().tokens.background.background)
            .children(self.render_stale_status(cx))
            .children(self.render_runtime_status(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .map(|this| {
                        if self.timeline_rows.is_empty() {
                            return this.child(
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .py_8()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        Label::new(cx.global::<I18n>().t("conversation-empty"))
                                            .text_sm(),
                                    ),
                            );
                        }

                        this.child(
                            list(timeline.clone(), move |ix, window, cx| {
                                page.upgrade()
                                    .and_then(|page| page.read(cx).timeline_rows.row(ix))
                                    .map(|row| {
                                        gpui::RenderOnce::render(row, window, cx).into_any_element()
                                    })
                                    .unwrap_or_else(|| div().into_any_element())
                            })
                            .size_full(),
                        )
                        .vertical_scrollbar(&timeline)
                    }),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(860.))
                            .mx_auto()
                            .child(ChatInput::new(
                                &self.chat_form,
                                ChatFormSkillCompletionPlacement::AboveForm,
                            )),
                    ),
            )
            .into_any_element()
    }
}

fn copy_to_clipboard(text: String, window: &mut Window, cx: &mut App) -> bool {
    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));

    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .is_some_and(|copied| copied == text);

    if !copied {
        let i18n = cx.global::<I18n>();
        push_conversation_notification(
            window,
            cx,
            i18n.t("conversation-copy-failed"),
            i18n.t("conversation-copy-failed-message"),
            NotificationType::Error,
        );
    }

    copied
}

fn push_conversation_notification(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    notification_type: NotificationType,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(notification_type),
        cx,
    );
}

fn entry_references_attachment(
    entry: &jaco_core::ConversationEntry,
    attachment_id: &jaco_core::AttachmentId,
) -> bool {
    let jaco_core::ConversationEntryPayload::Message { content, .. } = &entry.payload else {
        return false;
    };
    content.iter().any(|part| match part {
        jaco_core::ContentPart::Image {
            attachment_id: current,
        }
        | jaco_core::ContentPart::File {
            attachment_id: current,
        }
        | jaco_core::ContentPart::Audio {
            attachment_id: current,
        }
        | jaco_core::ContentPart::Attachment {
            attachment_id: current,
        } => current == attachment_id,
        jaco_core::ContentPart::Text { .. } => false,
    })
}

fn sync_timeline_list(
    list_state: &ListState,
    previous_keys: &[message::TimelineRowKey],
    next_keys: &[message::TimelineRowKey],
    remeasure_hint: Option<&message::TimelineRowKey>,
) {
    if previous_keys == next_keys {
        if let Some(row_ix) = remeasure_hint
            .and_then(|key| next_keys.iter().position(|current_key| current_key == key))
        {
            list_state.remeasure_items(row_ix..row_ix + 1);
        } else {
            list_state.remeasure();
        }
        return;
    }

    let first_diff = previous_keys
        .iter()
        .zip(next_keys.iter())
        .position(|(previous, next)| previous != next)
        .unwrap_or_else(|| previous_keys.len().min(next_keys.len()));

    list_state.splice(
        first_diff..previous_keys.len(),
        next_keys.len().saturating_sub(first_diff),
    );
}

fn timeline_row_remeasure_range(
    keys: &[message::TimelineRowKey],
    key: &message::TimelineRowKey,
) -> Option<Range<usize>> {
    keys.iter()
        .position(|current| current == key)
        .map(|row_ix| row_ix..row_ix + 1)
}

fn take_matching_ticket<T: PartialEq>(owned: &mut Option<T>, candidate: &T) -> bool {
    if owned.as_ref() != Some(candidate) {
        return false;
    }
    owned.take();
    true
}

fn reconcile_tool_invocation_ui_state<T>(
    expanded: &mut HashMap<ToolInvocationId, bool>,
    previews: &mut HashMap<ToolInvocationId, T>,
    current_ids: &HashSet<ToolInvocationId>,
) -> Vec<ToolInvocationId> {
    expanded.retain(|id, _| current_ids.contains(id));
    previews.clear();
    expanded
        .iter()
        .filter_map(|(id, is_expanded)| is_expanded.then_some(id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        path::Path,
        sync::Arc,
    };

    use super::{
        ConversationDetailPage, ConversationModel, ConversationOperation, MessageTextUpdate,
        conversation,
        message::{TimelineRow, TimelineRowKey},
        message_text_update, reconcile_tool_invocation_ui_state, take_matching_ticket,
        timeline_row_remeasure_range,
        tool_invocation::{
            AgentDetailItem, ToolInvocationDetail, ToolInvocationPreview,
            tool_inspection_evidence_for_test,
        },
    };
    use crate::database;
    use gpui::{AppContext as _, TestAppContext};
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunStatus, AgentRunTriggerKind, AgentRuntimeSnapshot,
        AgentStoppedReason, ApprovalRequestPayload, ContentPart, ConversationChange,
        ConversationChanges, ConversationEntryPayload, ConversationEntryStatus,
        ConversationMetadata, ConversationSettingsSnapshot, ConversationStatusCode,
        ConversationStatusEntry, ProjectKind, ProjectMetadata, ProviderRawPayload,
        ProviderSettingsPayload, RunSettingsSnapshot, StructuredOutput, ToolAccessKind,
        ToolAccessRequestPayload, ToolApprovalMode, ToolApprovalPolicy, ToolArguments,
        ToolExecutionPolicy, ToolInvocationId, ToolInvocationInput, ToolInvocationOutput,
        ToolInvocationStatus, ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use jaco_db::{
        AgentRunFinalEntry, FinishAgentRun, FreshStore, NewAgentRun, NewConversation,
        NewConversationEntry, NewProject, NewToolInvocation, NewToolInvocationApproval,
    };

    struct ReloadToolFixture {
        conversation_id: String,
        agent_run_id: String,
        expanded_invocation_id: ToolInvocationId,
        collapsed_invocation_id: ToolInvocationId,
        inspectable_invocation_id: ToolInvocationId,
        lifecycle_entry_ids: Vec<String>,
    }

    #[gpui::test]
    fn db_reopen_and_real_reload_rebuilds_tool_invocation_page_state(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let fixture = seed_reload_tool_fixture(directory.path());

        cx.update(|cx| {
            gpui_component::init(cx);
            crate::components::chat::input::init(cx);
            database::install_for_test(cx, directory.path());
            crate::foundation::i18n::init(cx);
            crate::state::providers::init(cx);
        });
        cx.run_until_parked();

        let runtime = cx.update(|cx| conversation::runtime::create(cx).unwrap());
        cx.run_until_parked();
        let model = cx.update(|cx| {
            let executor = database::ready_executor(cx).unwrap();
            cx.new(|_| ConversationModel::new(fixture.conversation_id.clone(), executor))
        });
        cx.update(|cx| model.update(cx, |model, cx| model.refresh(cx)));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(matches!(
                model.read(cx).operation(),
                ConversationOperation::Ready(ready) if ready.data().is_some()
            ));
        });
        let inspection_before_reload = cx.update(|cx| {
            let snapshot = model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .expect("loaded conversation");
            let invocation = snapshot
                .tool_invocations
                .iter()
                .find(|invocation| invocation.id == fixture.inspectable_invocation_id)
                .expect("inspectable invocation");
            tool_inspection_evidence_for_test(invocation)
        });
        assert!(inspection_before_reload.truncated);
        assert!(inspection_before_reload.provider_raw_hidden);
        assert!(
            inspection_before_reload
                .copy_text
                .contains("Provider raw payload is hidden")
        );
        assert!(
            !inspection_before_reload
                .copy_text
                .contains("raw-provider-secret")
        );

        let page = cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    ConversationDetailPage::new_without_focus(
                        model.clone(),
                        runtime.clone(),
                        window,
                        cx,
                    )
                })
            })
            .unwrap()
        });
        cx.update(|cx| {
            page.update(cx, |page, _window, _cx| {
                for lifecycle_id in &fixture.lifecycle_entry_ids {
                    assert!(
                        !page
                            .message_text_states
                            .iter()
                            .any(|state| &state.id == lifecycle_id),
                        "tool lifecycle entry must not create TextViewState: {lifecycle_id}"
                    );
                }
            })
            .unwrap();
        });
        let stale_invocation_id = "removed-between-reloads".to_string();
        let (old_expanded_preview, initial_row_keys) = cx.update(|cx| {
            page.update(cx, |page, window, cx| {
                page.toggle_tool_invocation(fixture.expanded_invocation_id.clone(), window, cx);
                page.toggle_tool_invocation(fixture.collapsed_invocation_id.clone(), window, cx);
                page.toggle_tool_invocation(fixture.collapsed_invocation_id.clone(), window, cx);
                page.expanded_tool_invocations
                    .insert(stale_invocation_id.clone(), true);

                assert_eq!(
                    page.expanded_tool_invocations
                        .get(&fixture.expanded_invocation_id),
                    Some(&true)
                );
                assert_eq!(
                    page.expanded_tool_invocations
                        .get(&fixture.collapsed_invocation_id),
                    Some(&false)
                );
                assert!(
                    page.tool_invocation_previews
                        .contains_key(&fixture.collapsed_invocation_id)
                );
                let preview = page
                    .tool_invocation_previews
                    .get(&fixture.expanded_invocation_id)
                    .unwrap()
                    .preview
                    .clone();
                assert_eq!(preview.access_requests[0].target.text, "old-target");
                (preview, page.timeline_rows.keys().to_vec())
            })
            .unwrap()
        });

        let updated_invocation = cx.update(|cx| {
            database::with_ready_repository(cx, |repository| {
                let current = repository
                    .get_tool_invocation(&fixture.expanded_invocation_id)?
                    .expect("expanded invocation exists");
                let mut approval = current.approval.expect("expanded approval exists");
                approval.request.access_requests = vec![tool_access_request("new-target")];
                let updated_invocation = repository.record_tool_invocation_approval(
                    &fixture.expanded_invocation_id,
                    approval,
                    ToolInvocationStatus::AwaitingApproval,
                )?;
                Ok(updated_invocation)
            })
            .unwrap()
        });

        cx.update(|cx| {
            page.update(cx, |page, _window, _cx| {
                page.last_tool_invocation_remeasure = None;
            })
            .unwrap();
            model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(updated_invocation.clone()),
                    }]),
                    cx,
                );
            });
        });
        cx.run_until_parked();

        let changed_preview = cx.update(|cx| {
            page.update(cx, |page, _window, _cx| {
                let rebuilt = page
                    .tool_invocation_previews
                    .get(&fixture.expanded_invocation_id)
                    .expect("changed event rebuilds expanded preview");
                assert_eq!(rebuilt.revision, updated_invocation.updated_at);
                assert_eq!(rebuilt.preview.access_requests[0].target.text, "new-target");
                assert!(!Arc::ptr_eq(&rebuilt.preview, &old_expanded_preview));
                let owning_row_key = page
                    .timeline_rows
                    .row_key_for_tool_invocation(&fixture.expanded_invocation_id)
                    .expect("expanded invocation owning row");
                let owning_row_ix = page
                    .timeline_rows
                    .keys()
                    .iter()
                    .position(|key| key == &owning_row_key)
                    .expect("owning row index");
                assert_eq!(
                    page.last_tool_invocation_remeasure,
                    Some(owning_row_ix..owning_row_ix + 1)
                );
                rebuilt.preview.clone()
            })
            .unwrap()
        });

        let appended_entry_id = cx.update(|cx| {
            database::with_ready_repository(cx, |repository| {
                let appended_entry =
                    repository.append_conversation_entry(NewConversationEntry {
                        conversation_id: fixture.conversation_id.clone(),
                        status: ConversationEntryStatus::Completed,
                        agent_run_id: None,
                        provider_step_id: None,
                        tool_invocation_id: None,
                        provider_item_id: None,
                        payload: ConversationEntryPayload::Message {
                            role: TranscriptRole::User,
                            content: vec![ContentPart::Text {
                                text: "later row changes the timeline key suffix".to_string(),
                            }],
                        },
                    })?;
                Ok(appended_entry.id.clone())
            })
            .unwrap()
        });

        cx.update(|cx| model.update(cx, |model, cx| model.refresh(cx)));
        cx.run_until_parked();

        cx.update(|cx| {
            let snapshot = model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .expect("reloaded conversation");
            let reloaded = snapshot
                .tool_invocations
                .iter()
                .find(|invocation| invocation.id == fixture.expanded_invocation_id)
                .expect("reloaded expanded invocation");
            assert_eq!(
                reloaded.approval.as_ref().unwrap().request.access_requests[0].target,
                "new-target"
            );
            let inspectable = snapshot
                .tool_invocations
                .iter()
                .find(|invocation| invocation.id == fixture.inspectable_invocation_id)
                .expect("reloaded inspectable invocation");
            assert_eq!(
                tool_inspection_evidence_for_test(inspectable),
                inspection_before_reload
            );

            page.update(cx, |page, _window, _cx| {
                assert_eq!(
                    &page.timeline_rows.keys()[..initial_row_keys.len()],
                    initial_row_keys.as_slice()
                );
                assert!(matches!(
                    page.timeline_rows.keys().last(),
                    Some(TimelineRowKey::User(id)) if id == &appended_entry_id
                ));
                assert_eq!(
                    page.expanded_tool_invocations
                        .get(&fixture.expanded_invocation_id),
                    Some(&true)
                );
                assert_eq!(
                    page.expanded_tool_invocations
                        .get(&fixture.collapsed_invocation_id),
                    Some(&false)
                );
                assert!(
                    !page
                        .expanded_tool_invocations
                        .contains_key(&stale_invocation_id)
                );
                assert!(
                    !page
                        .tool_invocation_previews
                        .contains_key(&fixture.collapsed_invocation_id)
                );

                let rebuilt = page
                    .tool_invocation_previews
                    .get(&fixture.expanded_invocation_id)
                    .expect("expanded preview is rebuilt");
                assert_eq!(rebuilt.revision, updated_invocation.updated_at);
                assert_eq!(rebuilt.preview.access_requests[0].target.text, "new-target");
                assert!(!Arc::ptr_eq(&rebuilt.preview, &changed_preview));

                let detail = tool_detail(page, &fixture.expanded_invocation_id);
                assert_eq!(detail.agent_run_id, fixture.agent_run_id);
                assert_eq!(detail.status, ToolInvocationStatus::AwaitingApproval);
                assert!(!detail.approval_decidable);
                for lifecycle_id in &fixture.lifecycle_entry_ids {
                    assert!(
                        !page
                            .message_text_states
                            .iter()
                            .any(|state| &state.id == lifecycle_id),
                        "reloaded tool lifecycle entry must not create TextViewState: {lifecycle_id}"
                    );
                }
                let owning_row_key = page
                    .timeline_rows
                    .row_key_for_tool_invocation(&fixture.expanded_invocation_id)
                    .expect("expanded invocation owning row");
                let owning_row_ix = page
                    .timeline_rows
                    .keys()
                    .iter()
                    .position(|key| key == &owning_row_key)
                    .expect("owning row index");
                assert_eq!(
                    timeline_row_remeasure_range(page.timeline_rows.keys(), &owning_row_key),
                    Some(owning_row_ix..owning_row_ix + 1)
                );
            })
            .unwrap();
        });

        let preserved_state = cx.update(|cx| {
            page.update(cx, |page, _window, _cx| {
                (
                    page.expanded_tool_invocations.clone(),
                    page.tool_invocation_previews
                        .get(&fixture.expanded_invocation_id)
                        .expect("expanded preview cache")
                        .preview
                        .clone(),
                    page.timeline_rows.keys().to_vec(),
                )
            })
            .unwrap()
        });
        let fresh_runtime_event =
            conversation::runtime::ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                conversation_id: fixture.conversation_id.clone(),
                agent_run_id: fixture.agent_run_id.clone(),
                tool_invocation_id: fixture.expanded_invocation_id.clone(),
            };
        cx.update(|cx| {
            page.update(cx, |page, window, cx| {
                page.handle_runtime_event(&runtime, &fresh_runtime_event, window, cx);
                assert!(!tool_detail(page, &fixture.expanded_invocation_id).approval_decidable);
                assert_tool_ui_state_preserved(
                    page,
                    &fixture.expanded_invocation_id,
                    &preserved_state,
                );
            })
            .unwrap();
        });

        let run_ticket = cx.update(|cx| {
            runtime.update(cx, |runtime, _cx| {
                runtime.install_tool_approval_authorities_for_test(
                    fixture.conversation_id.clone(),
                    fixture.agent_run_id.clone(),
                    &[
                        fixture.expanded_invocation_id.clone(),
                        fixture.collapsed_invocation_id.clone(),
                    ],
                )
            })
        });
        for invocation_id in [
            &fixture.expanded_invocation_id,
            &fixture.collapsed_invocation_id,
        ] {
            let event =
                conversation::runtime::ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                    conversation_id: fixture.conversation_id.clone(),
                    agent_run_id: fixture.agent_run_id.clone(),
                    tool_invocation_id: invocation_id.clone(),
                };
            cx.update(|cx| {
                page.update(cx, |page, window, cx| {
                    page.handle_runtime_event(&runtime, &event, window, cx);
                })
                .unwrap();
            });
        }
        cx.update(|cx| {
            page.update(cx, |page, _window, _cx| {
                assert!(tool_detail(page, &fixture.expanded_invocation_id).approval_decidable);
                assert!(tool_detail(page, &fixture.collapsed_invocation_id).approval_decidable);
                assert_tool_ui_state_preserved(
                    page,
                    &fixture.expanded_invocation_id,
                    &preserved_state,
                );
            })
            .unwrap();
        });

        let run_finished = cx.update(|cx| {
            runtime.update(cx, |runtime, _cx| {
                runtime.finish_run_event_for_test(run_ticket)
            })
        });
        cx.update(|cx| {
            page.update(cx, |page, window, cx| {
                page.handle_runtime_event(&runtime, &run_finished, window, cx);
                assert!(!tool_detail(page, &fixture.expanded_invocation_id).approval_decidable);
                assert!(!tool_detail(page, &fixture.collapsed_invocation_id).approval_decidable);
                assert_tool_ui_state_preserved(
                    page,
                    &fixture.expanded_invocation_id,
                    &preserved_state,
                );
            })
            .unwrap();
        });
    }

    fn seed_reload_tool_fixture(data_dir: &Path) -> ReloadToolFixture {
        let store =
            FreshStore::open_or_create_initial(data_dir.join(jaco_db::DATABASE_FILE)).unwrap();
        let repository = store.repository();
        let project = repository
            .insert_project(NewProject {
                path: data_dir.to_string_lossy().into_owned(),
                display_name: "Reload Test".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Reload Test".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: conversation_settings(),
            })
            .unwrap();
        let trigger = repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "inspect tools".to_string(),
                    }],
                },
            })
            .unwrap();
        let run = repository
            .insert_agent_run(NewAgentRun {
                conversation_id: conversation.id.clone(),
                trigger_entry_id: trigger.id.clone(),
                trigger_kind: AgentRunTriggerKind::User,
                input: agent_run_input(),
            })
            .unwrap();
        let expanded_invocation_id =
            insert_approval_invocation(&repository, &run.id, "expanded-call", "old-target");
        let collapsed_invocation_id =
            insert_approval_invocation(&repository, &run.id, "collapsed-call", "collapsed-target");
        let inspectable_invocation_id = insert_inspectable_invocation(&repository, &run.id);
        let lifecycle_entry_ids = append_tool_lifecycle_entries(
            &repository,
            &conversation.id,
            &run.id,
            &expanded_invocation_id,
        );
        repository
            .finish_agent_run(
                &run.id,
                FinishAgentRun {
                    status: AgentRunStatus::Canceled,
                    stopped_reason: AgentStoppedReason::Canceled,
                    error: None,
                    final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                        conversation_id: conversation.id.clone(),
                        status: ConversationEntryStatus::Completed,
                        agent_run_id: Some(run.id.clone()),
                        provider_step_id: None,
                        tool_invocation_id: None,
                        provider_item_id: None,
                        payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                            code: ConversationStatusCode::Canceled,
                            message: None,
                        }),
                    })),
                },
            )
            .unwrap();

        let fixture = ReloadToolFixture {
            conversation_id: conversation.id,
            agent_run_id: run.id,
            expanded_invocation_id,
            collapsed_invocation_id,
            inspectable_invocation_id,
            lifecycle_entry_ids,
        };
        drop(repository);
        drop(store);
        fixture
    }

    fn insert_approval_invocation(
        repository: &jaco_db::FreshRepository,
        agent_run_id: &str,
        call_id: &str,
        access_target: &str,
    ) -> ToolInvocationId {
        let invocation = repository
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run_id.to_string(),
                provider_step_id: None,
                status: ToolInvocationStatus::Requested,
                input: ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    call_id: call_id.to_string(),
                    arguments: ToolArguments {
                        value: serde_json::json!({"text": call_id}),
                    },
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: None,
                error: None,
            })
            .unwrap();
        repository
            .request_tool_invocation_approval(
                &invocation.id,
                NewToolInvocationApproval {
                    request: ApprovalRequestPayload {
                        reason: "test access".to_string(),
                        tool_source: ToolSource::Local,
                        tool_name: "echo".to_string(),
                        arguments_preview: "{}".to_string(),
                        access_requests: vec![tool_access_request(access_target)],
                    },
                    expires_at: None,
                },
            )
            .unwrap()
            .id
    }

    fn insert_inspectable_invocation(
        repository: &jaco_db::FreshRepository,
        agent_run_id: &str,
    ) -> ToolInvocationId {
        repository
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run_id.to_string(),
                provider_step_id: None,
                status: ToolInvocationStatus::Succeeded,
                input: ToolInvocationInput {
                    source: ToolSource::Mcp {
                        server_id: "fixture-server".to_string(),
                    },
                    namespace: Some("fixture".to_string()),
                    tool_name: "inspect".to_string(),
                    runtime_tool_name: "fixture__inspect".to_string(),
                    call_id: "inspectable-call".to_string(),
                    arguments: ToolArguments {
                        value: serde_json::json!({"large": "x".repeat(300 * 1_024)}),
                    },
                    approval_policy: ToolApprovalPolicy::Never,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: Some(ToolInvocationOutput {
                    content: vec![ContentPart::Text {
                        text: "y".repeat(70 * 1_024),
                    }],
                    structured_output: Some(StructuredOutput {
                        value: serde_json::json!({"result": "bounded"}),
                    }),
                    raw_output: Some(ProviderRawPayload {
                        provider_kind: "fixture".to_string(),
                        value: serde_json::json!({"secret": "raw-provider-secret"}),
                    }),
                    is_error: false,
                }),
                error: None,
            })
            .unwrap()
            .id
    }

    fn append_tool_lifecycle_entries(
        repository: &jaco_db::FreshRepository,
        conversation_id: &str,
        agent_run_id: &str,
        invocation_id: &ToolInvocationId,
    ) -> Vec<String> {
        let call_id = "expanded-call".to_string();
        let call = repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation_id.to_string(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(agent_run_id.to_string()),
                provider_step_id: None,
                tool_invocation_id: Some(invocation_id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ToolCall(jaco_core::ToolCallEntry {
                    tool_invocation_id: Some(invocation_id.clone()),
                    call_id: call_id.clone(),
                    source: ToolSource::Local,
                    name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    arguments: ToolArguments {
                        value: serde_json::json!({"text": "synthetic"}),
                    },
                }),
            })
            .unwrap()
            .id
            .clone();
        let result = repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation_id.to_string(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(agent_run_id.to_string()),
                provider_step_id: None,
                tool_invocation_id: Some(invocation_id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ToolResult(jaco_core::ToolResultEntry {
                    tool_invocation_id: Some(invocation_id.clone()),
                    call_id,
                    content: vec![ContentPart::Text {
                        text: "synthetic result".to_string(),
                    }],
                    is_error: false,
                    structured_output: None,
                    raw_output: None,
                }),
            })
            .unwrap()
            .id
            .clone();
        vec![call, result]
    }

    fn tool_access_request(target: &str) -> ToolAccessRequestPayload {
        ToolAccessRequestPayload {
            kind: ToolAccessKind::Read,
            target: target.to_string(),
            normalized_path: Some(format!("/normalized/{target}")),
            within_project: true,
            reason_key: Some("test".to_string()),
        }
    }

    fn conversation_settings() -> ConversationSettingsSnapshot {
        ConversationSettingsSnapshot {
            prompt: None,
            provider_id: None,
            model_id: None,
            model_capabilities: None,
            tool_policy: tool_policy(),
        }
    }

    fn agent_run_input() -> AgentRunInput {
        AgentRunInput {
            prompt_snapshot: None,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            settings_snapshot: RunSettingsSnapshot {
                prompt: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                model_capabilities: conservative_model_capabilities("openai"),
                provider_settings: ProviderSettingsPayload {
                    provider_kind: "openai".to_string(),
                    fields: Vec::new(),
                },
                reasoning_selection: None,
                tool_policy: tool_policy(),
            },
            runtime_snapshot: AgentRuntimeSnapshot {
                engine: AgentEngineKind::Rig,
                engine_version: "test".to_string(),
                skill_catalog_hash: None,
                tool_name_strategy: ToolNameStrategy::Direct,
            },
            max_steps: 8,
        }
    }

    fn tool_policy() -> ToolPolicySnapshot {
        ToolPolicySnapshot {
            approval_policy: ToolApprovalPolicy::OnRequest,
            enabled_sources: vec![ToolSource::Local],
            max_steps: 8,
            approval_mode: ToolApprovalMode::RequestApproval,
            permission_scope: None,
        }
    }

    fn tool_detail(
        page: &ConversationDetailPage,
        invocation_id: &ToolInvocationId,
    ) -> ToolInvocationDetail {
        for index in 0..page.timeline_rows.keys().len() {
            let Some(TimelineRow::Agent(row)) = page.timeline_rows.row(index) else {
                continue;
            };
            if let Some(detail) = row.items.iter().find_map(|item| match item {
                AgentDetailItem::ToolInvocation(detail) if &detail.id == invocation_id => {
                    Some(detail.clone())
                }
                AgentDetailItem::Entry(_)
                | AgentDetailItem::ToolInvocation(_)
                | AgentDetailItem::UnresolvedToolLifecycle(_) => None,
            }) {
                return detail;
            }
        }
        panic!("tool invocation detail {invocation_id} is missing");
    }

    fn assert_tool_ui_state_preserved(
        page: &ConversationDetailPage,
        expanded_invocation_id: &ToolInvocationId,
        preserved: &(
            HashMap<ToolInvocationId, bool>,
            Arc<ToolInvocationPreview>,
            Vec<TimelineRowKey>,
        ),
    ) {
        assert_eq!(&page.expanded_tool_invocations, &preserved.0);
        assert_eq!(page.timeline_rows.keys(), preserved.2.as_slice());
        let current = page
            .tool_invocation_previews
            .get(expanded_invocation_id)
            .expect("preserved expanded preview cache");
        assert!(Arc::ptr_eq(&current.preview, &preserved.1));
    }

    #[test]
    fn run_error_ticket_is_consumed_only_by_its_owner() {
        let mut owned = Some(7_u64);

        assert!(!take_matching_ticket(&mut owned, &8));
        assert_eq!(owned, Some(7));
        assert!(take_matching_ticket(&mut owned, &7));
        assert_eq!(owned, None);
    }

    #[test]
    fn reload_tool_invocation_state_clears_cache_and_prunes_missing_expansion() {
        let mut expanded = HashMap::from([
            ("kept-open".to_string(), true),
            ("kept-closed".to_string(), false),
            ("removed".to_string(), true),
        ]);
        let mut previews = HashMap::from([
            ("kept-open".to_string(), "old-open-preview"),
            ("kept-closed".to_string(), "old-closed-preview"),
            ("removed".to_string(), "old-removed-preview"),
        ]);
        let current_ids = HashSet::from([
            "kept-open".to_string(),
            "kept-closed".to_string(),
            "new".to_string(),
        ]);

        let rebuild =
            reconcile_tool_invocation_ui_state(&mut expanded, &mut previews, &current_ids);

        assert!(previews.is_empty());
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded.get("kept-open"), Some(&true));
        assert_eq!(expanded.get("kept-closed"), Some(&false));
        assert_eq!(rebuild, vec!["kept-open".to_string()]);
    }

    #[test]
    fn invocation_update_remeasures_only_its_owning_agent_row() {
        let keys = vec![
            TimelineRowKey::User("prompt".to_string()),
            TimelineRowKey::Agent("run-a".to_string()),
            TimelineRowKey::Agent("run-b".to_string()),
        ];

        assert_eq!(
            timeline_row_remeasure_range(&keys, &TimelineRowKey::Agent("run-b".to_string())),
            Some(2..3)
        );
        assert_eq!(
            timeline_row_remeasure_range(&keys, &TimelineRowKey::User("prompt".to_string())),
            Some(0..1)
        );
        assert_eq!(
            timeline_row_remeasure_range(&keys, &TimelineRowKey::Agent("missing".to_string())),
            None
        );
    }

    #[test]
    fn message_text_update_detects_unchanged_source() {
        assert_eq!(
            message_text_update("hello", "hello"),
            MessageTextUpdate::Unchanged
        );
    }

    #[test]
    fn message_text_update_detects_append_only_source() {
        assert_eq!(
            message_text_update("hello", "hello world"),
            MessageTextUpdate::Append(" world")
        );
    }

    #[test]
    fn conversation_runtime_append_only_message_update_keeps_append_delta() {
        assert_eq!(
            message_text_update("streaming", "streaming output"),
            MessageTextUpdate::Append(" output")
        );
    }

    #[test]
    fn message_text_update_replaces_non_append_source() {
        assert_eq!(
            message_text_update("hello world", "hello markdown"),
            MessageTextUpdate::Replace
        );
    }

    #[test]
    fn streaming_markdown_keeps_latest_complete_source() {
        let chunks = [
            "Paragraph with `inline code`.\n\n",
            "```ru",
            "st\nfn main() {\n",
            "    println!(\"hello\");\n}\n",
            "```\n\n",
            "```unknown-language\nplain fallback\n```\n",
            "```\nuntyped fallback\n```",
        ];
        let expected = chunks.concat();
        let mut current = String::new();

        for chunk in chunks {
            let next = format!("{current}{chunk}");
            assert_eq!(
                message_text_update(&current, &next),
                MessageTextUpdate::Append(chunk)
            );
            current.push_str(chunk);
        }

        assert_eq!(current, expected);
    }

    #[test]
    fn streaming_markdown_preserves_split_fences_and_plain_fallback_source() {
        let chunks = [
            "before\n\n`",
            "``javascript\nconst value = 1;\n",
            "```\n\n```not-registered\nvalue\n",
            "```\n\nafter",
        ];
        let mut source = String::new();
        for chunk in chunks {
            source.push_str(chunk);
        }

        assert!(source.contains("```javascript\nconst value = 1;\n```"));
        assert!(source.contains("```not-registered\nvalue\n```"));
        assert!(source.ends_with("after"));
    }
}
