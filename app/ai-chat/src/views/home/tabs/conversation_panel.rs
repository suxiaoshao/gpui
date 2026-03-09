use crate::{
    components::chat_form::{ChatForm, ChatFormEvent, ChatFormSnapshot},
    components::message::MessageView,
    config::AiChatConfig,
    database::{Content, Conversation, Db, Message, Mode, NewMessage, Role, Status},
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    gpui_ext::{AsyncWindowContextResultExt, EntityResultExt, WeakEntityResultExt},
    llm::{FetchRunner, provider_by_name},
    store::{ChatData, ChatDataInner},
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{
    AlignItems, AppContext, AsyncWindowContext, Context, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, SharedString, Styled, Subscription, Task,
    WeakEntity, Window, div, list, prelude::FluentBuilder, px,
};
use gpui_component::{h_flex, label::Label, scroll::ScrollableElement, v_flex};
use smol::stream::StreamExt;
use std::ops::Deref;
use time::OffsetDateTime;
use tracing::{Instrument, Level, event, span};

pub(crate) struct ConversationPanelView {
    conversation_id: i32,
    conversation_icon: SharedString,
    conversation_title: SharedString,
    conversation_info: Option<SharedString>,
    message_list: ListState,
    message_revisions: Vec<MessageRevision>,
    chat_form: Entity<ChatForm>,
    _subscriptions: Vec<Subscription>,
    task: Option<RunningTask>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MessageRevision {
    id: i32,
    status: Status,
    updated_time_nanos: i128,
    content_len: usize,
    error_len: usize,
}

impl MessageRevision {
    fn new(message: &Message) -> Self {
        let content_len = match &message.content {
            Content::Text(content) => content.len(),
            Content::Extension {
                source,
                extension_name,
                content,
            } => source.len() + extension_name.len() + content.len(),
        };

        Self {
            id: message.id,
            status: message.status,
            updated_time_nanos: message.updated_time.unix_timestamp_nanos(),
            content_len,
            error_len: message.error.as_ref().map_or(0, String::len),
        }
    }
}

struct RunningTask {
    user_message_id: Option<i32>,
    assistant_message_id: Option<i32>,
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

    fn bind_messages(&mut self, user_message_id: Option<i32>, assistant_message_id: Option<i32>) {
        self.user_message_id = user_message_id;
        self.assistant_message_id = assistant_message_id;
    }

    fn contains_message(&self, message_id: i32) -> bool {
        running_task_contains_message(self.user_message_id, self.assistant_message_id, message_id)
    }

    fn message_ids(&self) -> [Option<i32>; 2] {
        [self.user_message_id, self.assistant_message_id]
    }
}

fn running_task_contains_message(
    user_message_id: Option<i32>,
    assistant_message_id: Option<i32>,
    message_id: i32,
) -> bool {
    user_message_id == Some(message_id) || assistant_message_id == Some(message_id)
}

// Initializes the panel and keeps chat-form state in sync.
impl ConversationPanelView {
    pub fn new(conversation: &Conversation, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            conversation_id: conversation.id,
            conversation_icon: conversation.icon.clone().into(),
            conversation_title: conversation.title.clone().into(),
            conversation_info: conversation.info.clone().map(Into::into),
            message_list: ListState::new(0, ListAlignment::Top, px(1000.)),
            message_revisions: Vec::new(),
            chat_form,
            _subscriptions,
            task: None,
        }
    }

    fn sync_message_list(&mut self, next_revisions: Vec<MessageRevision>) {
        if self.message_list.item_count() != self.message_revisions.len() {
            self.message_list.reset(next_revisions.len());
            self.message_revisions = next_revisions;
            return;
        }

        if self.message_revisions == next_revisions {
            return;
        }

        let first_diff = self
            .message_revisions
            .iter()
            .zip(next_revisions.iter())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| self.message_revisions.len().min(next_revisions.len()));

        self.message_list.splice(
            first_diff..self.message_revisions.len(),
            next_revisions.len().saturating_sub(first_diff),
        );
        self.message_revisions = next_revisions;
    }
}

// Handles user-triggered conversation actions.
impl ConversationPanelView {
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
            let conversation_id = self.conversation_id;
            let chat_data = cx.global::<ChatData>().deref().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let state = this.clone();
                let context = FetchContext {
                    chat_data,
                    conversation_id,
                    composer_snapshot: snapshot,
                    extension_container,
                    config,
                };
                if let Err(err) = Self::fetch(state, context, cx)
                    .compat()
                    .instrument(span)
                    .await
                {
                    event!(Level::ERROR, "fetch failed: {}", err);
                    let _ = this.update_result(cx, |this, cx| {
                        this.clear_running_task_for_message(None, cx);
                    });
                }
            });
            self.task = Some(RunningTask::new(task));
            self.chat_form
                .update(cx, |chat_form, cx| chat_form.set_running(true, cx));
        }
    }

    fn on_pause_requested(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.pause_task(cx) {
            event!(Level::ERROR, "pause fetch failed: {}", err);
            cx.notify();
        }
    }

    fn pause_task(&mut self, cx: &mut Context<Self>) -> AiChatResult<()> {
        let Some(task) = self.task.take() else {
            return Ok(());
        };

        let conversation_id = self.conversation_id;
        let chat_data = cx.global::<ChatData>().deref().clone();
        let paused_messages = chat_data.update(cx, move |data, cx| {
            let mut paused_messages = Vec::new();
            if let Ok(data) = data {
                for message_id in task.message_ids().into_iter().flatten() {
                    let Some(mut message) = data.message(conversation_id, message_id) else {
                        continue;
                    };
                    if !matches!(message.status, Status::Loading) {
                        continue;
                    }
                    let now = OffsetDateTime::now_utc();
                    message.status = Status::Paused;
                    message.updated_time = now;
                    message.end_time = now;
                    data.replace_message(conversation_id, message.clone());
                    paused_messages.push(message);
                }
                cx.notify();
            }
            paused_messages
        });
        let conn = &mut cx.global::<Db>().get()?;
        for message in &paused_messages {
            Self::persist_message_snapshot(message, conn)?;
        }
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.set_running(false, cx));
        cx.notify();
        Ok(())
    }

    pub(crate) fn pause_message(&mut self, message_id: i32, cx: &mut Context<Self>) {
        if !self
            .task
            .as_ref()
            .is_some_and(|task| task.contains_message(message_id))
        {
            return;
        }
        if let Err(err) = self.pause_task(cx) {
            event!(Level::ERROR, "pause message failed: {}", err);
        }
    }

    pub(crate) fn resend_message(
        &mut self,
        message_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.has_running_task() {
            return;
        }

        let config = cx.global::<AiChatConfig>().clone();
        let chat_data = cx.global::<ChatData>().deref().clone();
        let conversation_id = self.conversation_id;
        let span = span!(Level::INFO, "ResendMessage", message_id, conversation_id);
        let task = cx.spawn_in(window, async move |this, cx| {
            let state = this.clone();
            if let Err(err) = Self::fetch_existing_assistant_message(
                state,
                chat_data,
                config,
                conversation_id,
                message_id,
                cx,
            )
            .compat()
            .instrument(span)
            .await
            {
                event!(Level::ERROR, "resend message failed: {}", err);
                let _ = this.update_result(cx, |this, cx| {
                    this.clear_running_task_for_message(Some(message_id), cx);
                });
            }
        });
        let mut running_task = RunningTask::new(task);
        running_task.bind_messages(None, Some(message_id));
        self.task = Some(running_task);
    }
}

// Tracks the active fetch task and its bound messages.
impl ConversationPanelView {
    pub(crate) fn has_running_task(&self) -> bool {
        self.task.is_some()
    }

    fn can_start_task(&self) -> bool {
        !self.has_running_task()
    }

    fn bind_running_task_messages(
        &mut self,
        user_message_id: Option<i32>,
        assistant_message_id: Option<i32>,
    ) {
        if let Some(task) = self.task.as_mut() {
            task.bind_messages(user_message_id, assistant_message_id);
        }
    }

    fn clear_running_task_for_message(&mut self, message_id: Option<i32>, cx: &mut Context<Self>) {
        let should_clear = self.task.as_ref().is_some_and(|task| {
            message_id.is_none_or(|message_id| task.contains_message(message_id))
        });
        if should_clear {
            self.task = None;
            self.chat_form
                .update(cx, |chat_form, cx| chat_form.set_running(false, cx));
        }
    }
}

// Prepares request state and coordinates async fetch execution.
impl ConversationPanelView {
    async fn fetch(
        state: WeakEntity<Self>,
        context: FetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        event!(Level::INFO, "conversation fetch");
        let request_text = context.composer_snapshot.text.clone();
        Self::clear_input(state.clone(), cx)?;
        let Some(prepared) = Self::prepare_fetch(state.clone(), &context, request_text, cx).await?
        else {
            return Ok(());
        };
        Self::bind_prepared_messages(state.clone(), &context, &prepared, cx)?;
        Self::stream_fetch(
            state,
            &context,
            prepared.runner,
            prepared.assistant_message.id,
            cx,
        )
        .await?;
        Ok(())
    }

    async fn fetch_existing_assistant_message(
        state: WeakEntity<Self>,
        chat_data: Entity<AiChatResult<ChatDataInner>>,
        config: AiChatConfig,
        conversation_id: i32,
        message_id: i32,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::find(message_id, conn)
        })??;

        if message.conversation_id != conversation_id || message.role != Role::Assistant {
            state.update_result(cx, |this, cx| {
                this.clear_running_task_for_message(Some(message_id), cx);
            })?;
            return Ok(());
        }

        let message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::reset_for_resend(message_id, conn)?;
            Message::find(message_id, conn)
        })??;
        Self::replace_chat_message_by_id(&chat_data, conversation_id, message.clone(), false, cx)?;

        Self::stream_existing_message(
            state,
            ExistingMessageFetchContext {
                chat_data,
                conversation_id,
                config,
                provider_name: message.provider.clone(),
                request_body: message.send_content,
            },
            message_id,
            cx,
        )
        .await
    }

    async fn stream_existing_message(
        state: WeakEntity<Self>,
        context: ExistingMessageFetchContext,
        assistant_message_id: i32,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let provider = provider_by_name(&context.provider_name)?;
        let settings = context
            .config
            .get_provider_settings(provider.name())
            .ok_or(AiChatError::ProviderSettingsNotFound(
                provider.name().to_string(),
            ))?
            .clone();
        let stream =
            provider.fetch_by_request_body(context.config, settings, &context.request_body);
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(message) => {
                    Self::append_assistant_message_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        message,
                        cx,
                    )?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        error.to_string(),
                        cx,
                    )?;
                    return Ok(());
                }
            }
        }
        Self::finish_assistant_message(
            state,
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            cx,
        )?;
        Ok(())
    }

    fn clear_input(state: WeakEntity<Self>, cx: &mut AsyncWindowContext) -> AiChatResult<()> {
        state.update_in_result(cx, |this, window, cx| {
            this.chat_form
                .update(cx, |chat_form, cx| chat_form.clear_input(window, cx));
        })?;
        Ok(())
    }

    async fn prepare_fetch(
        state: WeakEntity<Self>,
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedFetch>> {
        match context.composer_snapshot.extension_name.as_deref() {
            Some(extension_name) => {
                Self::prepare_extension_fetch(state, context, extension_name, request_text, cx)
                    .await
            }
            None => Self::prepare_plain_fetch(context, request_text, cx).map(Some),
        }
    }

    async fn prepare_extension_fetch(
        state: WeakEntity<Self>,
        context: &FetchContext,
        extension_name: &str,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedFetch>> {
        let user_message = Self::insert_loading_user_message(
            context.conversation_id,
            &context.composer_snapshot.provider_name,
            &request_text,
            cx,
        )?;
        let user_message_id = user_message.id;
        state.update_result(cx, |this, _cx| {
            this.bind_running_task_messages(Some(user_message_id), None);
        })?;
        context.chat_data.update_result(cx, |data, cx| {
            if let Ok(data) = data {
                data.add_message(context.conversation_id, user_message.clone());
                cx.notify();
            }
        })?;

        let Some(extension_runner) = context
            .extension_container
            .get_extension(extension_name)
            .await
            .map(Some)
            .or_else(|error| {
                Self::record_extension_error(state.clone(), user_message_id, context, error, cx)
                    .map(|()| None)
            })?
        else {
            return Ok(None);
        };
        let Some(user_content) = Runner::get_new_user_content(request_text, Some(extension_runner))
            .await
            .map(Some)
            .or_else(|error| {
                Self::record_extension_error(state.clone(), user_message_id, context, error, cx)
                    .map(|()| None)
            })?
        else {
            return Ok(None);
        };
        (|| -> AiChatResult<PreparedFetch> {
            let runner = Self::build_runner(
                context,
                Role::User,
                user_content.send_content().to_string(),
                cx,
            )?;
            let send_content = runner.request_body();
            let (user_message, assistant_message) =
                cx.read_global_result(|db: &Db, _window, _cx| {
                    let conn = &mut db.get()?;
                    Message::update_content(user_message_id, &user_content, conn)?;
                    Message::update_send_content(user_message_id, send_content, conn)?;
                    Message::update_status(user_message_id, Status::Normal, conn)?;
                    let user_message = Message::find(user_message_id, conn)?;
                    let assistant_message = Message::insert(
                        NewMessage::new(
                            context.conversation_id,
                            &context.composer_snapshot.provider_name,
                            Role::Assistant,
                            &Content::Text(String::new()),
                            send_content,
                            Status::Loading,
                        ),
                        conn,
                    )?;
                    Ok::<_, AiChatError>((user_message, assistant_message))
                })??;
            Ok(PreparedFetch {
                runner,
                user_message,
                assistant_message,
            })
        })()
        .map(Some)
        .or_else(|error| {
            Self::record_extension_error(state, user_message_id, context, error, cx).map(|()| None)
        })
    }

    fn prepare_plain_fetch(
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedFetch> {
        let user_content = Content::Text(request_text);
        let runner = Self::build_runner(
            context,
            Role::User,
            user_content.send_content().to_string(),
            cx,
        )?;
        let send_content = runner.request_body();
        let (user_message, assistant_message) =
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                let user_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        &context.composer_snapshot.provider_name,
                        Role::User,
                        &user_content,
                        send_content,
                        Status::Normal,
                    ),
                    conn,
                )?;
                let assistant_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        &context.composer_snapshot.provider_name,
                        Role::Assistant,
                        &Content::Text(String::new()),
                        send_content,
                        Status::Loading,
                    ),
                    conn,
                )?;
                Ok::<_, AiChatError>((user_message, assistant_message))
            })??;
        Ok(PreparedFetch {
            runner,
            user_message,
            assistant_message,
        })
    }

    fn build_runner(
        context: &FetchContext,
        user_message_role: Role,
        user_message_content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Runner> {
        let history_messages = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            let conversation = Conversation::find(context.conversation_id, conn)?;
            Ok::<_, AiChatError>(conversation.messages)
        })??;
        let provider_name = context.composer_snapshot.provider_name.clone();
        let template = context.composer_snapshot.request_template.clone();
        let prompts = context.composer_snapshot.prompts.clone();
        let mode = context.composer_snapshot.mode;
        let request_body = build_request_body(
            &provider_name,
            &template,
            prompts,
            mode,
            &history_messages,
            user_message_role,
            &user_message_content,
        )?;
        Ok(Runner {
            config: context.config.clone(),
            provider_name,
            request_body,
        })
    }

    fn insert_loading_user_message(
        conversation_id: i32,
        provider_name: &str,
        request_text: &str,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Message> {
        cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::insert(
                NewMessage::new(
                    conversation_id,
                    provider_name,
                    Role::User,
                    &Content::Text(request_text.to_string()),
                    &serde_json::json!({}),
                    Status::Loading,
                ),
                conn,
            )
        })?
    }

    fn bind_prepared_messages(
        state: WeakEntity<Self>,
        context: &FetchContext,
        prepared: &PreparedFetch,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        state.update_result(cx, |this, _cx| {
            this.bind_running_task_messages(
                Some(prepared.user_message.id),
                Some(prepared.assistant_message.id),
            );
        })?;
        context.chat_data.update_result(cx, |data, cx| {
            if let Ok(data) = data {
                data.replace_message(context.conversation_id, prepared.user_message.clone());
                data.add_message(context.conversation_id, prepared.assistant_message.clone());
                cx.notify();
            }
        })?;
        Ok(())
    }

    async fn stream_fetch(
        state: WeakEntity<Self>,
        context: &FetchContext,
        runner: Runner,
        assistant_message_id: i32,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let stream = runner.fetch();
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(message) => {
                    Self::append_assistant_message(context, assistant_message_id, message, cx)?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        error.to_string(),
                        cx,
                    )?;
                    return Ok(());
                }
            }
        }
        Self::finish_assistant_message(
            state,
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            cx,
        )?;
        Ok(())
    }
}

// Applies streamed message updates and persists terminal state.
impl ConversationPanelView {
    fn append_assistant_message(
        context: &FetchContext,
        assistant_message_id: i32,
        content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        Self::append_assistant_message_by_id(
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            content,
            cx,
        )
    }

    fn append_assistant_message_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let _ = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                message.content += content.as_str();
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        Ok(())
    }

    fn record_stream_error(
        state: WeakEntity<Self>,
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        error: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let message = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                message.status = Status::Error;
                message.error = Some(error.clone());
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        if let Some(message) = message {
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                Self::persist_message_snapshot(&message, conn)
            })??;
        }
        state.update_result(cx, |this, cx| {
            this.clear_running_task_for_message(Some(assistant_message_id), cx);
        })?;
        Ok(())
    }

    fn finish_assistant_message(
        state: WeakEntity<Self>,
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let message = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                message.status = Status::Normal;
                message.error = None;
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        if let Some(message) = message {
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                Self::persist_message_snapshot(&message, conn)
            })??;
        }
        state.update_result(cx, |this, cx| {
            this.clear_running_task_for_message(Some(assistant_message_id), cx);
        })?;
        Ok(())
    }

    fn update_chat_message_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        message_id: i32,
        cx: &mut AsyncWindowContext,
        update: impl FnOnce(&mut Message) + 'static,
    ) -> AiChatResult<Option<Message>> {
        let mut update = Some(update);
        chat_data.update_result(cx, move |data, cx| {
            let Ok(data) = data else {
                return None;
            };
            let update = update.take()?;
            if !data.update_message(conversation_id, message_id, update) {
                return None;
            }
            let message = data.message(conversation_id, message_id);
            if message.is_some() {
                cx.notify();
            }
            message
        })
    }

    fn persist_message_snapshot(
        message: &Message,
        conn: &mut diesel::SqliteConnection,
    ) -> AiChatResult<()> {
        Message::update_content(message.id, &message.content, conn)?;
        match message.status {
            Status::Error => {
                Message::record_error(message.id, message.error.clone().unwrap_or_default(), conn)?
            }
            status => Message::update_status(message.id, status, conn)?,
        }
        Ok(())
    }

    fn replace_chat_message_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        message: Message,
        add_when_missing: bool,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        chat_data.update_result(cx, |data, cx| {
            if let Ok(data) = data {
                data.replace_message(conversation_id, message.clone());
                if add_when_missing {
                    data.add_message(conversation_id, message);
                }
                cx.notify();
            }
        })?;
        Ok(())
    }

    fn record_extension_error(
        state: WeakEntity<Self>,
        user_message_id: i32,
        context: &FetchContext,
        error: AiChatError,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let error = match context.composer_snapshot.extension_name.as_deref() {
            Some(extension_name) => format!("extension {extension_name}: {error}"),
            None => error.to_string(),
        };
        let user_message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::record_error(user_message_id, error, conn)?;
            Message::find(user_message_id, conn)
        })??;
        Self::replace_chat_message_by_id(
            &context.chat_data,
            context.conversation_id,
            user_message,
            false,
            cx,
        )?;
        state.update_result(cx, |this, cx| {
            this.clear_running_task_for_message(Some(user_message_id), cx);
        })?;
        Ok(())
    }
}

struct FetchContext {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
    conversation_id: i32,
    composer_snapshot: ChatFormSnapshot,
    extension_container: ExtensionContainer,
    config: AiChatConfig,
}

struct PreparedFetch {
    runner: Runner,
    user_message: Message,
    assistant_message: Message,
}

struct ExistingMessageFetchContext {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
    conversation_id: i32,
    config: AiChatConfig,
    provider_name: String,
    request_body: serde_json::Value,
}

// Renders the conversation header, message list, and input composer.
impl Render for ConversationPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let message_revisions = chat_data
            .read(cx)
            .as_ref()
            .ok()
            .and_then(|data| data.conversation_messages(self.conversation_id))
            .map(|messages| {
                messages
                    .iter()
                    .map(MessageRevision::new)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.sync_message_list(message_revisions);

        let message_list = self.message_list.clone();
        let conversation_id = self.conversation_id;
        let chat_data_for_list = chat_data.clone();
        v_flex()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .pb_2()
            .child(
                h_flex()
                    .flex_initial()
                    .p_2()
                    .gap_2()
                    .child(Label::new(&self.conversation_icon))
                    .child(
                        Label::new(&self.conversation_title)
                            .text_xl()
                            .when_some(self.conversation_info.as_ref(), |this, description| {
                                this.secondary(description)
                            }),
                    ),
            )
            .child(
                div()
                    .id("conversation-panel")
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .w_full()
                    .child(
                        list(message_list.clone(), move |ix, _window, cx| {
                            chat_data_for_list
                                .read(cx)
                                .as_ref()
                                .ok()
                                .and_then(|data| data.conversation_message_at(conversation_id, ix))
                                .map(|message| MessageView::new(message).into_any_element())
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .size_full(),
                    )
                    .vertical_scrollbar(&message_list),
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

struct Runner {
    config: AiChatConfig,
    provider_name: String,
    request_body: serde_json::Value,
}

impl FetchRunner for Runner {
    fn get_provider(&self) -> &str {
        &self.provider_name
    }

    fn get_config(&self) -> &AiChatConfig {
        &self.config
    }

    fn request_body(&self) -> &serde_json::Value {
        &self.request_body
    }
}

fn build_history_messages(
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    user_message_role: Role,
    user_message_content: &str,
) -> Vec<crate::llm::Message> {
    use crate::llm::Message as FetchMessage;

    let mut request_messages = prompts
        .into_iter()
        .map(|prompt| FetchMessage::new(prompt.role, prompt.prompt))
        .collect::<Vec<_>>();

    request_messages.extend(
        history_messages
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
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    user_message_role: Role,
    user_message_content: &str,
) -> AiChatResult<serde_json::Value> {
    let history = build_history_messages(
        prompts,
        mode,
        history_messages,
        user_message_role,
        user_message_content,
    );
    provider_by_name(provider_name)?.request_body(template, history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Content, ConversationTemplatePrompt, Status};
    use time::OffsetDateTime;

    fn make_message(id: i32, role: Role, status: Status, content: Content) -> Message {
        let now = OffsetDateTime::now_utc();
        Message {
            id,
            conversation_id: 1,
            conversation_path: "/test".to_string(),
            provider: "OpenAI".to_string(),
            role,
            content,
            send_content: serde_json::json!({}),
            status,
            created_time: now,
            updated_time: now,
            start_time: now,
            end_time: now,
            error: None,
        }
    }

    #[test]
    fn running_task_contains_any_bound_message() {
        assert!(running_task_contains_message(Some(7), Some(8), 7));
        assert!(running_task_contains_message(Some(7), Some(8), 8));
        assert!(!running_task_contains_message(Some(7), Some(8), 9));
    }

    #[test]
    fn get_history_contextual_includes_all_normal_messages_and_user() {
        let contents = build_history_messages(
            vec![
                ConversationTemplatePrompt {
                    prompt: "system".to_string(),
                    role: Role::Developer,
                },
                ConversationTemplatePrompt {
                    prompt: "primer".to_string(),
                    role: Role::Assistant,
                },
            ],
            Mode::Contextual,
            &[
                make_message(
                    1,
                    Role::User,
                    Status::Normal,
                    Content::Text("u1".to_string()),
                ),
                make_message(
                    2,
                    Role::Assistant,
                    Status::Normal,
                    Content::Extension {
                        source: "src".to_string(),
                        extension_name: "ext".to_string(),
                        content: "a1".to_string(),
                    },
                ),
                make_message(
                    3,
                    Role::User,
                    Status::Error,
                    Content::Text("bad".to_string()),
                ),
            ],
            Role::User,
            "latest",
        )
        .into_iter()
        .map(|message| (message.role, message.content))
        .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                (Role::Developer, "system".to_string()),
                (Role::Assistant, "primer".to_string()),
                (Role::User, "u1".to_string()),
                (Role::Assistant, "a1".to_string()),
                (Role::User, "latest".to_string()),
            ]
        );
    }

    #[test]
    fn get_history_single_only_prompts_and_user() {
        let contents = build_history_messages(
            vec![
                ConversationTemplatePrompt {
                    prompt: "system".to_string(),
                    role: Role::Developer,
                },
                ConversationTemplatePrompt {
                    prompt: "primer".to_string(),
                    role: Role::Assistant,
                },
            ],
            Mode::Single,
            &[make_message(
                1,
                Role::Assistant,
                Status::Normal,
                Content::Text("a1".to_string()),
            )],
            Role::User,
            "latest",
        )
        .into_iter()
        .map(|message| message.content)
        .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                "system".to_string(),
                "primer".to_string(),
                "latest".to_string()
            ]
        );
    }

    #[test]
    fn get_history_assistant_only_filters_roles() {
        let contents = build_history_messages(
            vec![
                ConversationTemplatePrompt {
                    prompt: "system".to_string(),
                    role: Role::Developer,
                },
                ConversationTemplatePrompt {
                    prompt: "primer".to_string(),
                    role: Role::Assistant,
                },
            ],
            Mode::AssistantOnly,
            &[
                make_message(
                    1,
                    Role::User,
                    Status::Normal,
                    Content::Text("u1".to_string()),
                ),
                make_message(
                    2,
                    Role::Assistant,
                    Status::Normal,
                    Content::Text("a1".to_string()),
                ),
                make_message(
                    3,
                    Role::Assistant,
                    Status::Error,
                    Content::Text("bad".to_string()),
                ),
            ],
            Role::User,
            "latest",
        )
        .into_iter()
        .map(|message| (message.role, message.content))
        .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                (Role::Developer, "system".to_string()),
                (Role::Assistant, "primer".to_string()),
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
            vec![],
            Mode::Single,
            &[],
            Role::User,
            "hello",
        )?;
        assert_eq!(request_body["model"], serde_json::json!("override-model"));
        Ok(())
    }
}
