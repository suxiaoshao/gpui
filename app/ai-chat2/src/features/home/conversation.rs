pub(crate) mod format;
mod message;
mod timeline;

use std::{collections::HashMap, path::Path};

use ai_chat_core::{AgentRunId, ConversationId};
use ai_chat_db::AgentRunRecord;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    label::Label,
    list::{List, ListState},
    notification::{Notification, NotificationType},
    v_flex,
};
use tracing::{Level, event};

use crate::{
    foundation::I18n,
    state::{self, conversations::ConversationLoadSnapshot},
};

use super::chat_form::{ChatForm, ChatFormEvent, ChatFormSubmit};

pub(crate) struct ConversationPage {
    conversation_id: ConversationId,
    snapshot: Result<Option<ConversationLoadSnapshot>, String>,
    chat_form: Entity<ChatForm>,
    timeline: Entity<ListState<timeline::ConversationTimelineDelegate>>,
    expanded_agent_runs: HashMap<AgentRunId, bool>,
    runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationPage {
    pub(crate) fn new(
        conversation_id: ConversationId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let chat_form = cx.new(|cx| ChatForm::new(window, cx));
        let runtime = state::conversation_runtime::runtime(cx);
        let snapshot = load_snapshot(&conversation_id, cx);
        let page = cx.entity().downgrade();
        let (on_toggle, on_copy) = timeline::callback_pair(
            {
                let page = page.clone();
                move |agent_run_id, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.toggle_agent_run(agent_run_id.clone(), window, cx);
                    });
                }
            },
            copy_to_clipboard,
        );
        let rows = snapshot
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map(|snapshot| timeline::build_rows(snapshot, &HashMap::new(), on_toggle, on_copy))
            .unwrap_or_default();
        let timeline = cx.new(|cx| {
            ListState::new(
                timeline::ConversationTimelineDelegate::new(rows),
                window,
                cx,
            )
        });
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |page, _chat_form, event: &ChatFormEvent, window, cx| match event {
                ChatFormEvent::SendRequested(submit) => {
                    page.submit_message((**submit).clone(), window, cx);
                }
                ChatFormEvent::AddRequested => {}
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
            snapshot,
            chat_form,
            timeline,
            expanded_agent_runs: HashMap::new(),
            runtime,
            _subscriptions: vec![chat_form_subscription, runtime_subscription],
        };
        page.refresh_chat_form_context(cx);
        page.sync_submit_blocked(cx);
        page
    }

    fn submit_message(
        &mut self,
        submit: ChatFormSubmit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.runtime.read(cx).is_running(&self.conversation_id) {
            return;
        }
        let request = state::conversations::SendConversationMessageRequest {
            conversation_id: self.conversation_id.clone(),
            content_parts: submit.composer.content_parts.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
        };
        match state::conversations::send_conversation_message(request, cx) {
            Ok(sent) => {
                let _ = &sent.item.id;
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.clear_after_submit(cx);
                });
                self.reload(window, cx);
                state::workspace::workspace(cx).update(cx, |workspace, cx| {
                    workspace.reload_sidebar(cx);
                });
                self.runtime.update(cx, |runtime, cx| {
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
        }
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
            | state::conversation_runtime::ConversationRuntimeEvent::RunFinished {
                conversation_id,
            } => conversation_id,
        };
        if event_conversation_id != &self.conversation_id {
            return;
        }

        self.reload(window, cx);
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

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.snapshot = load_snapshot(&self.conversation_id, cx);
        self.refresh_chat_form_context(cx);
        self.sync_timeline(window, cx);
        self.sync_submit_blocked(cx);
        cx.notify();
    }

    fn refresh_chat_form_context(&mut self, cx: &mut Context<Self>) {
        let project_path = self
            .snapshot
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map(|snapshot| snapshot.project.path.clone());
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(project_path.as_deref().map(Path::new), cx);
        });
    }

    fn sync_timeline(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let (on_toggle, on_copy) = timeline::callback_pair(
            {
                let page = page.clone();
                move |agent_run_id, window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.toggle_agent_run(agent_run_id.clone(), window, cx);
                    });
                }
            },
            copy_to_clipboard,
        );
        let rows = self
            .snapshot
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map(|snapshot| {
                timeline::build_rows(snapshot, &self.expanded_agent_runs, on_toggle, on_copy)
            })
            .unwrap_or_default();
        self.timeline.update(cx, |timeline, cx| {
            timeline.delegate_mut().set_rows(rows);
            cx.notify();
        });
    }

    fn sync_submit_blocked(&mut self, cx: &mut Context<Self>) {
        let running = self.runtime.read(cx).is_running(&self.conversation_id);
        let tooltip = running.then(|| {
            cx.global::<I18n>()
                .t("conversation-send-disabled-running")
                .into()
        });
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.set_submit_blocked(running, tooltip, cx);
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
        self.expanded_agent_runs.insert(agent_run_id, !current);
        self.sync_timeline(window, cx);
        cx.notify();
    }

    fn default_agent_run_expanded(&self, agent_run_id: &AgentRunId) -> bool {
        let Some(snapshot) = self.snapshot.as_ref().ok().and_then(Option::as_ref) else {
            return true;
        };
        let Some(run) = snapshot.runs.iter().find(|run| &run.id == agent_run_id) else {
            return true;
        };
        if !format::is_terminal_run(run) {
            return true;
        }
        !final_item_exists(run, snapshot)
    }

    fn render_missing(&self, cx: &mut Context<Self>) -> AnyElement {
        let (title, subtitle) = match &self.snapshot {
            Err(error) => (
                cx.global::<I18n>().t("conversation-load-failed"),
                error.clone(),
            ),
            Ok(None) => (
                cx.global::<I18n>().t("conversation-missing-title"),
                cx.global::<I18n>().t("conversation-missing-subtitle"),
            ),
            Ok(Some(_)) => return div().into_any_element(),
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
            .into_any_element()
    }
}

impl Render for ConversationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !matches!(self.snapshot, Ok(Some(_))) {
            return self.render_missing(cx);
        }

        v_flex()
            .id("ai-chat2-conversation-page")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(List::new(&self.timeline).large().flex_1()),
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

fn load_snapshot(
    conversation_id: &ConversationId,
    cx: &App,
) -> Result<Option<ConversationLoadSnapshot>, String> {
    state::conversations::load_conversation(conversation_id, cx).map_err(|err| {
        event!(Level::ERROR, error = ?err, conversation_id, "load conversation failed");
        err.to_string()
    })
}

fn final_item_exists(run: &AgentRunRecord, snapshot: &ConversationLoadSnapshot) -> bool {
    let Some(final_item_id) = run
        .output
        .as_ref()
        .and_then(|output| output.final_item_id.as_ref())
    else {
        return snapshot.items.iter().any(|item| {
            item.agent_run_id.as_ref() == Some(&run.id) && format::is_assistant_message(item)
        });
    };
    snapshot.items.iter().any(|item| &item.id == final_item_id)
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
