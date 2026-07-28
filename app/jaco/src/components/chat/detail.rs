mod attachments;
mod message;
mod timeline;
mod tool_blocks;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
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
        ChatFormSkillCompletionPlacement, ChatInputController, ChatInputEvent, ChatInputSubmit,
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
    runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
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
            chat_form
                .set_skill_completion_placement(ChatFormSkillCompletionPlacement::AboveForm, cx);
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
            runtime,
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
        if self.chat_form.read(cx).submission_pending(cx)
            || self.runtime.read(cx).is_running(&self.conversation_id)
        {
            return;
        }
        if !matches!(
            self.conversation.read(cx).operation(),
            ConversationOperation::Ready(ready) if ready.data().is_some()
        ) {
            return;
        }
        let request = conversation::SendConversationMessageRequest {
            conversation_id: self.conversation_id.clone(),
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.attachments.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
        };
        let task = conversation::send_conversation_message(request, cx);
        let page = cx.entity().downgrade();
        let completion = window.spawn(cx, async move |cx| {
            let result = task.await;
            let _ = page.update_in(cx, |page, window, cx| {
                page.chat_form.update(cx, |chat_form, cx| {
                    chat_form.finish_submission(cx);
                });
                match result {
                    Ok(sent) => {
                        page.chat_form.update(cx, |chat_form, cx| {
                            chat_form.clear_after_submit(window, cx);
                        });
                        page.timeline.set_follow_mode(FollowMode::Tail);
                        page.timeline.scroll_to_end();
                        let start = page
                            .runtime
                            .update(cx, |runtime, cx| runtime.start_run(sent.run_request, cx));
                        if let Err(error) = start {
                            let title = cx.global::<I18n>().t("conversation-run-failed");
                            push_conversation_notification(
                                window,
                                cx,
                                title,
                                error,
                                NotificationType::Error,
                            );
                        }
                    }
                    Err(err) => {
                        let title = cx.global::<I18n>().t("conversation-send-failed");
                        push_conversation_notification(
                            window,
                            cx,
                            title,
                            err.to_string(),
                            NotificationType::Error,
                        );
                    }
                }
            });
        });
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.begin_submission(completion, cx);
        });
    }

    fn handle_runtime_event(
        &mut self,
        runtime: &Entity<conversation::runtime::ConversationRuntimeStore>,
        event: &conversation::runtime::ConversationRuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let event_conversation_id = match event {
            conversation::runtime::ConversationRuntimeEvent::RunStarted { conversation_id }
            | conversation::runtime::ConversationRuntimeEvent::RunFinished { conversation_id } => {
                conversation_id
            }
        };
        if event_conversation_id != &self.conversation_id {
            return;
        }

        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_primary_action(cx);
        });
        cx.notify();
        if matches!(
            event,
            conversation::runtime::ConversationRuntimeEvent::RunFinished { .. }
        ) {
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
    }

    fn handle_conversation_model_event(
        &mut self,
        event: &ConversationModelEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ConversationModelEvent::Reloaded => {
                self.refresh_chat_form_context(cx);
                self.sync_message_text_states(cx);
                self.sync_timeline(cx, None);
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
            ConversationEffect::ProviderStepChanged { .. }
            | ConversationEffect::ToolInvocationChanged { .. } => {}
            ConversationEffect::Deleted => {
                self.message_text_states.clear();
                self.sync_timeline(cx, None);
            }
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.conversation
            .update(cx, |conversation, cx| conversation.refresh(cx));
    }

    fn refresh_chat_form_context(&mut self, cx: &mut Context<Self>) {
        let project_path = self
            .conversation
            .read(cx)
            .operation()
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| snapshot.project.path.clone());
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(project_path.as_deref().map(Path::new), cx);
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
                timeline::build_rows(
                    snapshot,
                    active_agent_run_id.as_ref(),
                    &self.expanded_agent_runs,
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
            .map(format::item_markdown);
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
        let Some((entry, attachments)) = self
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
                    .cloned()
                    .map(|entry| (entry, conversation.attachments.clone()))
            })
        else {
            self.sync_timeline(cx, None);
            return;
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

    fn remeasure_timeline_row(&self, key: &message::TimelineRowKey) {
        if let Some(row_ix) = self
            .timeline_rows
            .keys()
            .iter()
            .position(|current| current == key)
        {
            self.timeline.remeasure_items(row_ix..row_ix + 1);
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
                            .child(self.chat_form.clone()),
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

#[cfg(test)]
mod tests {
    use super::{MessageTextUpdate, message_text_update};

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
