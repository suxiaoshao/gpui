use crate::{
    components::{
        add_conversation::add_conversation_dialog_with_messages,
        chat_form::{ChatForm, ChatFormEvent, ChatFormSnapshot},
        message::{MessageView, MessageViewExt},
    },
    config::AiChatConfig,
    database::{Content, Mode, Role, Status},
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    gpui_ext::WeakEntityResultExt,
    hotkey::TemporaryData,
    i18n::I18n,
    llm::{FetchRunner, provider_by_name},
    store::AddConversationMessage,
    views::{
        message_preview::{MessagePreviewExt, open_message_preview_window},
        temporary::TemporaryView,
    },
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, Root, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    v_flex,
};
use smol::stream::StreamExt;
use std::{any::TypeId, rc::Rc};
use time::OffsetDateTime;
use tracing::{Instrument, Level, event, span};

actions!([Esc]);

const CONTEXT: &str = "template-detail";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Esc, Some(CONTEXT))]);
}

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
        let message = find_temporary_view(window, cx).and_then(|temporary_view| {
            let temporary_view = temporary_view.read(cx);
            let template_detail = temporary_view.detail.clone();
            let template_detail = template_detail.read(cx);
            template_detail
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
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                this.detail.update(cx, |this, cx| {
                    this.pause_message(message_id, cx);
                });
            });
        }
    }

    fn delete_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                this.detail.update(cx, |this, _cx| {
                    this.messages.retain(|message| message.id != message_id);
                });
            });
        } else {
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
        }
    }

    fn can_resend(&self, cx: &App) -> bool {
        if self.role != Role::Assistant {
            return false;
        }
        cx.windows()
            .into_iter()
            .find_map(|window| {
                window.downcast::<Root>().filter(|root| {
                    root.read(cx)
                        .ok()
                        .map(|root| root.view().entity_type() == TypeId::of::<TemporaryView>())
                        .unwrap_or(false)
                })
            })
            .and_then(|root| {
                root.read(cx)
                    .ok()
                    .and_then(|root| root.view().clone().downcast::<TemporaryView>().ok())
            })
            .is_some_and(|temporary_view| {
                !temporary_view.read(cx).detail.read(cx).has_running_task()
            })
    }

    fn resend_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                this.detail.update(cx, |this, cx| {
                    this.resend_message(message_id, window, cx);
                });
            });
        }
    }
}

impl MessagePreviewExt for TemporaryMessage {
    fn on_update_content(
        &self,
        content: Content,
        window: &mut Window,
        cx: &mut App,
    ) -> AiChatResult<()> {
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                this.detail.update(cx, |this, _cx| {
                    if let Some(message) = this
                        .messages
                        .iter_mut()
                        .find(|message| message.id == self.id)
                    {
                        message.content = content;
                    }
                });
            });
        }
        Ok(())
    }
}

impl TemporaryMessage {
    fn add_content(&mut self, content: &str) {
        let now = OffsetDateTime::now_utc();
        self.content += content;
        self.updated_time = now;
        self.end_time = now;
    }
    fn resolve_extension(&mut self, content: Content, send_content: Rc<serde_json::Value>) {
        let now = OffsetDateTime::now_utc();
        self.content = content;
        self.send_content = send_content;
        self.status = Status::Normal;
        self.error = None;
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
        self.content = Content::Text(String::new());
        self.status = Status::Loading;
        self.error = None;
        self.updated_time = now;
        self.start_time = now;
        self.end_time = now;
    }
}

fn find_temporary_view(window: &mut Window, cx: &App) -> Option<Entity<TemporaryView>> {
    let root = window.root::<Root>()??;
    let root = root.read(cx);
    let view = root.view().downgrade().upgrade()?;
    view.downcast::<TemporaryView>().ok()
}

pub(crate) struct TemplateDetailView {
    focus_handle: FocusHandle,
    messages: Vec<TemporaryMessage>,
    chat_form: Entity<ChatForm>,
    _subscriptions: Vec<Subscription>,
    task: Option<RunningTask>,
    autoincrement_id: usize,
}

struct RunningTask {
    user_message_id: Option<usize>,
    assistant_message_id: Option<usize>,
    _task: Task<()>,
}

impl RunningTask {
    fn new(task: Task<()>) -> Self {
        Self {
            user_message_id: None,
            assistant_message_id: None,
            _task: task,
        }
    }

    fn bind_messages(
        &mut self,
        user_message_id: Option<usize>,
        assistant_message_id: Option<usize>,
    ) {
        self.user_message_id = user_message_id;
        self.assistant_message_id = assistant_message_id;
    }

    fn contains_message(&self, message_id: usize) -> bool {
        self.user_message_id == Some(message_id) || self.assistant_message_id == Some(message_id)
    }

    fn message_ids(&self) -> [Option<usize>; 2] {
        [self.user_message_id, self.assistant_message_id]
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

// Initializes the temporary conversation view and template-backed form state.
impl TemplateDetailView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let chat_form = cx.new(|cx| ChatForm::new(window, cx));
        let _subscriptions = vec![cx.subscribe_in(
            &chat_form,
            window,
            |this, _chat_form, event: &ChatFormEvent, window, cx| match event {
                ChatFormEvent::SendRequested => this.on_send_requested(window, cx),
                ChatFormEvent::PauseRequested => this.on_pause_requested(cx),
            },
        )];
        Self {
            focus_handle,
            messages: Vec::new(),
            chat_form,
            _subscriptions,
            task: None,
            autoincrement_id: 0,
        }
    }
}

// Exposes the current temporary messages for rendering.
impl TemplateDetailView {
    fn messages(&self) -> Vec<MessageView<TemporaryMessage>> {
        self.messages
            .iter()
            .cloned()
            .map(MessageView::new)
            .collect()
    }
}

// Handles user-facing actions for sending, pausing, and saving drafts.
impl TemplateDetailView {
    fn on_send_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = match self.chat_form.read(cx).snapshot(cx) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return,
            Err(err) => {
                event!(Level::ERROR, "collect chat form snapshot failed: {}", err);
                return;
            }
        };
        let span = span!(
            Level::INFO,
            "Fetch",
            send_content = snapshot.text.clone(),
            extension_name = snapshot.extension_name.clone()
        );
        if self.can_start_task() {
            let config = cx.global::<AiChatConfig>().clone();
            let extension_container = cx.global::<ExtensionContainer>().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let context = TemporaryFetchContext {
                    composer_snapshot: snapshot,
                    extension_container,
                    config,
                    now: OffsetDateTime::now_utc(),
                };
                if let Err(err) = Self::fetch(this, context, cx)
                    .compat()
                    .instrument(span)
                    .await
                {
                    event!(Level::ERROR, "fetch failed: {}", err);
                };
            });
            self.task = Some(RunningTask::new(task));
            self.chat_form
                .update(cx, |chat_form, cx| chat_form.set_running(true, cx));
        }
    }
    fn on_pause_requested(&mut self, cx: &mut Context<Self>) {
        self.pause_running_task(cx);
    }
    fn on_escape(&mut self, _: &Esc, window: &mut Window, cx: &mut Context<Self>) {
        TemporaryData::hide_with_delay(window, cx);
    }
    fn has_running_task(&self) -> bool {
        self.task.is_some()
    }
    fn can_start_task(&self) -> bool {
        !self.has_running_task()
    }
    fn pause_message(&mut self, message_id: usize, cx: &mut Context<Self>) {
        if !self
            .task
            .as_ref()
            .is_some_and(|task| task.contains_message(message_id))
        {
            return;
        }
        self.pause_running_task(cx);
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
        let mut running_task = RunningTask::new(task);
        running_task.bind_messages(None, Some(message_id));
        self.task = Some(running_task);
    }
    fn on_clear_conversation(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.messages.clear();
        self.autoincrement_id = 0;
        cx.notify();
    }
    fn on_save_conversation(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial_messages = self
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
        add_conversation_dialog_with_messages(None, Some(initial_messages), window, cx);
    }
}

// Manages the in-memory temporary messages and active running task.
impl TemplateDetailView {
    fn new_id(&mut self) -> usize {
        self.autoincrement_id += 1;
        self.autoincrement_id
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
        self.messages.push(message);
        self.messages.last_mut().unwrap()
    }
    fn on_message(&mut self, content: &str, message_id: usize) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.add_content(content);
        }
    }
    fn on_error(&mut self, message_id: usize, error: String, cx: &mut Context<Self>) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.record_error(error);
        }
        self.clear_running_task_for_message(Some(message_id), cx);
    }
    fn on_success(&mut self, message_id: usize, cx: &mut Context<Self>) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Normal);
        }
        self.clear_running_task_for_message(Some(message_id), cx);
    }
    fn bind_running_task_messages(
        &mut self,
        user_message_id: Option<usize>,
        assistant_message_id: Option<usize>,
    ) {
        if let Some(task) = self.task.as_mut() {
            task.bind_messages(user_message_id, assistant_message_id);
        }
    }
    fn clear_running_task_for_message(
        &mut self,
        message_id: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let should_clear = self.task.as_ref().is_some_and(|task| {
            message_id.is_none_or(|message_id| task.contains_message(message_id))
        });
        if should_clear {
            self.task = None;
            self.chat_form
                .update(cx, |chat_form, cx| chat_form.set_running(false, cx));
        }
    }
    fn pause_running_task(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.task.take() else {
            return;
        };
        for message_id in task.message_ids().into_iter().flatten() {
            if let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == message_id)
                && matches!(message.status, Status::Loading)
            {
                message.update_status(Status::Paused);
            }
        }
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.set_running(false, cx));
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
        let Some(prepared) = Self::prepare_fetch(state.clone(), &context, cx).await? else {
            return Ok(());
        };
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
    ) -> AiChatResult<Option<PreparedTemporaryFetch>> {
        match context.composer_snapshot.extension_name.as_deref() {
            Some(extension_name) => {
                Self::prepare_extension_fetch(state, context, extension_name, cx).await
            }
            None => Self::prepare_plain_fetch(state, context, cx).map(Some),
        }
    }

    async fn prepare_extension_fetch(
        state: WeakEntity<Self>,
        context: &TemporaryFetchContext,
        extension_name: &str,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedTemporaryFetch>> {
        let user_message_id = state.update_in_result(cx, |this, window, cx| {
            this.chat_form
                .update(cx, |chat_form, cx| chat_form.clear_input(window, cx));
            this.add_message(NewTemporaryMessage {
                now: context.now,
                provider: context.composer_snapshot.provider_name.clone(),
                role: Role::User,
                content: Content::Text(context.composer_snapshot.text.clone()),
                send_content: Rc::new(serde_json::json!({})),
                status: Status::Loading,
                error: None,
            })
            .id
        })?;
        state.update_result(cx, |this, _cx| {
            this.bind_running_task_messages(Some(user_message_id), None);
        })?;

        let Some(extension_runner) = context
            .extension_container
            .get_extension(extension_name)
            .await
            .map(Some)
            .or_else(|error| {
                Self::record_extension_error(
                    state.clone(),
                    user_message_id,
                    extension_name,
                    error,
                    cx,
                )
                .map(|()| None)
            })?
        else {
            return Ok(None);
        };
        let Some(content) = Runner::get_new_user_content(
            context.composer_snapshot.text.clone(),
            Some(extension_runner),
        )
        .await
        .map(Some)
        .or_else(|error| {
            Self::record_extension_error(state.clone(), user_message_id, extension_name, error, cx)
                .map(|()| None)
        })?
        else {
            return Ok(None);
        };
        (|| -> AiChatResult<PreparedTemporaryFetch> {
            let runner = Self::build_runner(
                state.clone(),
                context.config.clone(),
                context.composer_snapshot.clone(),
                Role::User,
                content.send_content().to_string(),
                cx,
            )?;
            let send_content = runner.request_body.clone();
            let assistant_message_id = state.update_result(cx, |this, _cx| {
                if let Some(message) = this
                    .messages
                    .iter_mut()
                    .find(|message| message.id == user_message_id)
                {
                    message.resolve_extension(content, send_content.clone());
                }
                this.add_message(NewTemporaryMessage {
                    now: context.now,
                    provider: context.composer_snapshot.provider_name.clone(),
                    role: Role::Assistant,
                    content: Content::Text(String::new()),
                    send_content: send_content.clone(),
                    status: Status::Loading,
                    error: None,
                })
                .id
            })?;
            state.update_result(cx, |this, _cx| {
                this.bind_running_task_messages(Some(user_message_id), Some(assistant_message_id));
            })?;
            Ok(PreparedTemporaryFetch {
                runner,
                assistant_message_id,
            })
        })()
        .map(Some)
        .or_else(|error| {
            Self::record_extension_error(state, user_message_id, extension_name, error, cx)
                .map(|()| None)
        })
    }

    fn prepare_plain_fetch(
        state: WeakEntity<Self>,
        context: &TemporaryFetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedTemporaryFetch> {
        let content = Content::Text(context.composer_snapshot.text.clone());
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
                    content: Content::Text(String::new()),
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
                &this.messages,
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

// Applies streamed assistant output and records extension failures.
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
                Ok(message) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_message(&message, assistant_message_id);
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
                Ok(message) => {
                    state.update_result(cx, |this, _cx| {
                        this.on_message(&message, assistant_message_id);
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

    fn record_extension_error(
        state: WeakEntity<Self>,
        user_message_id: usize,
        extension_name: &str,
        error: AiChatError,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let error = format!("extension {extension_name}: {error}");
        state.update_result(cx, |this, cx| {
            this.on_error(user_message_id, error, cx);
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
    extension_container: ExtensionContainer,
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

    fn get_config(&self) -> &crate::config::AiChatConfig {
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

// Renders the temporary conversation header, message list, and composer.
impl Render for TemplateDetailView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let (title, subtitle, clear_tooltip, save_tooltip) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("temporary-chat-title"),
                i18n.t("temporary-chat-description"),
                i18n.t("tooltip-clear-conversation"),
                i18n.t("tooltip-save-conversation"),
            )
        };
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_escape))
            .size_full()
            .overflow_hidden()
            .pb_2()
            .child(
                h_flex()
                    .flex_initial()
                    .p_2()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex().gap_1().child(Label::new(title).text_xl()).child(
                            Label::new(subtitle)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("temporary-clear")
                                    .icon(IconName::Delete)
                                    .ghost()
                                    .small()
                                    .disabled(self.has_running_task())
                                    .on_click(cx.listener(Self::on_clear_conversation))
                                    .tooltip(clear_tooltip),
                            )
                            .child(
                                Button::new("temporary-save")
                                    .icon(IconName::Inbox)
                                    .ghost()
                                    .small()
                                    .disabled(self.has_running_task())
                                    .on_click(cx.listener(Self::on_save_conversation))
                                    .tooltip(save_tooltip),
                            ),
                    ),
            )
            .child(Divider::horizontal())
            .child(
                div()
                    .id("template-detail-content")
                    .flex_1()
                    .overflow_hidden()
                    .children(self.messages())
                    .child(div().h_2())
                    .overflow_y_scrollbar(),
            )
            .child({
                let mut footer = v_flex();
                footer.style().align_items = Some(AlignItems::Stretch);
                footer
                    .w_full()
                    .flex_initial()
                    .px_2()
                    .child(self.chat_form.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{TemporaryMessage, build_history_messages, build_request_body};
    use crate::database::{Content, ConversationTemplatePrompt, Mode, Role, Status};
    use std::rc::Rc;
    use time::OffsetDateTime;

    fn make_message(role: Role, status: Status, content: Content) -> TemporaryMessage {
        let now = OffsetDateTime::now_utc();
        TemporaryMessage {
            id: 1,
            provider: "OpenAI".to_string(),
            role,
            content,
            send_content: Rc::new(serde_json::json!({})),
            status,
            error: None,
            created_time: now,
            updated_time: now,
            start_time: now,
            end_time: now,
        }
    }

    #[test]
    fn runner_history_appends_current_user_message() {
        let history = build_history_messages(
            &[ConversationTemplatePrompt {
                prompt: "system".to_string(),
                role: Role::Developer,
            }],
            Mode::Contextual,
            &[
                make_message(
                    Role::Assistant,
                    Status::Normal,
                    Content::Text("a1".to_string()),
                ),
                make_message(Role::User, Status::Error, Content::Text("bad".to_string())),
            ],
            Role::User,
            "latest",
        )
        .into_iter()
        .map(|message| (message.role, message.content))
        .collect::<Vec<_>>();

        assert_eq!(
            history,
            vec![
                (Role::Developer, "system".to_string()),
                (Role::Assistant, "a1".to_string()),
                (Role::User, "latest".to_string()),
            ]
        );
    }

    #[test]
    fn build_request_body_uses_override_template_model() -> anyhow::Result<()> {
        let mut template = serde_json::json!({
            "model": "gpt-4o",
            "stream": false,
            "temperature": 1.0,
            "top_p": 1.0,
            "n": 1,
            "max_completion_tokens": null,
            "presence_penalty": 0.0,
            "frequency_penalty": 0.0
        });
        template["model"] = serde_json::json!("override-model");
        let request_body = build_request_body(
            "OpenAI",
            &template,
            &[],
            Mode::Contextual,
            &[],
            Role::User,
            "hello",
        )?;
        assert_eq!(request_body["model"], serde_json::json!("override-model"));
        Ok(())
    }
}
