mod attachment_access;
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
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use fluent_bundle::FluentArgs;
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
    AgentRunId, AttachmentId, ConversationEffect, ConversationEntryId, ConversationEntryStatus,
    ConversationId, ToolInvocationId,
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
    attachment_access: HashMap<AttachmentId, attachment_access::AttachmentAccessState>,
    attachment_access_targets: HashMap<AttachmentId, AttachmentAccessFingerprint>,
    attachment_access_generation: u64,
    attachment_probe_task: Option<Task<()>>,
    attachment_actions_in_flight: HashSet<(
        attachment_access::AttachmentActionTarget,
        attachment_access::AttachmentAction,
    )>,
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
    key: attachments::TimelineTextKey,
    state: Entity<TextViewState>,
    source: String,
    has_streaming_baseline: bool,
    _subscription: Subscription,
}

#[derive(Clone)]
struct AttachmentProbeInput {
    attachment_id: AttachmentId,
    kind: attachment_access::AttachmentAccessKind,
    static_problem: Option<attachment_access::AttachmentAccessProblem>,
    record: Option<jaco_core::ConversationAttachment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttachmentAccessFingerprint {
    kind: attachment_access::AttachmentAccessKind,
    static_problem: Option<attachment_access::AttachmentAccessProblem>,
    record_updated_at: Option<time::OffsetDateTime>,
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

fn requires_streaming_text_state_rebuild(
    is_streaming: bool,
    has_streaming_baseline: bool,
    update: &MessageTextUpdate<'_>,
) -> bool {
    (!has_streaming_baseline && (is_streaming || matches!(update, MessageTextUpdate::Append(_))))
        || (is_streaming && matches!(update, MessageTextUpdate::Replace))
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
            attachment_access: HashMap::new(),
            attachment_access_targets: HashMap::new(),
            attachment_access_generation: 0,
            attachment_probe_task: None,
            attachment_actions_in_flight: HashSet::new(),
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
        page.sync_attachment_access(true, cx);
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
                self.sync_attachment_access(true, cx);
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
                self.sync_attachment_access(false, cx);
                self.sync_timeline(cx, None);
            }
            ConversationEffect::EntryChanged { entry_id, .. } => {
                self.sync_message_text_state(entry_id, cx);
                self.sync_attachment_access(false, cx);
                self.update_timeline_entry(entry_id, cx);
            }
            ConversationEffect::AttachmentChanged { attachment_id } => {
                self.sync_attachment_access(true, cx);
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
                self.attachment_access.clear();
                self.attachment_access_targets.clear();
                self.attachment_access_generation =
                    self.attachment_access_generation.wrapping_add(1);
                self.attachment_probe_task = None;
                self.attachment_actions_in_flight.clear();
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
            {
                let page = page.clone();
                move |target, action, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.handle_attachment_action(target, action, window, cx);
                    });
                }
            },
        );
        let attachment_access = self.attachment_access_view_map(cx);
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
                    &attachment_access,
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
                let attachments_by_id = attachments::attachments_by_id(&snapshot.attachments);
                snapshot
                    .entries
                    .iter()
                    .flat_map(|item| {
                        let is_streaming = item.status == ConversationEntryStatus::Running;
                        message_text_sources(item, &attachments_by_id)
                            .into_iter()
                            .map(move |(key, source)| (key, source, is_streaming))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let next_keys = sources
            .iter()
            .map(|(key, _, _)| key.clone())
            .collect::<HashSet<_>>();

        for (key, source, is_streaming) in sources {
            self.ensure_message_text_state(key, &source, is_streaming, cx);
        }

        self.message_text_states
            .retain(|entry| next_keys.contains(&entry.key));
    }

    fn sync_message_text_state(&mut self, item_id: &ConversationEntryId, cx: &mut Context<Self>) {
        let sources = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|conversation| {
                let attachments_by_id = attachments::attachments_by_id(&conversation.attachments);
                conversation
                    .entries
                    .iter()
                    .find(|entry| &entry.id == item_id)
                    .map(|entry| {
                        let is_streaming = entry.status == ConversationEntryStatus::Running;
                        message_text_sources(entry, &attachments_by_id)
                            .into_iter()
                            .map(|(key, source)| (key, source, is_streaming))
                            .collect::<Vec<_>>()
                    })
            })
            .unwrap_or_default();
        let next_keys = sources
            .iter()
            .map(|(key, _, _)| key.clone())
            .collect::<HashSet<_>>();
        let current_keys = self
            .message_text_states
            .iter()
            .filter(|state| timeline_text_key_entry_id(&state.key) == item_id)
            .map(|state| state.key.clone())
            .collect::<HashSet<_>>();

        if current_keys != next_keys {
            self.message_text_states
                .retain(|state| timeline_text_key_entry_id(&state.key) != item_id);
        }
        for (key, source, is_streaming) in sources {
            self.ensure_message_text_state(key, &source, is_streaming, cx);
        }
    }

    fn sync_attachment_access(&mut self, force: bool, cx: &mut Context<Self>) {
        let inputs = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(projected_attachment_probe_inputs)
            .unwrap_or_default();
        let targets = inputs
            .iter()
            .map(|(attachment_id, input)| {
                (
                    attachment_id.clone(),
                    AttachmentAccessFingerprint {
                        kind: input.kind,
                        static_problem: input.static_problem.clone(),
                        record_updated_at: input.record.as_ref().map(|record| record.updated_at),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        if !force && targets == self.attachment_access_targets {
            return;
        }

        self.attachment_access_generation = self.attachment_access_generation.wrapping_add(1);
        let generation = self.attachment_access_generation;
        self.attachment_probe_task = None;
        self.attachment_access_targets = targets;
        self.attachment_actions_in_flight.retain(|(target, _)| {
            self.attachment_access_targets
                .get(&target.attachment_id)
                .is_some_and(|fingerprint| fingerprint.kind == target.kind.into())
        });

        let mut probes = Vec::new();
        self.attachment_access = inputs
            .into_iter()
            .map(|(attachment_id, input)| {
                let state = if let Some(problem) = input.static_problem {
                    attachment_access::AttachmentAccessState::Unavailable(problem)
                } else if let Some(record) = input.record {
                    probes.push((input.attachment_id, input.kind, record));
                    attachment_access::AttachmentAccessState::Checking
                } else {
                    attachment_access::AttachmentAccessState::Unavailable(
                        attachment_access::AttachmentAccessProblem::MissingRecord,
                    )
                };
                (attachment_id, state)
            })
            .collect();

        if probes.is_empty() {
            return;
        }

        let conversation_id = self.conversation_id.clone();
        let data_dir =
            crate::database::store(cx).read(cx, |resource| resource.target.data_dir.clone());
        let probe = cx.background_spawn(async move {
            probes
                .into_iter()
                .map(|(attachment_id, kind, record)| {
                    let result = attachment_access::resolve_local_attachment(
                        &record,
                        &conversation_id,
                        kind,
                        &data_dir,
                    );
                    (attachment_id, result)
                })
                .collect::<Vec<_>>()
        });
        self.attachment_probe_task = Some(cx.spawn(async move |page, cx| {
            let results = probe.await;
            let Some(page) = page.upgrade() else {
                return;
            };
            page.update(cx, |page, cx| {
                if page.attachment_access_generation != generation {
                    return;
                }
                let attachment_ids = results
                    .iter()
                    .map(|(attachment_id, _)| attachment_id.clone())
                    .collect::<Vec<_>>();
                for (attachment_id, result) in results {
                    if !page.attachment_access_targets.contains_key(&attachment_id) {
                        continue;
                    }
                    page.attachment_access.insert(
                        attachment_id,
                        match result {
                            Ok(resolved) => {
                                attachment_access::AttachmentAccessState::Available(resolved)
                            }
                            Err(problem) => {
                                attachment_access::AttachmentAccessState::Unavailable(problem)
                            }
                        },
                    );
                }
                for attachment_id in attachment_ids {
                    page.sync_attachment_rows(&attachment_id, cx);
                }
                cx.notify();
            });
        }));
    }

    fn attachment_access_view_map(
        &self,
        cx: &App,
    ) -> HashMap<AttachmentId, attachment_access::AttachmentAccessView> {
        let records = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| attachments::attachments_by_id(&snapshot.attachments))
            .unwrap_or_default();

        self.attachment_access
            .iter()
            .map(|(attachment_id, state)| {
                let source = state
                    .resolved()
                    .map(attachment_access::ResolvedLocalAttachment::source_label)
                    .or_else(|| {
                        records.get(attachment_id).map(|record| {
                            attachment_access::AttachmentSourceLabel::from(
                                attachment_access::attachment_source_hint(record),
                            )
                        })
                    })
                    .unwrap_or(attachment_access::AttachmentSourceLabel::Unknown);
                let busy_actions = self
                    .attachment_actions_in_flight
                    .iter()
                    .filter_map(|(target, action)| {
                        (&target.attachment_id == attachment_id).then_some(*action)
                    })
                    .collect();
                (
                    attachment_id.clone(),
                    attachment_access::AttachmentAccessView {
                        availability: state.availability(),
                        source,
                        busy_actions,
                        resolved: state.resolved().cloned(),
                    },
                )
            })
            .collect()
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
        let text_states = self.message_text_state_map();
        let attachment_access = self.attachment_access_view_map(cx);
        let Some(key) =
            self.timeline_rows
                .update_entry(entry, &attachments, &text_states, &attachment_access)
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

    fn handle_attachment_action(
        &mut self,
        target: attachment_access::AttachmentActionTarget,
        action: attachment_access::AttachmentAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let action_key = (target.clone(), action);
        if !self.attachment_actions_in_flight.insert(action_key.clone()) {
            return;
        }
        self.sync_attachment_rows(&target.attachment_id, cx);
        cx.notify();

        let record = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .attachments
                    .iter()
                    .find(|record| record.id == target.attachment_id)
                    .cloned()
            });
        let Some(record) = record else {
            self.finish_attachment_action_failure(
                action_key,
                attachment_access::AttachmentAccessProblem::MissingRecord,
                window,
                cx,
            );
            return;
        };
        let record_updated_at = record.updated_at;
        let conversation_id = self.conversation_id.clone();
        let data_dir =
            crate::database::store(cx).read(cx, |resource| resource.target.data_dir.clone());
        let preflight_target = target.clone();
        let preflight = cx.background_spawn(async move {
            attachment_access::resolve_local_attachment(
                &record,
                &conversation_id,
                preflight_target.kind,
                &data_dir,
            )
        });
        let page = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = preflight.await;
            let _ = page.update_in(cx, |page, window, cx| {
                page.finish_attachment_preflight(action_key, record_updated_at, result, window, cx);
            });
        });
        crate::app::tasks::retain_window(window, task, cx);
    }

    fn finish_attachment_preflight(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        record_updated_at: time::OffsetDateTime,
        result: Result<
            attachment_access::ResolvedLocalAttachment,
            attachment_access::AttachmentAccessProblem,
        >,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (target, action) = &action_key;
        let still_current = self.attachment_record_is_current(target, record_updated_at, cx);
        if !still_current {
            self.finish_stale_attachment_action(action_key, window, cx);
            return;
        }

        let resolved = match result {
            Ok(resolved) => resolved,
            Err(problem) => {
                self.finish_attachment_action_failure(action_key, problem, window, cx);
                return;
            }
        };
        if resolved.attachment_id() != &target.attachment_id
            || resolved.kind() != target.kind.into()
        {
            self.finish_attachment_action_failure(
                action_key,
                attachment_access::AttachmentAccessProblem::KindMismatch,
                window,
                cx,
            );
            return;
        }
        self.attachment_access.insert(
            target.attachment_id.clone(),
            attachment_access::AttachmentAccessState::Available(resolved.clone()),
        );

        match action {
            attachment_access::AttachmentAction::Open => {
                cx.open_with_system(resolved.path());
                self.finish_attachment_action_success(action_key, cx);
            }
            attachment_access::AttachmentAction::Reveal => {
                cx.reveal_path(resolved.path());
                self.finish_attachment_action_success(action_key, cx);
            }
            attachment_access::AttachmentAction::SaveCopy => {
                self.prompt_attachment_save_copy(
                    action_key,
                    record_updated_at,
                    resolved,
                    window,
                    cx,
                );
            }
        }
    }

    fn attachment_record_is_current(
        &self,
        target: &attachment_access::AttachmentActionTarget,
        record_updated_at: time::OffsetDateTime,
        cx: &App,
    ) -> bool {
        self.attachment_access_targets
            .get(&target.attachment_id)
            .is_some_and(|fingerprint| fingerprint.kind == target.kind.into())
            && self
                .conversation
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .and_then(|snapshot| {
                    snapshot
                        .attachments
                        .iter()
                        .find(|record| record.id == target.attachment_id)
                })
                .is_some_and(|record| record.updated_at == record_updated_at)
    }

    fn prompt_attachment_save_copy(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        record_updated_at: time::OffsetDateTime,
        resolved: attachment_access::ResolvedLocalAttachment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = resolved.path().to_path_buf();
        let initial_dir = source
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let suggested = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .and_then(|snapshot| {
                snapshot
                    .attachments
                    .iter()
                    .find(|record| record.id == action_key.0.attachment_id)
            })
            .and_then(|record| attachment_access::safe_display_name(record.name.as_deref()))
            .unwrap_or_else(|| {
                cx.global::<I18n>()
                    .t("conversation-attachment-fallback-name")
                    .to_string()
            });
        let prompt = cx.prompt_for_new_path(&initial_dir, Some(&suggested));
        let fallback_name = cx
            .global::<I18n>()
            .t("conversation-attachment-fallback-name")
            .to_string();
        let page = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let target_path = match prompt.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    let _ = page.update_in(cx, |page, _, cx| {
                        page.finish_attachment_action_success(action_key, cx);
                    });
                    return;
                }
                Ok(Err(_)) | Err(_) => {
                    let _ = page.update_in(cx, |page, window, cx| {
                        page.finish_attachment_save_failure(action_key, window, cx);
                    });
                    return;
                }
            };
            let action_key_for_check = action_key.clone();
            let current = page
                .update_in(cx, |page, window, cx| {
                    let current = page.attachment_record_is_current(
                        &action_key_for_check.0,
                        record_updated_at,
                        cx,
                    );
                    if !current {
                        page.finish_stale_attachment_action(action_key_for_check, window, cx);
                    }
                    current
                })
                .unwrap_or(true);
            if !current {
                return;
            }
            let saved_name = attachment_saved_name(&target_path, &fallback_name);
            let copy = cx.background_spawn(async move {
                crate::foundation::persistence::atomic_copy_file(&source, &target_path)
            });
            let result = copy.await;
            let _ = page.update_in(cx, |page, window, cx| match result {
                Ok(_) => page.finish_attachment_save_success(action_key, saved_name, window, cx),
                Err(_) => page.finish_attachment_save_failure(action_key, window, cx),
            });
        });
        crate::app::tasks::retain_window(window, task, cx);
    }

    fn finish_attachment_action_success(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        cx: &mut Context<Self>,
    ) {
        let attachment_id = action_key.0.attachment_id.clone();
        self.attachment_actions_in_flight.remove(&action_key);
        self.sync_attachment_rows(&attachment_id, cx);
        cx.notify();
    }

    fn finish_attachment_action_failure(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        problem: attachment_access::AttachmentAccessProblem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let attachment_id = action_key.0.attachment_id.clone();
        self.attachment_actions_in_flight.remove(&action_key);
        self.attachment_access.insert(
            attachment_id.clone(),
            attachment_access::AttachmentAccessState::Unavailable(problem),
        );
        self.sync_attachment_rows(&attachment_id, cx);
        let title = cx
            .global::<I18n>()
            .t("conversation-attachment-action-failed-title");
        let message = cx
            .global::<I18n>()
            .t("conversation-attachment-action-failed-message");
        push_conversation_notification(window, cx, title, message, NotificationType::Error);
        cx.notify();
    }

    fn finish_stale_attachment_action(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_after_stale_attachment_action(action_key, cx);
        let title = cx
            .global::<I18n>()
            .t("conversation-attachment-action-failed-title");
        let message = cx
            .global::<I18n>()
            .t("conversation-attachment-action-failed-message");
        push_conversation_notification(window, cx, title, message, NotificationType::Error);
    }

    fn refresh_after_stale_attachment_action(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        cx: &mut Context<Self>,
    ) {
        let attachment_id = action_key.0.attachment_id.clone();
        self.attachment_actions_in_flight.remove(&action_key);
        self.sync_attachment_access(true, cx);
        self.sync_attachment_rows(&attachment_id, cx);
        cx.notify();
    }

    fn finish_attachment_save_failure(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let attachment_id = action_key.0.attachment_id.clone();
        self.attachment_actions_in_flight.remove(&action_key);
        self.sync_attachment_rows(&attachment_id, cx);
        let title = cx
            .global::<I18n>()
            .t("conversation-attachment-save-failed-title");
        let message = cx
            .global::<I18n>()
            .t("conversation-attachment-save-failed-message");
        push_conversation_notification(window, cx, title, message, NotificationType::Error);
        cx.notify();
    }

    fn finish_attachment_save_success(
        &mut self,
        action_key: (
            attachment_access::AttachmentActionTarget,
            attachment_access::AttachmentAction,
        ),
        saved_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let attachment_id = action_key.0.attachment_id.clone();
        self.attachment_actions_in_flight.remove(&action_key);
        self.sync_attachment_rows(&attachment_id, cx);
        let mut args = FluentArgs::new();
        args.set("name", saved_name);
        let title = cx
            .global::<I18n>()
            .t("conversation-attachment-save-success-title");
        let message = cx
            .global::<I18n>()
            .t_with_args("conversation-attachment-save-success-message", &args);
        push_conversation_notification(window, cx, title, message, NotificationType::Success);
        cx.notify();
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
        key: attachments::TimelineTextKey,
        source: &str,
        is_streaming: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self
            .message_text_states
            .iter()
            .position(|entry| entry.key == key)
        {
            let update = message_text_update(&self.message_text_states[index].source, source);
            if requires_streaming_text_state_rebuild(
                is_streaming,
                self.message_text_states[index].has_streaming_baseline,
                &update,
            ) {
                self.message_text_states[index] =
                    Self::create_message_text_state(key, source, true, cx);
                return;
            }

            let entry = &mut self.message_text_states[index];
            match update {
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
                    entry.has_streaming_baseline = false;
                }
            }
            return;
        }

        self.message_text_states
            .push(Self::create_message_text_state(
                key,
                source,
                is_streaming,
                cx,
            ));
    }

    fn create_message_text_state(
        key: attachments::TimelineTextKey,
        source: &str,
        is_streaming: bool,
        cx: &mut Context<Self>,
    ) -> MessageTextState {
        let state = cx.new(|cx| message_text_view_state(source, is_streaming, cx));
        let observed_item_id = timeline_text_key_entry_id(&key).clone();
        let subscription = cx.observe(&state, move |page, _, cx| {
            if let Some(row_ix) = page.timeline_rows.row_index_for_item(&observed_item_id) {
                page.timeline.remeasure_items(row_ix..row_ix + 1);
                cx.notify();
            }
        });

        MessageTextState {
            key,
            state,
            source: source.to_owned(),
            has_streaming_baseline: is_streaming,
            _subscription: subscription,
        }
    }

    fn message_text_state_map(
        &self,
    ) -> HashMap<attachments::TimelineTextKey, Entity<TextViewState>> {
        self.message_text_states
            .iter()
            .map(|entry| (entry.key.clone(), entry.state.clone()))
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

fn message_text_view_state(
    source: &str,
    is_streaming: bool,
    cx: &mut Context<TextViewState>,
) -> TextViewState {
    if !is_streaming {
        return TextViewState::markdown(source, cx);
    }

    // Seed the async parser through the same append path used by later streaming deltas.
    let mut state = TextViewState::markdown("", cx);
    state.push_str(source, cx);
    state
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

fn message_text_sources(
    entry: &jaco_core::ConversationEntry,
    attachments_by_id: &HashMap<jaco_core::AttachmentId, jaco_core::ConversationAttachment>,
) -> Vec<(attachments::TimelineTextKey, String)> {
    if tool_invocation::is_tool_lifecycle_entry(entry) {
        return Vec::new();
    }

    if matches!(
        &entry.payload,
        jaco_core::ConversationEntryPayload::Message { .. }
    ) {
        return attachments::project_message_content(entry, attachments_by_id)
            .into_iter()
            .filter_map(|block| match block {
                attachments::MessageContentBlock::Text {
                    start_part_index,
                    markdown,
                } if !markdown.is_empty() => Some((
                    attachments::TimelineTextKey::MessageBlock {
                        entry_id: entry.id.clone(),
                        start_part_index,
                    },
                    markdown,
                )),
                attachments::MessageContentBlock::Text { .. }
                | attachments::MessageContentBlock::Images { .. }
                | attachments::MessageContentBlock::File(_)
                | attachments::MessageContentBlock::Attachment(_) => None,
            })
            .collect();
    }

    let source = format::item_markdown(entry);
    if source.is_empty() {
        Vec::new()
    } else {
        vec![(
            attachments::TimelineTextKey::WholeEntry(entry.id.clone()),
            source,
        )]
    }
}

fn projected_attachment_probe_inputs(
    snapshot: &jaco_core::Conversation,
) -> HashMap<AttachmentId, AttachmentProbeInput> {
    let attachments_by_id = attachments::attachments_by_id(&snapshot.attachments);
    let mut inputs = HashMap::<AttachmentId, AttachmentProbeInput>::new();
    for entry in &snapshot.entries {
        for block in attachments::project_message_content(entry, &attachments_by_id) {
            match block {
                attachments::MessageContentBlock::Images { attachments, .. } => {
                    for image in attachments {
                        let attachment_id = image.attachment_id().clone();
                        insert_attachment_probe_input(
                            &mut inputs,
                            AttachmentProbeInput {
                                attachment_id: attachment_id.clone(),
                                kind: attachment_access::AttachmentAccessKind::Image,
                                static_problem: None,
                                record: attachments_by_id.get(&attachment_id).cloned(),
                            },
                        );
                    }
                }
                attachments::MessageContentBlock::File(card)
                | attachments::MessageContentBlock::Attachment(card) => {
                    let attachment_id = card.attachment_id.clone();
                    insert_attachment_probe_input(
                        &mut inputs,
                        AttachmentProbeInput {
                            attachment_id: attachment_id.clone(),
                            kind: card.kind.into(),
                            static_problem: card.static_problem,
                            record: attachments_by_id.get(&attachment_id).cloned(),
                        },
                    );
                }
                attachments::MessageContentBlock::Text { .. } => {}
            }
        }
    }
    inputs
}

fn insert_attachment_probe_input(
    inputs: &mut HashMap<AttachmentId, AttachmentProbeInput>,
    input: AttachmentProbeInput,
) {
    match inputs.entry(input.attachment_id.clone()) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(input);
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if entry.get().kind != input.kind {
                entry.get_mut().static_problem =
                    Some(attachment_access::AttachmentAccessProblem::KindMismatch);
                entry.get_mut().record = None;
            }
        }
    }
}

fn attachment_saved_name(path: &Path, fallback: &str) -> String {
    attachment_access::safe_display_name(path.file_name().and_then(|name| name.to_str()))
        .unwrap_or_else(|| fallback.to_string())
}

fn timeline_text_key_entry_id(key: &attachments::TimelineTextKey) -> &ConversationEntryId {
    match key {
        attachments::TimelineTextKey::WholeEntry(entry_id)
        | attachments::TimelineTextKey::MessageBlock { entry_id, .. } => entry_id,
    }
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
        fs,
        path::Path,
        sync::Arc,
    };

    use super::{
        ConversationDetailPage, ConversationModel, ConversationOperation, MessageTextUpdate,
        attachments, conversation,
        message::{TimelineRow, TimelineRowKey},
        message_text_sources, message_text_update, message_text_view_state,
        reconcile_tool_invocation_ui_state, requires_streaming_text_state_rebuild,
        take_matching_ticket, timeline_row_remeasure_range, timeline_text_key_entry_id,
        tool_invocation::{
            AgentDetailItem, ToolInvocationDetail, ToolInvocationPreview,
            tool_inspection_evidence_for_test,
        },
    };
    use crate::database;
    use gpui::{AppContext as _, Context, Entity, TestAppContext, Window, WindowHandle};
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunStatus, AgentRunTriggerKind, AgentRuntimeSnapshot,
        AgentStoppedReason, ApprovalRequestPayload, AttachmentKind, AttachmentMetadata,
        AttachmentSource, AttachmentStorageKind, ContentPart, ConversationChange,
        ConversationChanges, ConversationEntry, ConversationEntryKind, ConversationEntryPayload,
        ConversationEntryStatus, ConversationMetadata, ConversationSettingsSnapshot,
        ConversationStatusCode, ConversationStatusEntry, EntryChangeKind, ProjectKind,
        ProjectMetadata, ProviderRawPayload, ProviderSettingsPayload, RunSettingsSnapshot,
        StructuredOutput, ToolAccessKind, ToolAccessRequestPayload, ToolApprovalMode,
        ToolApprovalPolicy, ToolArguments, ToolExecutionPolicy, ToolInvocationId,
        ToolInvocationInput, ToolInvocationOutput, ToolInvocationStatus, ToolNameStrategy,
        ToolPolicySnapshot, ToolSource, TranscriptRole, conservative_model_capabilities,
    };
    use jaco_db::{
        AgentRunFinalEntry, FinishAgentRun, FreshStore, NewAgentRun, NewAttachment,
        NewConversation, NewConversationEntry, NewProject, NewToolInvocation,
        NewToolInvocationApproval,
    };

    struct ReloadToolFixture {
        conversation_id: String,
        agent_run_id: String,
        expanded_invocation_id: ToolInvocationId,
        collapsed_invocation_id: ToolInvocationId,
        inspectable_invocation_id: ToolInvocationId,
        lifecycle_entry_ids: Vec<String>,
    }

    struct AttachmentActionFixture {
        directory: tempfile::TempDir,
        attachment_id: String,
        model: Entity<ConversationModel>,
        page: WindowHandle<ConversationDetailPage>,
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
                            .any(|state| timeline_text_key_entry_id(&state.key) == lifecycle_id),
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
                            .any(|state| timeline_text_key_entry_id(&state.key) == lifecycle_id),
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

    #[gpui::test]
    fn attachment_save_picker_cancel_is_silent_and_duplicate_actions_are_deduplicated(
        cx: &mut TestAppContext,
    ) {
        let fixture = attachment_action_fixture(cx);
        let target = super::attachment_access::AttachmentActionTarget {
            attachment_id: fixture.attachment_id.clone(),
            kind: attachments::AttachmentCardKind::File,
        };
        let action = super::attachment_access::AttachmentAction::SaveCopy;

        update_attachment_page(&fixture, cx, |page, window, cx| {
            page.handle_attachment_action(target.clone(), action, window, cx);
            page.handle_attachment_action(target.clone(), action, window, cx);

            assert_eq!(page.attachment_actions_in_flight.len(), 1);
        });
        cx.run_until_parked();

        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        assert!(
            !cx.did_prompt_for_new_path(),
            "one duplicate action must not leave a second save picker pending"
        );

        cx.update(|cx| {
            let page = fixture.page.read(cx).unwrap();
            assert!(page.attachment_actions_in_flight.is_empty());
            assert!(matches!(
                page.attachment_access.get(&fixture.attachment_id),
                Some(super::attachment_access::AttachmentAccessState::Available(
                    _
                ))
            ));
        });
    }

    #[gpui::test]
    fn stale_attachment_action_result_is_rejected_by_record_revision_fence(
        cx: &mut TestAppContext,
    ) {
        let fixture = attachment_action_fixture(cx);
        let target = super::attachment_access::AttachmentActionTarget {
            attachment_id: fixture.attachment_id.clone(),
            kind: attachments::AttachmentCardKind::File,
        };
        let action = super::attachment_access::AttachmentAction::Open;
        let old_record = cx.update(|cx| {
            fixture
                .model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .and_then(|conversation| {
                    conversation
                        .attachments
                        .iter()
                        .find(|record| record.id == fixture.attachment_id)
                })
                .cloned()
                .expect("fixture attachment record")
        });

        let latest_path = fixture.directory.path().join("latest.txt");
        fs::write(&latest_path, b"latest attachment").unwrap();
        let mut latest_record = old_record.clone();
        latest_record.path = Some(latest_path.to_string_lossy().into_owned());
        latest_record.metadata.source = AttachmentSource::LocalFile {
            path: latest_path.to_string_lossy().into_owned(),
        };
        latest_record.updated_at = old_record.updated_at + time::Duration::seconds(1);

        cx.update(|cx| {
            fixture.model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![ConversationChange::AttachmentUpserted {
                        attachment: Box::new(latest_record.clone()),
                    }]),
                    cx,
                );
            });
        });
        let (stale_is_current, latest_is_current) =
            update_attachment_page(&fixture, cx, |page, _window, cx| {
                let stale_is_current =
                    page.attachment_record_is_current(&target, old_record.updated_at, cx);
                let latest_is_current =
                    page.attachment_record_is_current(&target, latest_record.updated_at, cx);
                page.attachment_actions_in_flight
                    .insert((target.clone(), action));
                page.refresh_after_stale_attachment_action((target.clone(), action), cx);
                (stale_is_current, latest_is_current)
            });
        assert!(!stale_is_current);
        assert!(latest_is_current);
        cx.run_until_parked();

        cx.update(|cx| {
            let page = fixture.page.read(cx).unwrap();
            let state = page
                .attachment_access
                .get(&fixture.attachment_id)
                .expect("latest attachment availability");
            assert!(matches!(
                state,
                super::attachment_access::AttachmentAccessState::Available(resolved)
                    if resolved.path() == fs::canonicalize(&latest_path).unwrap()
            ));
            assert!(page.attachment_actions_in_flight.is_empty());
        });
    }

    #[gpui::test]
    fn attachment_access_reprobes_when_same_id_kind_conflict_disappears_on_entry_change(
        cx: &mut TestAppContext,
    ) {
        let fixture = attachment_action_fixture(cx);
        let initial_entry = cx.update(|cx| {
            fixture
                .model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .and_then(|conversation| conversation.entries.first())
                .cloned()
                .expect("fixture message entry")
        });
        let conflict_entry = message_entry_with_content(
            initial_entry,
            vec![
                ContentPart::File {
                    attachment_id: fixture.attachment_id.clone(),
                },
                ContentPart::Attachment {
                    attachment_id: fixture.attachment_id.clone(),
                },
            ],
        );

        cx.update(|cx| {
            fixture.model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![ConversationChange::EntryUpdated {
                        entry: Box::new(conflict_entry.clone()),
                        kind: EntryChangeKind::Replaced,
                    }]),
                    cx,
                );
            });
        });
        cx.run_until_parked();

        // Establish the cached conflict state explicitly so this test remains focused on the
        // recovery transition even if the preceding EntryChanged event is delivered eagerly.
        update_attachment_page(&fixture, cx, |page, _window, cx| {
            page.sync_attachment_access(true, cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let page = fixture.page.read(cx).unwrap();
            assert!(matches!(
                page.attachment_access.get(&fixture.attachment_id),
                Some(
                    super::attachment_access::AttachmentAccessState::Unavailable(
                        super::attachment_access::AttachmentAccessProblem::KindMismatch
                    )
                )
            ));
        });

        let file_only_entry = message_entry_with_content(
            conflict_entry,
            vec![ContentPart::File {
                attachment_id: fixture.attachment_id.clone(),
            }],
        );
        cx.update(|cx| {
            fixture.model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![ConversationChange::EntryUpdated {
                        entry: Box::new(file_only_entry),
                        kind: EntryChangeKind::Replaced,
                    }]),
                    cx,
                );
            });
        });
        cx.run_until_parked();

        cx.update(|cx| {
            let page = fixture.page.read(cx).unwrap();
            assert!(matches!(
                page.attachment_access.get(&fixture.attachment_id),
                Some(super::attachment_access::AttachmentAccessState::Available(resolved))
                    if resolved.path().ends_with("source.txt")
            ));
        });
    }

    fn attachment_action_fixture(cx: &mut TestAppContext) -> AttachmentActionFixture {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.txt");
        fs::write(&source_path, b"fixture attachment").unwrap();
        let (conversation_id, attachment_id) =
            seed_attachment_action_fixture(directory.path(), &source_path);

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
            cx.new(|_| ConversationModel::new(conversation_id.clone(), executor))
        });
        cx.update(|cx| model.update(cx, |model, cx| model.refresh(cx)));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(matches!(
                model.read(cx).operation(),
                ConversationOperation::Ready(ready) if ready.data().is_some()
            ));
        });

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
        cx.run_until_parked();

        AttachmentActionFixture {
            directory,
            attachment_id,
            model,
            page,
        }
    }

    fn update_attachment_page<R>(
        fixture: &AttachmentActionFixture,
        cx: &mut TestAppContext,
        update: impl FnOnce(
            &mut ConversationDetailPage,
            &mut Window,
            &mut Context<ConversationDetailPage>,
        ) -> R,
    ) -> R {
        fixture.page.update(cx, update).unwrap()
    }

    fn message_entry_with_content(
        mut entry: ConversationEntry,
        content: Vec<ContentPart>,
    ) -> ConversationEntry {
        let ConversationEntryPayload::Message {
            content: entry_content,
            ..
        } = &mut entry.payload
        else {
            panic!("fixture entry must be a message")
        };
        *entry_content = content;
        entry
    }

    fn seed_attachment_action_fixture(data_dir: &Path, source_path: &Path) -> (String, String) {
        let store =
            FreshStore::open_or_create_initial(data_dir.join(jaco_db::DATABASE_FILE)).unwrap();
        let repository = store.repository();
        let project = repository
            .insert_project(NewProject {
                path: data_dir.to_string_lossy().into_owned(),
                display_name: "Attachment action test".to_string(),
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
                title: "Attachment action test".to_string(),
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
        let source = source_path.to_string_lossy().into_owned();
        let attachment = repository
            .insert_attachment(NewAttachment {
                id: jaco_core::new_id(),
                conversation_id: conversation.id.clone(),
                kind: AttachmentKind::File,
                storage_kind: AttachmentStorageKind::LocalFile,
                mime_type: Some("text/plain".to_string()),
                name: Some("source.txt".to_string()),
                path: Some(source.clone()),
                external_uri: None,
                provider_id: None,
                provider_file_id: None,
                sha256: None,
                size_bytes: Some(17),
                metadata: AttachmentMetadata {
                    source: AttachmentSource::LocalFile { path: source },
                    width: None,
                    height: None,
                    duration_ms: None,
                    preview_attachment_id: None,
                },
            })
            .unwrap();
        repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![
                        ContentPart::Text {
                            text: "before".to_string(),
                        },
                        ContentPart::File {
                            attachment_id: attachment.id.clone(),
                        },
                        ContentPart::Text {
                            text: "after".to_string(),
                        },
                    ],
                },
            })
            .unwrap();

        (conversation.id, attachment.id)
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
    fn streaming_text_state_rebuilds_when_its_background_baseline_is_stale() {
        assert!(requires_streaming_text_state_rebuild(
            false,
            false,
            &MessageTextUpdate::Append(" suffix"),
        ));
        assert!(requires_streaming_text_state_rebuild(
            true,
            true,
            &MessageTextUpdate::Replace,
        ));
        assert!(!requires_streaming_text_state_rebuild(
            true,
            true,
            &MessageTextUpdate::Append(" suffix"),
        ));
        assert!(!requires_streaming_text_state_rebuild(
            false,
            false,
            &MessageTextUpdate::Replace,
        ));
    }

    #[gpui::test]
    fn streaming_text_state_keeps_initial_chunk_after_first_append(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let state = cx.update(|cx| cx.new(|cx| message_text_view_state("你是", true, cx)));
        cx.run_until_parked();

        state.update(cx, |state, cx| state.push_str(" Grok 4.6", cx));
        cx.run_until_parked();
        state.update(cx, |state, cx| state.select_all(cx));

        state.read_with(cx, |state, _| {
            assert_eq!(state.selected_text().trim_end(), "你是 Grok 4.6");
        });
    }

    #[gpui::test]
    fn streaming_text_state_keeps_initial_chunk_when_appends_coalesce(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let state = cx.update(|cx| cx.new(|cx| message_text_view_state("你是", true, cx)));
        state.update(cx, |state, cx| state.push_str(" Grok 4.6", cx));
        cx.run_until_parked();
        state.update(cx, |state, cx| state.select_all(cx));

        state.read_with(cx, |state, _| {
            assert_eq!(state.selected_text().trim_end(), "你是 Grok 4.6");
        });
    }

    #[test]
    fn mixed_message_text_sources_use_distinct_stable_block_keys() {
        let entry = message_entry_for_text_state(vec![
            ContentPart::Text {
                text: "before".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
            ContentPart::Text {
                text: "after".to_string(),
            },
        ]);

        let sources = message_text_sources(&entry, &HashMap::new());

        assert_eq!(
            sources,
            vec![
                (
                    attachments::TimelineTextKey::MessageBlock {
                        entry_id: entry.id.clone(),
                        start_part_index: 0,
                    },
                    "before".to_string(),
                ),
                (
                    attachments::TimelineTextKey::MessageBlock {
                        entry_id: entry.id.clone(),
                        start_part_index: 2,
                    },
                    "after".to_string(),
                ),
            ]
        );
        assert_eq!(
            message_text_update(&sources[1].1, "after streamed"),
            MessageTextUpdate::Append(" streamed")
        );
    }

    #[test]
    fn structural_part_change_moves_only_to_new_message_block_keys() {
        let before = message_entry_for_text_state(vec![
            ContentPart::Text {
                text: "first".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
            ContentPart::Text {
                text: "second".to_string(),
            },
        ]);
        let after = message_entry_for_text_state(vec![
            ContentPart::Text {
                text: "first".to_string(),
            },
            ContentPart::Image {
                attachment_id: "missing-image".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
            ContentPart::Text {
                text: "second".to_string(),
            },
        ]);

        let before_keys = message_text_sources(&before, &HashMap::new())
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>();
        let after_keys = message_text_sources(&after, &HashMap::new())
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>();

        assert_eq!(before_keys[0], after_keys[0]);
        assert_ne!(before_keys[1], after_keys[1]);
        assert!(matches!(
            after_keys[1],
            attachments::TimelineTextKey::MessageBlock {
                start_part_index: 3,
                ..
            }
        ));
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

    fn message_entry_for_text_state(content: Vec<ContentPart>) -> ConversationEntry {
        ConversationEntry {
            id: "entry-mixed".to_string(),
            conversation_id: "conversation-1".to_string(),
            seq: 1,
            kind: ConversationEntryKind::Message,
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content,
            },
            search_text: "text only".to_string(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }
}
