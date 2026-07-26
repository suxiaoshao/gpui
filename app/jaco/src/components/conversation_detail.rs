mod attachments;
mod message;
mod timeline;
mod tool_blocks;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
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
use gpui_operation::{Complete, Load, Refresh, Retry, Transition};
use jaco_core::{AgentRunId, ConversationEntryId, ConversationId, ToolInvocationId};

use crate::{
    components::chat_input::{
        ChatFormSkillCompletionPlacement, ChatInputController, ChatInputEvent, ChatInputSubmit,
    },
    foundation::{I18n, conversation_format as format},
    state::{self, conversations::ConversationLoadSnapshot},
};

pub(crate) struct ConversationDetailPage {
    conversation_id: ConversationId,
    operation: state::conversations::ConversationTimelineOperation,
    pending_changes: Vec<jaco_db::ConversationChange>,
    chat_form: Entity<ChatInputController>,
    timeline: ListState,
    timeline_rows: timeline::ConversationTimelineRows,
    message_text_states: Vec<MessageTextState>,
    expanded_agent_runs: HashMap<AgentRunId, bool>,
    runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
    _subscriptions: Vec<Subscription>,
}

struct MessageTextState {
    id: ConversationEntryId,
    state: Entity<TextViewState>,
    source: String,
    _subscription: Subscription,
}

#[derive(Debug, PartialEq, Eq)]
enum MessageTextUpdate<'a> {
    Unchanged,
    Append(&'a str),
    Replace,
}

struct TimelineCommit {
    conversation: Option<jaco_db::ConversationRecord>,
    changes: Vec<jaco_db::ConversationChange>,
}

impl Transition<TimelineCommit> for &mut Option<ConversationLoadSnapshot> {
    type Output = ();

    fn transition(self, commit: TimelineCommit) {
        let Some(snapshot) = self.as_mut() else {
            return;
        };
        if let Some(conversation) = commit.conversation {
            snapshot.conversation = conversation;
        }
        for change in commit.changes {
            match change {
                jaco_db::ConversationChange::EntryAppended { entry }
                | jaco_db::ConversationChange::EntryUpdated { entry } => {
                    match snapshot.items.iter_mut().find(|item| item.id == entry.id) {
                        Some(current) => *current = entry,
                        None => snapshot.items.push(entry),
                    }
                    snapshot.items.sort_by(|left, right| {
                        left.seq
                            .cmp(&right.seq)
                            .then_with(|| left.id.cmp(&right.id))
                    });
                }
                jaco_db::ConversationChange::ToolInvocationChanged { invocation } => match snapshot
                    .tool_invocations
                    .iter_mut()
                    .find(|current| current.id == invocation.id)
                {
                    Some(current) => *current = invocation,
                    None => snapshot.tool_invocations.push(invocation),
                },
                jaco_db::ConversationChange::RunStatusChanged { run } => {
                    match snapshot
                        .runs
                        .iter_mut()
                        .find(|current| current.id == run.id)
                    {
                        Some(current) => *current = run,
                        None => snapshot.runs.push(run),
                    }
                }
                jaco_db::ConversationChange::ProviderStepChanged { .. } => {}
            }
        }
    }
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
        conversation_id: ConversationId,
        runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_focus(conversation_id, runtime, true, window, cx)
    }

    pub(crate) fn new_without_focus(
        conversation_id: ConversationId,
        runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_focus(conversation_id, runtime, false, window, cx)
    }

    fn new_with_focus(
        conversation_id: ConversationId,
        runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
        focus_composer: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let chat_form = cx.new(|cx| {
            let mut chat_form = if focus_composer {
                ChatInputController::new(window, cx)
            } else {
                ChatInputController::new_without_focus(window, cx)
            };
            chat_form
                .set_skill_completion_placement(ChatFormSkillCompletionPlacement::AboveForm, cx);
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
            |page,
             runtime,
             event: &state::conversation_runtime::ConversationRuntimeEvent,
             window,
             cx| {
                page.handle_runtime_event(runtime, event, window, cx);
            },
        );

        let mut page = Self {
            conversation_id,
            operation: state::conversations::ConversationTimelineOperation::new(),
            pending_changes: Vec::new(),
            chat_form,
            timeline,
            timeline_rows,
            message_text_states: Vec::new(),
            expanded_agent_runs: HashMap::new(),
            runtime,
            _subscriptions: vec![chat_form_subscription, runtime_subscription],
        };
        page.reload(window, cx);
        page.refresh_chat_form_context(cx);
        page.sync_message_text_states(cx);
        page.sync_timeline(window, cx, None);
        page.timeline.scroll_to_end();
        page.sync_agent_running(cx);
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
        if self.runtime.read(cx).is_running(&self.conversation_id) {
            return;
        }
        let request = state::conversations::SendConversationMessageRequest {
            conversation_id: self.conversation_id.clone(),
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.attachments.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
        };
        let task = state::conversations::send_conversation_message(request, cx);
        let page = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                let result = task.await;
                let _ = page.update_in(cx, |page, window, cx| match result {
                    Ok(sent) => {
                        page.chat_form.update(cx, |chat_form, cx| {
                            chat_form.clear_after_submit(window, cx);
                        });
                        let runtime = page.runtime.clone();
                        page.handle_runtime_event(
                            &runtime,
                            &state::conversation_runtime::ConversationRuntimeEvent::ConversationChanges {
                                conversation_id: sent.conversation.id.clone(),
                                conversation: Some(Box::new(sent.conversation)),
                                changes: vec![jaco_db::ConversationChange::EntryAppended {
                                    entry: sent.item,
                                }],
                            },
                            window,
                            cx,
                        );
                        page.timeline.set_follow_mode(FollowMode::Tail);
                        page.timeline.scroll_to_end();
                        page.runtime.update(cx, |runtime, cx| {
                            runtime.start_run(sent.run_request, window, cx);
                        });
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
                });
            })
            .detach();
    }

    fn handle_runtime_event(
        &mut self,
        runtime: &Entity<state::conversation_runtime::ConversationRuntimeStore>,
        event: &state::conversation_runtime::ConversationRuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let event_conversation_id = match event {
            state::conversation_runtime::ConversationRuntimeEvent::RunStarted {
                conversation_id,
            }
            | state::conversation_runtime::ConversationRuntimeEvent::ConversationChanged {
                conversation_id,
            }
            | state::conversation_runtime::ConversationRuntimeEvent::ConversationChanges {
                conversation_id,
                ..
            }
            | state::conversation_runtime::ConversationRuntimeEvent::RunFinished {
                conversation_id,
            } => conversation_id,
        };
        if event_conversation_id != &self.conversation_id {
            return;
        }

        match event {
            state::conversation_runtime::ConversationRuntimeEvent::ConversationChanges {
                conversation,
                changes,
                ..
            } => {
                if let state::conversations::ConversationTimelineOperation::Ready(ready) =
                    &mut self.operation
                {
                    ready.transition(TimelineCommit {
                        conversation: conversation.as_deref().cloned(),
                        changes: changes.clone(),
                    });
                    self.refresh_chat_form_context(cx);
                    self.sync_message_text_states(cx);
                    self.sync_timeline(window, cx, None);
                    cx.notify();
                } else {
                    self.pending_changes.extend(changes.iter().cloned());
                }
            }
            state::conversation_runtime::ConversationRuntimeEvent::ConversationChanged {
                ..
            } => {
                self.reload(window, cx);
            }
            state::conversation_runtime::ConversationRuntimeEvent::RunStarted { .. }
            | state::conversation_runtime::ConversationRuntimeEvent::RunFinished { .. } => {
                self.sync_agent_running(cx);
                cx.notify();
            }
        }
        if matches!(
            event,
            state::conversation_runtime::ConversationRuntimeEvent::RunFinished { .. }
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

    fn apply_pending_changes(&mut self) {
        if self.pending_changes.is_empty() {
            return;
        }
        if self.operation.data().and_then(Option::as_ref).is_none() {
            return;
        }
        let changes = std::mem::take(&mut self.pending_changes);
        if let state::conversations::ConversationTimelineOperation::Ready(ready) =
            &mut self.operation
        {
            ready.transition(TimelineCommit {
                conversation: None,
                changes,
            });
        }
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.is_running() {
            return;
        }
        let Ok(executor) = crate::database::ready_executor(cx) else {
            return;
        };
        let conversation_id = self.conversation_id.clone();
        let completion_id = conversation_id.clone();
        let page = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = executor
                .execute(move |repository| {
                    repository.conversation_timeline_records(&conversation_id)
                })
                .await
                .map_err(state::conversations::ConversationTimelineProblem::from);
            let _ = page.update_in(cx, |page, window, cx| {
                if page.conversation_id != completion_id || !page.operation.is_running() {
                    return;
                }
                page.operation.transition(Complete(result));
                page.apply_pending_changes();
                page.refresh_chat_form_context(cx);
                page.sync_message_text_states(cx);
                page.sync_timeline(window, cx, None);
                page.sync_agent_running(cx);
                cx.notify();
            });
        });
        match &mut self.operation {
            state::conversations::ConversationTimelineOperation::Idle(_) => {
                self.operation.transition(Load(task))
            }
            state::conversations::ConversationTimelineOperation::Ready(_)
            | state::conversations::ConversationTimelineOperation::Degraded(_) => {
                self.operation.transition(Refresh(task))
            }
            state::conversations::ConversationTimelineOperation::Unavailable(_) => {
                self.operation.transition(Retry(task))
            }
            state::conversations::ConversationTimelineOperation::Loading(_)
            | state::conversations::ConversationTimelineOperation::Refreshing(_)
            | state::conversations::ConversationTimelineOperation::Retrying(_)
            | state::conversations::ConversationTimelineOperation::RefreshingDegraded(_) => {}
        }
        self.refresh_chat_form_context(cx);
        self.sync_message_text_states(cx);
        self.sync_timeline(window, cx, None);
        self.sync_agent_running(cx);
        cx.notify();
    }

    fn refresh_chat_form_context(&mut self, cx: &mut Context<Self>) {
        let project_path = self
            .operation
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| snapshot.project.path.clone());
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(project_path.as_deref().map(Path::new), cx);
        });
    }

    fn sync_timeline(
        &mut self,
        _window: &mut Window,
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
            .operation
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
            .operation
            .data()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                snapshot
                    .items
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

    fn sync_agent_running(&mut self, cx: &mut Context<Self>) {
        let running = self.runtime.read(cx).is_running(&self.conversation_id);
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.set_agent_running(running, cx);
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .expanded_agent_runs
            .get(&agent_run_id)
            .copied()
            .unwrap_or_else(|| self.default_agent_run_expanded(&agent_run_id));
        self.timeline.set_follow_mode(FollowMode::Normal);
        self.expanded_agent_runs
            .insert(agent_run_id.clone(), !current);
        self.sync_timeline(
            window,
            cx,
            Some(message::TimelineRowKey::Agent(agent_run_id)),
        );
        cx.notify();
    }

    fn default_agent_run_expanded(&self, agent_run_id: &AgentRunId) -> bool {
        let Some(snapshot) = self.operation.data().and_then(Option::as_ref) else {
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
        let (title, subtitle) = match self.operation.data() {
            None if self.operation.problem().is_some() => (
                cx.global::<I18n>().t("conversation-load-failed"),
                self.operation
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
            .children(self.operation.is_running().then(|| Spinner::new().small()))
            .child(
                Button::new("conversation-retry-load")
                    .label(cx.global::<I18n>().t("resource-status-refresh"))
                    .disabled(self.operation.is_running())
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.reload(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_stale_status(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let problem = self.operation.problem().map(ToString::to_string);
        let running = self.operation.is_running();
        (!matches!(
            self.operation,
            state::conversations::ConversationTimelineOperation::Ready(_)
        ))
        .then(|| {
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
                        .on_click(cx.listener(|page, _, window, cx| {
                            page.reload(window, cx);
                        })),
                )
                .into_any_element()
        })
    }
}

impl Render for ConversationDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.operation.data().and_then(Option::as_ref).is_none() {
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
