use crate::{
    components::{
        add_conversation::add_conversation_dialog_with_messages,
        chat_input::{ChatInput, Pause, Send, input_state},
        message::{MessageView, MessageViewExt},
    },
    config::AiChatConfig,
    database::{Content, ConversationTemplate, Role, Status},
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    gpui_ext::WeakEntityResultExt,
    i18n::I18n,
    llm::{FetchRunner, adapter_by_name},
    store::AddConversationMessage,
    views::{
        message_preview::{MessagePreviewExt, open_message_preview_window},
        temporary::TemporaryView,
    },
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Disableable, IconName, Root, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    input::InputState,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    select::{SearchableVec, SelectState},
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

type OnEsc = Rc<dyn Fn(&Esc, &mut Window, &mut App) + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporaryMessage {
    pub id: usize,
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
            let template_detail = temporary_view.selected_item.as_ref()?.clone();
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
                if let Some(template_detail) = this.selected_item.as_ref() {
                    template_detail.update(cx, |this, cx| {
                        this.pause_message(message_id, cx);
                    });
                }
            });
        }
    }

    fn delete_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                if let Some(template_detail) = this.selected_item.as_ref() {
                    template_detail.update(cx, |this, _cx| {
                        this.messages.retain(|message| message.id != message_id);
                    });
                }
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
                temporary_view
                    .read(cx)
                    .selected_item
                    .as_ref()
                    .is_some_and(|detail| !detail.read(cx).has_running_task())
            })
    }

    fn resend_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let temporary_view = find_temporary_view(window, cx);
        if let Some(temporary_view) = temporary_view {
            temporary_view.update(cx, |this, cx| {
                if let Some(template_detail) = this.selected_item.as_ref() {
                    template_detail.update(cx, |this, cx| {
                        this.resend_message(message_id, window, cx);
                    });
                }
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
                if let Some(template_detail) = this.selected_item.as_ref() {
                    template_detail.update(cx, |this, _cx| {
                        if let Some(message) = this
                            .messages
                            .iter_mut()
                            .find(|message| message.id == self.id)
                        {
                            message.content = content;
                        }
                    });
                }
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
    template: ConversationTemplate,
    on_esc: OnEsc,
    messages: Vec<TemporaryMessage>,
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<SearchableVec<String>>>,
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

impl TemplateDetailView {
    pub fn new(
        template: &ConversationTemplate,
        on_esc: impl Fn(&Esc, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let input_state = input_state(window, cx);
        input_state.focus_handle(cx).focus(window);
        let _subscriptions = vec![];
        let extension_container = cx.global::<ExtensionContainer>();
        let all_extensions = extension_container.get_all_config();
        Self {
            focus_handle,
            template: template.clone(),
            on_esc: Rc::new(on_esc),
            messages: Vec::new(),
            input_state,
            _subscriptions,
            task: None,
            autoincrement_id: 0,
            extension_state: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(
                        all_extensions
                            .into_iter()
                            .map(|x| x.name)
                            .collect::<Vec<_>>(),
                    ),
                    None,
                    window,
                    cx,
                )
                .searchable(true)
            }),
        }
    }
    fn messages(&self) -> Vec<MessageView<TemporaryMessage>> {
        self.messages
            .iter()
            .cloned()
            .map(MessageView::new)
            .collect()
    }
    fn on_send_action(&mut self, _: &Send, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input_state.read(cx).value();

        let extension_name = self.extension_state.read(cx).selected_value().cloned();
        let span = span!(
            Level::INFO,
            "Fetch",
            send_content = text.to_string(),
            extension_name = extension_name.clone()
        );
        if self.can_start_task() && !text.is_empty() {
            let config = cx.global::<AiChatConfig>().clone();
            let extension_container = cx.global::<ExtensionContainer>().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                if let Err(err) = Self::fetch(
                    this,
                    &text,
                    extension_name.as_ref(),
                    &extension_container,
                    config,
                    cx,
                )
                .compat()
                .instrument(span)
                .await
                {
                    event!(Level::ERROR, "fetch failed: {}", err);
                };
            });
            self.task = Some(RunningTask::new(task));
        }
    }
    fn on_pause_action(&mut self, _: &Pause, _window: &mut Window, cx: &mut Context<Self>) {
        self.pause_running_task(cx);
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
                let _ = this.update_result(cx, |this, _cx| {
                    this.clear_running_task_for_message(Some(message_id));
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
                role: message.role,
                content: message.content,
                send_content: (*message.send_content).clone(),
                status: message.status,
                error: message.error,
            })
            .collect::<Vec<_>>();
        add_conversation_dialog_with_messages(None, Some(initial_messages), window, cx);
    }
    fn new_id(&mut self) -> usize {
        self.autoincrement_id += 1;
        self.autoincrement_id
    }
    fn add_message(
        &mut self,
        now: OffsetDateTime,
        role: Role,
        content: Content,
        send_content: Rc<serde_json::Value>,
        status: Status,
        error: Option<String>,
    ) -> &mut TemporaryMessage {
        let id = self.new_id();
        let message = TemporaryMessage {
            id,
            role,
            content,
            send_content,
            status,
            error,
            created_time: now,
            updated_time: now,
            start_time: now,
            end_time: now,
        };
        self.messages.push(message);
        self.messages.last_mut().unwrap()
    }
    fn on_message(&mut self, content: &str, message_id: usize) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.add_content(content);
        }
    }
    fn on_error(&mut self, message_id: usize, error: String) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.record_error(error);
        }
        self.clear_running_task_for_message(Some(message_id));
    }
    fn on_success(&mut self, message_id: usize) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Normal);
        }
        self.clear_running_task_for_message(Some(message_id));
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
    fn clear_running_task_for_message(&mut self, message_id: Option<usize>) {
        let should_clear = self.task.as_ref().is_some_and(|task| {
            message_id.is_none_or(|message_id| task.contains_message(message_id))
        });
        if should_clear {
            self.task = None;
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
        cx.notify();
    }
    async fn fetch(
        state: WeakEntity<Self>,
        text: &SharedString,
        extension_name: Option<&String>,
        extension_container: &ExtensionContainer,
        config: AiChatConfig,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        event!(Level::INFO, "temporary fetch");
        let request_text = text.to_string();
        let now = OffsetDateTime::now_utc();
        let Some(prepared) = Self::prepare_fetch(
            state.clone(),
            request_text,
            extension_name.map(|name| name.as_str()),
            extension_container,
            config,
            now,
            cx,
        )
        .await?
        else {
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
        let (adapter_name, request_body, error) = state.update_result(cx, |this, _cx| {
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
                Some(this.template.adapter.clone()),
                message.send_content.clone(),
                None,
            )
        })?;
        if let Some(err) = error {
            return Err(err);
        }
        let Some(adapter_name) = adapter_name else {
            return Err(AiChatError::GpuiError);
        };
        Self::stream_existing_message(state, config, adapter_name, request_body, message_id, cx)
            .await
    }

    async fn prepare_fetch(
        state: WeakEntity<Self>,
        request_text: String,
        extension_name: Option<&str>,
        extension_container: &ExtensionContainer,
        config: AiChatConfig,
        now: OffsetDateTime,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedTemporaryFetch>> {
        match extension_name {
            Some(extension_name) => {
                Self::prepare_extension_fetch(
                    state,
                    request_text,
                    extension_name,
                    extension_container,
                    config,
                    now,
                    cx,
                )
                .await
            }
            None => Self::prepare_plain_fetch(state, request_text, config, now, cx).map(Some),
        }
    }

    async fn prepare_extension_fetch(
        state: WeakEntity<Self>,
        request_text: String,
        extension_name: &str,
        extension_container: &ExtensionContainer,
        config: AiChatConfig,
        now: OffsetDateTime,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedTemporaryFetch>> {
        let user_message_id = state.update_in_result(cx, |this, window, cx| {
            this.input_state.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            this.add_message(
                now,
                Role::User,
                Content::Text(request_text.clone()),
                Rc::new(serde_json::json!({})),
                Status::Loading,
                None,
            )
            .id
        })?;
        state.update_result(cx, |this, _cx| {
            this.bind_running_task_messages(Some(user_message_id), None);
        })?;

        let Some(extension_runner) = extension_container
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
        let Some(content) = Runner::get_new_user_content(request_text, Some(extension_runner))
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
        (|| -> AiChatResult<PreparedTemporaryFetch> {
            let runner = Self::build_runner(
                state.clone(),
                config,
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
                this.add_message(
                    now,
                    Role::Assistant,
                    Content::Text(String::new()),
                    send_content.clone(),
                    Status::Loading,
                    None,
                )
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
        request_text: String,
        config: AiChatConfig,
        now: OffsetDateTime,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedTemporaryFetch> {
        let content = Content::Text(request_text);
        let runner = Self::build_runner(
            state.clone(),
            config,
            Role::User,
            content.send_content().to_string(),
            cx,
        )?;
        let send_content = runner.request_body.clone();
        let assistant_message_id = state.update_in_result(cx, |this, window, cx| {
            this.input_state.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            let user_message_id = this
                .add_message(
                    now,
                    Role::User,
                    content,
                    send_content.clone(),
                    Status::Normal,
                    None,
                )
                .id;
            let assistant_message_id = this
                .add_message(
                    now,
                    Role::Assistant,
                    Content::Text(String::new()),
                    send_content.clone(),
                    Status::Loading,
                    None,
                )
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
        user_message_role: Role,
        user_message_content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Runner> {
        state.read_with_result(cx, |this, _cx| {
            let adapter_name = this.template.adapter.clone();
            let request_body = build_request_body(
                &adapter_name,
                &this.template.template,
                &this.template.prompts,
                &this.messages,
                user_message_role,
                &user_message_content,
            )?;
            Ok(Runner {
                config,
                adapter_name,
                request_body: Rc::new(request_body),
            })
        })?
    }

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
                    state.update_result(cx, |this, _cx| {
                        this.on_error(assistant_message_id, error.to_string());
                    })?;
                    return Ok(());
                }
            }
        }
        state.update_result(cx, |this, _cx| {
            this.on_success(assistant_message_id);
        })?;
        Ok(())
    }

    async fn stream_existing_message(
        state: WeakEntity<Self>,
        config: AiChatConfig,
        adapter_name: String,
        request_body: Rc<serde_json::Value>,
        assistant_message_id: usize,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let adapter = adapter_by_name(&adapter_name)?;
        let settings = config
            .get_adapter_settings(adapter.name())
            .ok_or(AiChatError::AdapterSettingsNotFound(
                adapter.name().to_string(),
            ))?
            .clone();
        let stream = adapter.fetch_by_request_body(config, settings, request_body.as_ref());
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
                    state.update_result(cx, |this, _cx| {
                        this.on_error(assistant_message_id, error.to_string());
                    })?;
                    return Ok(());
                }
            }
        }
        state.update_result(cx, |this, _cx| {
            this.on_success(assistant_message_id);
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
        state.update_result(cx, |this, _cx| {
            this.on_error(user_message_id, error);
        })?;
        Ok(())
    }
}

struct PreparedTemporaryFetch {
    runner: Runner,
    assistant_message_id: usize,
}

struct Runner {
    config: AiChatConfig,
    adapter_name: String,
    request_body: Rc<serde_json::Value>,
}

impl FetchRunner for Runner {
    fn get_adapter(&self) -> &str {
        &self.adapter_name
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
    adapter_name: &str,
    template: &serde_json::Value,
    prompts: &[crate::database::ConversationTemplatePrompt],
    messages: &[TemporaryMessage],
    user_message_role: Role,
    user_message_content: &str,
) -> AiChatResult<serde_json::Value> {
    let history =
        build_history_messages(prompts, messages, user_message_role, user_message_content);
    adapter_by_name(adapter_name)?.request_body(template, history)
}

impl Render for TemplateDetailView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let on_esc = self.on_esc.clone();
        let (clear_tooltip, save_tooltip) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("tooltip-clear-conversation"),
                i18n.t("tooltip-save-conversation"),
            )
        };
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(move |action, window, cx| {
                (on_esc)(action, window, cx);
            })
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
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Label::new(&self.template.icon))
                            .child(Label::new(&self.template.name).text_xl().when_some(
                                self.template.description.as_ref(),
                                |this, description| this.secondary(description),
                            )),
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
            .child(
                div()
                    .w_full()
                    .flex_initial()
                    .child(
                        ChatInput::new(&self.input_state, &self.extension_state)
                            .running(self.has_running_task())
                            .on_action(cx.listener(Self::on_send_action))
                            .on_action(cx.listener(Self::on_pause_action)),
                    )
                    .px_2(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{TemporaryMessage, build_history_messages};
    use crate::database::{Content, ConversationTemplatePrompt, Role, Status};
    use std::rc::Rc;
    use time::OffsetDateTime;

    fn make_message(role: Role, status: Status, content: Content) -> TemporaryMessage {
        let now = OffsetDateTime::now_utc();
        TemporaryMessage {
            id: 1,
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
}
