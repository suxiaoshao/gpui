use crate::{
    components::{
        add_conversation::open_add_conversation_dialog,
        chat_form::ChatFormSnapshot,
        delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
        message::MessageViewExt,
    },
    database::{Content, Mode, Role, Status},
    errors::{AiChatError, AiChatResult},
    hotkey::GlobalHotkeyState,
    i18n::I18n,
    llm::{FetchRunner, FetchUpdate, provider_by_name},
    platform::gpui_ext::WeakEntityResultExt,
    state::{AddConversationMessage, AiChatConfig},
    views::{
        conversation::{
            detail::{ConversationDetailView, ConversationDetailViewExt, DetailEscape},
            preview::{MessagePreviewExt, open_message_preview_window},
        },
        temporary::TemporaryView,
    },
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::*;
use gpui_component::{
    ActiveTheme, Root, WindowExt,
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use smol::stream::StreamExt;
use std::rc::Rc;
use time::OffsetDateTime;
use tracing::{Instrument, Level, event, span};

const CONTEXT: &str = "template-detail";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", DetailEscape, Some(CONTEXT))]);
}

pub(crate) type TemplateDetailView = ConversationDetailView<TemporaryDetailState>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporaryMessage {
    pub id: usize,
    pub provider: String,
    pub role: Role,
    pub content: Content,
    pub send_content: Rc<serde_json::Value>,
    pub status: Status,
    pub error: Option<String>,
    pub created_time: OffsetDateTime,
    pub updated_time: OffsetDateTime,
    pub start_time: OffsetDateTime,
    pub end_time: OffsetDateTime,
}

impl MessageViewExt for TemporaryMessage {
    type Id = usize;

    fn role(&self) -> &Role {
        &self.role
    }

    fn content(&self) -> &Content {
        &self.content
    }

    fn send_content(&self) -> &serde_json::Value {
        self.send_content.as_ref()
    }

    fn status(&self) -> &Status {
        &self.status
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn id(&self) -> Self::Id {
        self.id
    }

    fn open_view_by_id(id: Self::Id, window: &mut Window, cx: &mut App) {
        let message = find_temporary_detail(window, cx).and_then(|template_detail| {
            let template_detail = template_detail.read(cx);
            template_detail
                .detail
                .messages
                .iter()
                .find(|message| message.id == id)
                .cloned()
        });
        let Some(message) = message else {
            return;
        };
        open_message_preview_window(message, cx);
    }

    fn pause_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let template_detail = find_temporary_detail(window, cx);
        if let Some(template_detail) = template_detail {
            template_detail.update(cx, |this, cx| {
                this.pause_message(message_id, cx);
            });
        }
    }

    fn delete_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let Some(template_detail) = find_temporary_detail(window, cx) else {
            let (title, message) = {
                let i18n = cx.global::<I18n>();
                (
                    i18n.t("delete-message-failed-title"),
                    i18n.t("delete-message-failed-message"),
                )
            };
            window.push_notification(
                Notification::new()
                    .title(title)
                    .message(message)
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        };

        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-delete-temporary-message-title"),
                i18n.t("dialog-delete-temporary-message-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Delete,
            move |_window, cx| {
                template_detail.update(cx, |this, cx| {
                    this.detail
                        .messages
                        .retain(|message| message.id != message_id);
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }

    fn can_resend(&self, cx: &App) -> bool {
        if self.role != Role::Assistant {
            return false;
        }
        cx.windows()
            .iter()
            .find_map(|window| {
                let root = window.downcast::<Root>()?;
                let view = root.read(cx).ok()?.view().clone();
                let temporary_view = view.downcast::<TemporaryView>().ok()?;
                Some(temporary_view.read(cx).detail.clone())
            })
            .is_some_and(|detail| !detail.read(cx).has_running_task())
    }

    fn resend_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let Some(template_detail) = find_temporary_detail(window, cx) else {
            return;
        };
        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-regenerate-message-title"),
                i18n.t("dialog-regenerate-message-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Regenerate,
            move |window, cx| {
                template_detail.update(cx, |this, cx| {
                    this.resend_message(message_id, window, cx);
                });
            },
            window,
            cx,
        );
    }
}

impl MessagePreviewExt for TemporaryMessage {
    fn provider_name(&self) -> &str {
        &self.provider
    }

    fn created_time(&self) -> OffsetDateTime {
        self.created_time
    }

    fn updated_time(&self) -> OffsetDateTime {
        self.updated_time
    }

    fn start_time(&self) -> OffsetDateTime {
        self.start_time
    }

    fn end_time(&self) -> OffsetDateTime {
        self.end_time
    }

    fn on_update_content(
        &self,
        content: Content,
        window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()> {
        let template_detail = find_temporary_detail(window, cx);
        if let Some(template_detail) = template_detail {
            template_detail.update(cx, |this, _cx| {
                if let Some(message) = this
                    .detail
                    .messages
                    .iter_mut()
                    .find(|message| message.id == self.id)
                {
                    message.content = content;
                }
            });
        }
        Ok(())
    }

    fn set_content(&mut self, content: Content) {
        self.content = content;
    }
}

impl TemporaryMessage {
    fn add_content(&mut self, content: &str) {
        let now = OffsetDateTime::now_utc();
        if matches!(self.status, Status::Thinking) {
            self.status = Status::Loading;
        }
        self.content += content;
        self.updated_time = now;
        self.end_time = now;
    }
    fn add_reasoning_summary(&mut self, delta: &str) {
        let now = OffsetDateTime::now_utc();
        if !matches!(self.status, Status::Loading | Status::Normal) {
            self.status = Status::Thinking;
        }
        self.content.append_reasoning_summary(delta);
        self.updated_time = now;
        self.end_time = now;
    }
    fn update_status(&mut self, status: Status) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.updated_time = now;
        self.end_time = now;
    }
    fn record_error(&mut self, error: String) {
        self.update_status(Status::Error);
        self.error = Some(error);
    }
    fn reset_for_resend(&mut self) {
        let now = OffsetDateTime::now_utc();
        self.content = Content::default();
        self.status = Status::Loading;
        self.error = None;
        self.updated_time = now;
        self.start_time = now;
        self.end_time = now;
    }
}

fn find_temporary_detail(window: &Window, cx: &App) -> Option<Entity<TemplateDetailView>> {
    let root = window.root::<Root>()??;
    let root = root.read(cx);
    let view = root.view().downgrade().upgrade()?;
    if let Ok(temporary_view) = view.clone().downcast::<TemporaryView>() {
        return Some(temporary_view.read(cx).detail.clone());
    }
    None
}

#[derive(Clone)]
pub(crate) struct TemporaryDetailState {
    messages: Vec<TemporaryMessage>,
    autoincrement_id: usize,
}

impl TemporaryDetailState {
    fn clear_messages(&mut self) {
        self.messages.clear();
        self.autoincrement_id = 0;
    }
}

struct NewTemporaryMessage {
    now: OffsetDateTime,
    provider: String,
    role: Role,
    content: Content,
    send_content: Rc<serde_json::Value>,
    status: Status,
    error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TemporaryMessageRevision {
    id: usize,
    status: Status,
    updated_time_nanos: i128,
    content_len: usize,
    error_len: usize,
}

impl TemporaryMessageRevision {
    fn new(message: &TemporaryMessage) -> Self {
        let content_len = message.content.text.len()
            + message
                .content
                .reasoning_summary
                .as_ref()
                .map_or(0, String::len)
            + message
                .content
                .citations
                .iter()
                .map(|citation| citation.url.len())
                .sum::<usize>();

        Self {
            id: message.id,
            status: message.status,
            updated_time_nanos: message.updated_time.unix_timestamp_nanos(),
            content_len,
            error_len: message.error.as_ref().map_or(0, String::len),
        }
    }
}

impl TemplateDetailView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_state(
            TemporaryDetailState {
                messages: Vec::new(),
                autoincrement_id: 0,
            },
            window,
            cx,
        )
    }

    pub(crate) fn new_with_state(
        state: TemporaryDetailState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        ConversationDetailView::new_with_detail(state, window, cx)
    }
}

impl ConversationDetailViewExt for TemporaryDetailState {
    type Message = TemporaryMessage;
    type MessageId = usize;
    type Revision = TemporaryMessageRevision;

    fn title(&self, cx: &App) -> SharedString {
        cx.global::<I18n>().t("temporary-chat-title").into()
    }

    fn subtitle(&self, cx: &App) -> Option<SharedString> {
        Some(cx.global::<I18n>().t("temporary-chat-description").into())
    }

    fn empty_state(&self, cx: &App) -> Option<AnyElement> {
        Some(
            v_flex()
                .items_center()
                .gap_2()
                .max_w(px(360.))
                .text_center()
                .child(Label::new(cx.global::<I18n>().t("temporary-chat-empty-title")).text_lg())
                .child(
                    Label::new(cx.global::<I18n>().t("temporary-chat-empty-description"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element(),
        )
    }

    fn key_context(&self) -> Option<&'static str> {
        Some(CONTEXT)
    }

    fn focus_on_init(&self) -> bool {
        true
    }

    fn element_prefix(&self) -> SharedString {
        "temporary-detail".into()
    }

    fn message_revisions(&self, _cx: &App) -> Vec<Self::Revision> {
        self.messages
            .iter()
            .map(TemporaryMessageRevision::new)
            .collect()
    }

    fn message_at(&self, index: usize, _cx: &App) -> Option<Self::Message> {
        self.messages.get(index).cloned()
    }

    fn on_send_requested(
        view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        let snapshot = match view.chat_form.read(cx).snapshot(cx) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return,
            Err(err) => {
                event!(Level::ERROR, "collect chat form snapshot failed: {}", err);
                return;
            }
        };
        let span = span!(Level::INFO, "Fetch", send_content = snapshot.text.clone());
        if view.can_start_task() {
            let config = cx.global::<AiChatConfig>().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let context = TemporaryFetchContext {
                    composer_snapshot: snapshot,
                    config,
                    now: OffsetDateTime::now_utc(),
                };
                if let Err(err) = TemplateDetailView::fetch(this, context, cx)
                    .compat()
                    .instrument(span)
                    .await
                {
                    event!(Level::ERROR, "fetch failed: {}", err);
                };
            });
            view.set_running_task(task, cx);
        }
    }

    fn on_pause_requested(
        view: &mut ConversationDetailView<Self>,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        view.pause_running_task(cx);
    }

    fn on_escape(
        _view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        GlobalHotkeyState::request_hide_with_delay(window, cx);
    }

    fn supports_clear(&self) -> bool {
        !self.messages.is_empty()
    }

    fn clear(
        _view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        let detail = cx.entity().downgrade();
        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-clear-temporary-chat-title"),
                i18n.t("dialog-clear-temporary-chat-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Clear,
            move |_window, cx| {
                let _ = detail.update(cx, |view, cx| {
                    view.detail.clear_messages();
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }

    fn supports_save(&self) -> bool {
        !self.messages.is_empty()
    }

    fn save(
        view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        let initial_messages = view
            .detail
            .messages
            .iter()
            .cloned()
            .map(|message| AddConversationMessage {
                provider: message.provider,
                role: message.role,
                content: message.content,
                send_content: (*message.send_content).clone(),
                status: message.status,
                error: message.error,
            })
            .collect::<Vec<_>>();
        open_add_conversation_dialog(None, Some(initial_messages), window, cx);
    }
}

impl TemplateDetailView {
    fn pause_message(&mut self, message_id: usize, cx: &mut Context<Self>) {
        if self
            .task
            .as_ref()
            .is_some_and(|task| task.contains_message(message_id))
        {
            self.pause_running_task(cx);
            return;
        }

        if let Some(message) = self
            .detail
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
            && matches!(message.status, Status::Loading | Status::Thinking)
        {
            message.update_status(Status::Paused);
            self.set_chat_form_running(false, cx);
            cx.notify();
        }
    }

    fn resend_message(&mut self, message_id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_running_task() {
            return;
        }
        let config = cx.global::<AiChatConfig>().clone();
        let span = span!(Level::INFO, "TemporaryResendMessage", message_id);
        let task = cx.spawn_in(window, async move |this, cx| {
            let state = this.clone();
            if let Err(err) = Self::fetch_existing_assistant_message(state, config, message_id, cx)
                .compat()
                .instrument(span)
                .await
            {
                event!(Level::ERROR, "temporary resend message failed: {}", err);
                let _ = this.update_result(cx, |this, cx| {
                    this.clear_running_task_for_message(Some(message_id), cx);
                });
            }
        });
        self.set_running_task(task, cx);
        self.bind_running_task_messages(None, Some(message_id));
    }
}

impl TemplateDetailView {
    fn new_id(&mut self) -> usize {
        self.detail.autoincrement_id += 1;
        self.detail.autoincrement_id
    }
    fn add_message(&mut self, input: NewTemporaryMessage) -> &mut TemporaryMessage {
        let id = self.new_id();
        let message = TemporaryMessage {
            id,
            provider: input.provider,
            role: input.role,
            content: input.content,
            send_content: input.send_content,
            status: input.status,
            error: input.error,
            created_time: input.now,
            updated_time: input.now,
            start_time: input.now,
            end_time: input.now,
        };
        self.detail.messages.push(message);
        self.detail.messages.last_mut().unwrap()
    }
    fn on_message(&mut self, content: &str, message_id: usize) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            last.add_content(content);
        }
    }
    fn on_complete_content(&mut self, content: Content, message_id: usize) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            let now = OffsetDateTime::now_utc();
            last.content = content;
            last.updated_time = now;
            last.end_time = now;
        }
    }
    fn on_reasoning_summary(&mut self, delta: &str, message_id: usize) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            last.add_reasoning_summary(delta);
        }
    }
    fn on_thinking(&mut self, message_id: usize) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Thinking);
        }
    }
    fn on_error(&mut self, message_id: usize, error: String, cx: &mut Context<Self>) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            last.record_error(error);
        }
        self.clear_running_task_for_message(Some(message_id), cx);
    }
    fn on_success(&mut self, message_id: usize, cx: &mut Context<Self>) {
        if let Some(last) = self.detail.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Normal);
        }
        self.clear_running_task_for_message(Some(message_id), cx);
    }
    fn pause_running_task(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.task.take() else {
            return;
        };
        for message_id in task.message_ids().into_iter().flatten() {
            if let Some(message) = self
                .detail
                .messages
                .iter_mut()
                .find(|message| message.id == message_id)
                && matches!(message.status, Status::Loading | Status::Thinking)
            {
                message.update_status(Status::Paused);
            }
        }
        self.set_chat_form_running(false, cx);
        cx.notify();
    }
}

// Prepares request state and coordinates async fetch execution.
impl TemplateDetailView {
    async fn fetch(
        state: WeakEntity<Self>,
        context: TemporaryFetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        event!(Level::INFO, "temporary fetch");
        let prepared = Self::prepare_fetch(state.clone(), &context, cx).await?;
        Self::stream_fetch(state, prepared.runner, prepared.assistant_message_id, cx).await?;
        Ok(())
    }

    async fn fetch_existing_assistant_message(
        state: WeakEntity<Self>,
        config: AiChatConfig,
        message_id: usize,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let (provider_name, request_body, error) = state.update_result(cx, |this, _cx| {
            let Some(message) = this
                .detail
                .messages
                .iter_mut()
                .find(|message| message.id == message_id)
            else {
                return (
                    None,
                    Rc::new(serde_json::Value::Null),
                    Some(AiChatError::StreamError(
                        "temporary assistant message not found".to_string(),
                    )),
                );
            };
            if message.role != Role::Assistant {
                return (
                    None,
                    Rc::new(serde_json::Value::Null),
                    Some(AiChatError::StreamError(
                        "temporary message is not assistant".to_string(),
                    )),
                );
            }
            message.reset_for_resend();
            (
                Some(message.provider.clone()),
                message.send_content.clone(),
                None,
            )
        })?;
        if let Some(err) = error {
            return Err(err);
        }
        let Some(provider_name) = provider_name else {
            return Err(AiChatError::GpuiError);
        };
        Self::stream_existing_message(state, config, provider_name, request_body, message_id, cx)
            .await
    }

    async fn prepare_fetch(
        state: WeakEntity<Self>,
        context: &TemporaryFetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedTemporaryFetch> {
        Self::prepare_plain_fetch(state, context, cx)
    }

    fn prepare_plain_fetch(
        state: WeakEntity<Self>,
        context: &TemporaryFetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedTemporaryFetch> {
        let content = Content::new(context.composer_snapshot.text.clone());
        let runner = Self::build_runner(
            state.clone(),
            context.config.clone(),
            context.composer_snapshot.clone(),
            Role::User,
            content.send_content().to_string(),
            cx,
        )?;
        let send_content = runner.request_body.clone();
        let assistant_message_id = state.update_in_result(cx, |this, window, cx| {
            this.chat_form
                .update(cx, |chat_form, cx| chat_form.clear_input(window, cx));
            let user_message_id = this
                .add_message(NewTemporaryMessage {
                    now: context.now,
                    provider: context.composer_snapshot.provider_name.clone(),
                    role: Role::User,
                    content,
                    send_content: send_content.clone(),
                    status: Status::Normal,
                    error: None,
                })
                .id;
            let assistant_message_id = this
                .add_message(NewTemporaryMessage {
                    now: context.now,
                    provider: context.composer_snapshot.provider_name.clone(),
                    role: Role::Assistant,
                    content: Content::default(),
                    send_content: send_content.clone(),
                    status: Status::Loading,
                    error: None,
                })
                .id;
            this.bind_running_task_messages(Some(user_message_id), Some(assistant_message_id));
            assistant_message_id
        })?;
        Ok(PreparedTemporaryFetch {
            runner,
            assistant_message_id,
        })
    }

    fn build_runner(
        state: WeakEntity<Self>,
        config: AiChatConfig,
        composer_snapshot: ChatFormSnapshot,
        user_message_role: Role,
        user_message_content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Runner> {
        state.read_with_result(cx, |this, _cx| {
            let request_body = build_request_body(
                &composer_snapshot.provider_name,
                &composer_snapshot.request_template,
                &composer_snapshot.prompts,
                composer_snapshot.mode,
                &this.detail.messages,
                user_message_role,
                &user_message_content,
            )?;
            Ok(Runner {
                config,
                provider_name: composer_snapshot.provider_name.clone(),
                request_body: Rc::new(request_body),
            })
        })?
    }
}

// Applies streamed assistant output.
impl TemplateDetailView {
    async fn stream_fetch(
        state: WeakEntity<Self>,
        runner: Runner,
        assistant_message_id: usize,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let stream = runner.fetch();
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(FetchUpdate::ThinkingStarted) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_thinking(assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::ReasoningSummaryDelta(delta)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_reasoning_summary(&delta, assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::TextDelta(message)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_message(&message, assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::Complete(content)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_complete_content(content, assistant_message_id);
                    })?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    state.update_result(cx, |this, cx| {
                        this.on_error(assistant_message_id, error.to_string(), cx);
                    })?;
                    return Ok(());
                }
            }
        }
        state.update_result(cx, |this, cx| {
            this.on_success(assistant_message_id, cx);
        })?;
        Ok(())
    }

    async fn stream_existing_message(
        state: WeakEntity<Self>,
        config: AiChatConfig,
        provider_name: String,
        request_body: Rc<serde_json::Value>,
        assistant_message_id: usize,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let provider = provider_by_name(&provider_name)?;
        let settings = config
            .get_provider_settings(provider.name())
            .ok_or(AiChatError::ProviderSettingsNotFound(
                provider.name().to_string(),
            ))?
            .clone();
        let stream = provider.fetch_by_request_body(config, settings, request_body.as_ref());
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(FetchUpdate::ThinkingStarted) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_thinking(assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::ReasoningSummaryDelta(delta)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_reasoning_summary(&delta, assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::TextDelta(message)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_message(&message, assistant_message_id);
                    })?;
                }
                Ok(FetchUpdate::Complete(content)) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_complete_content(content, assistant_message_id);
                    })?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    state.update_result(cx, |this, cx| {
                        this.on_error(assistant_message_id, error.to_string(), cx);
                    })?;
                    return Ok(());
                }
            }
        }
        state.update_result(cx, |this, cx| {
            this.on_success(assistant_message_id, cx);
        })?;
        Ok(())
    }
}

struct PreparedTemporaryFetch {
    runner: Runner,
    assistant_message_id: usize,
}

struct TemporaryFetchContext {
    composer_snapshot: ChatFormSnapshot,
    config: AiChatConfig,
    now: OffsetDateTime,
}

struct Runner {
    config: AiChatConfig,
    provider_name: String,
    request_body: Rc<serde_json::Value>,
}

impl FetchRunner for Runner {
    fn get_provider(&self) -> &str {
        &self.provider_name
    }

    fn get_config(&self) -> &crate::state::AiChatConfig {
        &self.config
    }

    fn request_body(&self) -> &serde_json::Value {
        self.request_body.as_ref()
    }
}

fn build_history_messages(
    prompts: &[crate::database::ConversationTemplatePrompt],
    mode: Mode,
    messages: &[TemporaryMessage],
    user_message_role: Role,
    user_message_content: &str,
) -> Vec<crate::llm::Message> {
    use crate::llm::Message as FetchMessage;

    let mut request_messages = prompts
        .iter()
        .map(|prompt| FetchMessage::new(prompt.role, prompt.prompt.clone()))
        .collect::<Vec<_>>();

    request_messages.extend(
        messages
            .iter()
            .filter(|message| message.status == Status::Normal)
            .filter(|message| match mode {
                Mode::Contextual => true,
                Mode::Single => false,
                Mode::AssistantOnly => message.role == Role::Assistant,
            })
            .map(|message| {
                FetchMessage::new(message.role, message.content.send_content().to_string())
            }),
    );
    request_messages.push(FetchMessage::new(
        user_message_role,
        user_message_content.to_string(),
    ));
    request_messages
}

fn build_request_body(
    provider_name: &str,
    template: &serde_json::Value,
    prompts: &[crate::database::ConversationTemplatePrompt],
    mode: Mode,
    messages: &[TemporaryMessage],
    user_message_role: Role,
    user_message_content: &str,
) -> AiChatResult<serde_json::Value> {
    let history = build_history_messages(
        prompts,
        mode,
        messages,
        user_message_role,
        user_message_content,
    );
    provider_by_name(provider_name)?.request_body(template, history)
}

#[cfg(test)]
mod tests;
