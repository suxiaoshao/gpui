use crate::{
    components::{
        add_conversation::{InitialConversationFields, open_add_conversation_dialog_with_fields},
        chat_form::ChatFormSnapshot,
        delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    },
    database::{
        Content, Conversation, Db, Message, MessageRunPersistence, MessageRunState, Mode,
        NewMessage, Role, Status,
    },
    errors::{AiChatError, AiChatResult},
    features::conversation::detail::{
        ConversationDetailView, ConversationDetailViewExt, MessageRevisionExt,
    },
    features::home::conversation_export_menu,
    foundation::assets::IconName,
    foundation::i18n::I18n,
    llm::{
        LlmContentPart, LlmHistoryMessage, ProviderRunEvent, ProviderRunPersistenceAccumulator,
        ProviderRunRequest, ProviderRunRunner, ProviderRunState, build_input_items,
        persisted_provider_settings_snapshot, provider_by_name, provider_run_request_context_key,
    },
    platform::gpui_ext::{AsyncWindowContextResultExt, EntityResultExt, WeakEntityResultExt},
    state::{
        AiChatConfig, ChatData, ChatDataEvent, ChatDataInner, ConversationDraft, WorkspaceStore,
    },
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{
    AsyncWindowContext, Context, Entity, IntoElement, ListAlignment, SharedString, WeakEntity,
    Window,
};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};
use smol::stream::StreamExt;
use std::ops::Deref;
use time::OffsetDateTime;
use tracing::{Instrument, Level, event, span};

pub(crate) type ConversationPanelView = ConversationDetailView<ConversationPanelState>;

pub(crate) struct ConversationPanelState {
    conversation_id: i32,
    conversation_icon: SharedString,
    conversation_title: SharedString,
    conversation_info: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageRevision {
    id: i32,
    status: Status,
    updated_time_nanos: i128,
    content_len: usize,
    error_len: usize,
}

impl MessageRevision {
    fn new(message: &Message) -> Self {
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

impl MessageRevisionExt for MessageRevision {
    type Id = i32;

    fn message_id(&self) -> Self::Id {
        self.id
    }
}

impl ConversationPanelView {
    pub fn new(conversation: &Conversation, window: &mut Window, cx: &mut Context<Self>) -> Self {
        ConversationDetailView::new_with_detail(
            ConversationPanelState {
                conversation_id: conversation.id,
                conversation_icon: conversation.icon.clone().into(),
                conversation_title: conversation.title.clone().into(),
                conversation_info: conversation.info.clone().map(Into::into),
            },
            window,
            cx,
        )
    }

    pub(crate) fn restore_draft(
        &mut self,
        draft: ConversationDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.restore_draft(draft, window, cx)
        });
    }

    pub(crate) fn sync_metadata(&mut self, conversation: &Conversation, cx: &mut Context<Self>) {
        self.detail.conversation_icon = conversation.icon.clone().into();
        self.detail.conversation_title = conversation.title.clone().into();
        self.detail.conversation_info = conversation.info.clone().map(Into::into);
        cx.notify();
    }
}

impl ConversationDetailViewExt for ConversationPanelState {
    type Message = Message;
    type MessageId = i32;
    type Revision = MessageRevision;

    fn title(&self, _cx: &gpui::App) -> SharedString {
        self.conversation_title.clone()
    }

    fn subtitle(&self, _cx: &gpui::App) -> Option<SharedString> {
        self.conversation_info
            .clone()
            .filter(|info| !info.as_ref().trim().is_empty())
    }

    fn header_leading(&self, _cx: &gpui::App) -> Option<gpui::AnyElement> {
        Some(gpui_component::label::Label::new(&self.conversation_icon).into_any_element())
    }

    fn header_actions(
        view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) -> Vec<gpui::AnyElement> {
        let conversation_id = view.detail.conversation_id;
        let element_prefix = view.detail.element_prefix();
        let (copy_tooltip, export_tooltip) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("tooltip-copy-conversation"),
                i18n.t("tooltip-export-conversation"),
            )
        };

        vec![
            Button::new(SharedString::from(format!("{element_prefix}-copy")))
                .icon(IconName::Copy)
                .ghost()
                .small()
                .disabled(view.has_running_task())
                .tooltip(copy_tooltip)
                .on_click(move |_, window, cx| {
                    open_copy_conversation_dialog(conversation_id, window, cx);
                })
                .into_any_element(),
            Button::new(SharedString::from(format!("{element_prefix}-export")))
                .icon(IconName::Share)
                .ghost()
                .small()
                .disabled(view.has_running_task())
                .tooltip(export_tooltip)
                .dropdown_menu(move |menu, window, cx| {
                    conversation_export_menu(menu, conversation_id, window, cx)
                })
                .into_any_element(),
        ]
    }

    fn element_prefix(&self) -> SharedString {
        SharedString::from(format!("conversation-panel-{}", self.conversation_id))
    }

    fn message_list_alignment(&self) -> ListAlignment {
        ListAlignment::Top
    }

    fn measure_all_message_list(&self) -> bool {
        true
    }

    fn initially_reveal_latest_message(&self) -> bool {
        true
    }

    fn message_revisions(&self, cx: &gpui::App) -> Vec<Self::Revision> {
        cx.global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .and_then(|data| data.conversation_messages(self.conversation_id))
            .map(|messages| messages.iter().map(MessageRevision::new).collect())
            .unwrap_or_default()
    }

    fn message_at(&self, index: usize, cx: &gpui::App) -> Option<Self::Message> {
        cx.global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .and_then(|data| data.conversation_message_at(self.conversation_id, index))
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
            let conversation_id = view.detail.conversation_id;
            let chat_data = cx.global::<ChatData>().deref().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let state = this.clone();
                let context = FetchContext {
                    chat_data,
                    conversation_id,
                    composer_snapshot: snapshot,
                    config,
                };
                if let Err(err) = ConversationPanelView::fetch(state, context, cx)
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
            view.set_running_task(task, cx);
        }
    }

    fn on_pause_requested(
        view: &mut ConversationDetailView<Self>,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        if let Err(err) = view.pause_task(cx) {
            event!(Level::ERROR, "pause fetch failed: {}", err);
            cx.notify();
        }
    }

    fn on_chat_form_state_changed(
        view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        let draft = view.chat_form.read(cx).draft_snapshot(cx);
        cx.global::<WorkspaceStore>()
            .deref()
            .clone()
            .update(cx, |workspace, cx| {
                workspace.sync_conversation_chat_form_state(view.detail.conversation_id, draft, cx);
            });
    }

    fn supports_clear(&self) -> bool {
        true
    }

    fn clear(
        view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    ) {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let conversation_id = view.detail.conversation_id;
        let (title, message) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-clear-conversation-title"),
                i18n.t("dialog-clear-conversation-message"),
            )
        };
        open_destructive_confirm_dialog(
            title,
            message,
            DestructiveAction::Clear,
            move |_window, cx| {
                chat_data.update(cx, move |_this, cx| {
                    cx.emit(ChatDataEvent::ClearConversationMessages(conversation_id));
                });
            },
            window,
            cx,
        );
    }
}

pub(crate) fn open_copy_conversation_dialog(
    conversation_id: i32,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let conversation = cx
        .global::<ChatData>()
        .read(cx)
        .as_ref()
        .ok()
        .and_then(|data| data.conversation(conversation_id))
        .cloned();
    let Some(conversation) = conversation else {
        return;
    };

    open_add_conversation_dialog_with_fields(
        conversation.folder_id,
        InitialConversationFields {
            name: Some(conversation.title),
            icon: Some(conversation.icon),
            info: conversation.info,
        },
        None,
        window,
        cx,
    );
}

impl ConversationPanelView {
    fn pause_message_snapshot(message: &mut Message, now: OffsetDateTime) -> bool {
        if !matches!(message.status, Status::Loading | Status::Thinking) {
            return false;
        }
        message.status = Status::Paused;
        message.updated_time = now;
        message.end_time = now;
        true
    }

    fn pause_task(&mut self, cx: &mut Context<Self>) -> AiChatResult<()> {
        let Some(task) = self.task.take() else {
            return Ok(());
        };

        let conversation_id = self.detail.conversation_id;
        let chat_data = cx.global::<ChatData>().deref().clone();
        let paused_messages = chat_data.update(cx, move |data, cx| {
            let mut paused_messages = Vec::new();
            if let Ok(data) = data {
                for message_id in task.message_ids().into_iter().flatten() {
                    let Some(mut message) = data.message(conversation_id, message_id) else {
                        continue;
                    };
                    let now = OffsetDateTime::now_utc();
                    if !Self::pause_message_snapshot(&mut message, now) {
                        continue;
                    }
                    data.replace_message(conversation_id, message.clone());
                    paused_messages.push(message);
                }
                cx.notify();
            }
            paused_messages
        });
        let conn = &mut cx.global::<Db>().get()?;
        for message in &paused_messages {
            Self::persist_message_snapshot(message, None, conn)?;
        }
        self.set_chat_form_running(false, cx);
        cx.notify();
        Ok(())
    }

    fn pause_stale_message(&mut self, message_id: i32, cx: &mut Context<Self>) -> AiChatResult<()> {
        let conversation_id = self.detail.conversation_id;
        let chat_data = cx.global::<ChatData>().deref().clone();
        let paused_message = chat_data.update(cx, move |data, cx| {
            let Ok(data) = data else {
                return None;
            };
            let mut message = data.message(conversation_id, message_id)?;
            let now = OffsetDateTime::now_utc();
            if !Self::pause_message_snapshot(&mut message, now) {
                return None;
            }
            data.replace_message(conversation_id, message.clone());
            cx.notify();
            Some(message)
        });
        let Some(message) = paused_message else {
            return Ok(());
        };

        let conn = &mut cx.global::<Db>().get()?;
        Self::persist_message_snapshot(&message, None, conn)?;
        self.set_chat_form_running(false, cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn pause_message(&mut self, message_id: i32, cx: &mut Context<Self>) {
        if self
            .task
            .as_ref()
            .is_some_and(|task| task.contains_message(message_id))
        {
            if let Err(err) = self.pause_task(cx) {
                event!(Level::ERROR, "pause message failed: {}", err);
            }
            return;
        }

        if let Err(err) = self.pause_stale_message(message_id, cx) {
            event!(Level::ERROR, "pause stale message failed: {}", err);
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
        let conversation_id = self.detail.conversation_id;
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
        self.set_running_task(task, cx);
        self.bind_running_task_messages(None, Some(message_id));
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
        let prepared = Self::prepare_fetch(&context, request_text, cx).await?;
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
        let request =
            ProviderRunRequest::from_request_body(provider.name(), context.request_body.clone());
        let mut run_persistence = ProviderRunPersistenceAccumulator::new(&request, &context.config);
        let stream = provider.run(context.config, settings, &request);
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(ProviderRunEvent::ThinkingStarted) => {
                    Self::set_assistant_message_status_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        Status::Thinking,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::ReasoningSummaryDelta(delta)) => {
                    Self::append_assistant_reasoning_summary_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::TextDelta(delta)) => {
                    Self::append_assistant_message_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::Completed {
                    content,
                    state,
                    usage,
                }) => {
                    run_persistence.record_completed(state, usage);
                    Self::replace_assistant_message_content_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        content,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::OutputItemAdded(item)) => {
                    run_persistence.record_output_item_added(item);
                }
                Ok(ProviderRunEvent::OutputItemDone(item)) => {
                    run_persistence.record_output_item_done(item);
                }
                Ok(ProviderRunEvent::ToolCallRequested(tool_call)) => {
                    run_persistence.record_tool_call_requested(tool_call);
                }
                Ok(ProviderRunEvent::ToolResultReceived(tool_result)) => {
                    run_persistence.record_tool_result_received(tool_result);
                }
                Ok(ProviderRunEvent::McpApprovalRequested(request)) => {
                    run_persistence.record_mcp_approval_requested(request);
                }
                Ok(ProviderRunEvent::UsageUpdated(usage)) => {
                    run_persistence.record_usage(usage);
                }
                Ok(ProviderRunEvent::Failed { message }) => {
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        message,
                        run_persistence.persistence(),
                        cx,
                    )?;
                    return Ok(());
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        error.to_string(),
                        run_persistence.persistence(),
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
            run_persistence.persistence(),
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
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedFetch> {
        Self::prepare_plain_fetch(context, request_text, cx)
    }

    fn prepare_plain_fetch(
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedFetch> {
        let user_content = Content::new(request_text);
        let runner = Self::build_runner(
            context,
            Role::User,
            context.composer_snapshot.content_parts.clone(),
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
                        &Content::default(),
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
        user_message_content: Vec<LlmContentPart>,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Runner> {
        let provider_name = context.composer_snapshot.provider_name.clone();
        let template = context.composer_snapshot.request_template.clone();
        let mode = context.composer_snapshot.mode;
        let prompts = context.composer_snapshot.prompts.clone();
        let (history_messages, continuation) =
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                let conversation = Conversation::find(context.conversation_id, conn)?;
                let request_context_key = openai_request_context_key(
                    &provider_name,
                    &template,
                    prompts.clone(),
                    mode,
                    &conversation.messages,
                    (user_message_role, user_message_content.clone()),
                )?;
                let continuation = openai_continuation_candidate(
                    &provider_name,
                    &template,
                    mode,
                    &conversation.messages,
                    &context.config,
                    request_context_key.as_ref(),
                    conn,
                )?;
                Ok::<_, AiChatError>((conversation.messages, continuation))
            })??;
        let run_request = build_run_request_with_continuation(
            &provider_name,
            &template,
            prompts,
            mode,
            &history_messages,
            (user_message_role, user_message_content),
            continuation,
        )?;
        Ok(Runner {
            config: context.config.clone(),
            provider_name,
            run_request,
        })
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
        let mut run_persistence =
            ProviderRunPersistenceAccumulator::new(runner.run_request(), runner.get_config());
        let stream = runner.run();
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(ProviderRunEvent::ThinkingStarted) => {
                    Self::set_assistant_message_status(
                        context,
                        assistant_message_id,
                        Status::Thinking,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::ReasoningSummaryDelta(delta)) => {
                    Self::append_assistant_reasoning_summary(
                        context,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::TextDelta(delta)) => {
                    Self::append_assistant_message(context, assistant_message_id, delta, cx)?;
                }
                Ok(ProviderRunEvent::Completed {
                    content,
                    state,
                    usage,
                }) => {
                    run_persistence.record_completed(state, usage);
                    Self::replace_assistant_message_content(
                        context,
                        assistant_message_id,
                        content,
                        cx,
                    )?;
                }
                Ok(ProviderRunEvent::OutputItemAdded(item)) => {
                    run_persistence.record_output_item_added(item);
                }
                Ok(ProviderRunEvent::OutputItemDone(item)) => {
                    run_persistence.record_output_item_done(item);
                }
                Ok(ProviderRunEvent::ToolCallRequested(tool_call)) => {
                    run_persistence.record_tool_call_requested(tool_call);
                }
                Ok(ProviderRunEvent::ToolResultReceived(tool_result)) => {
                    run_persistence.record_tool_result_received(tool_result);
                }
                Ok(ProviderRunEvent::McpApprovalRequested(request)) => {
                    run_persistence.record_mcp_approval_requested(request);
                }
                Ok(ProviderRunEvent::UsageUpdated(usage)) => {
                    run_persistence.record_usage(usage);
                }
                Ok(ProviderRunEvent::Failed { message }) => {
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        message,
                        run_persistence.persistence(),
                        cx,
                    )?;
                    return Ok(());
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    Self::record_stream_error(
                        state,
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        error.to_string(),
                        run_persistence.persistence(),
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
            run_persistence.persistence(),
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

    fn append_assistant_reasoning_summary(
        context: &FetchContext,
        assistant_message_id: i32,
        delta: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        Self::append_assistant_reasoning_summary_by_id(
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            delta,
            cx,
        )
    }

    fn replace_assistant_message_content(
        context: &FetchContext,
        assistant_message_id: i32,
        content: Content,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        Self::replace_assistant_message_content_by_id(
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            content,
            cx,
        )
    }

    fn set_assistant_message_status(
        context: &FetchContext,
        assistant_message_id: i32,
        status: Status,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        Self::set_assistant_message_status_by_id(
            &context.chat_data,
            context.conversation_id,
            assistant_message_id,
            status,
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
                if matches!(message.status, Status::Thinking) {
                    message.status = Status::Loading;
                }
                message.content += content.as_str();
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        Ok(())
    }

    fn append_assistant_reasoning_summary_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        delta: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let _ = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                if !matches!(message.status, Status::Loading | Status::Normal) {
                    message.status = Status::Thinking;
                }
                message.content.append_reasoning_summary(&delta);
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        Ok(())
    }

    fn set_assistant_message_status_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        status: Status,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let _ = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                if message.status == status {
                    return;
                }
                message.status = status;
                message.updated_time = now;
                message.end_time = now;
            },
        )?;
        Ok(())
    }

    fn replace_assistant_message_content_by_id(
        chat_data: &Entity<AiChatResult<ChatDataInner>>,
        conversation_id: i32,
        assistant_message_id: i32,
        content: Content,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let _ = Self::update_chat_message_by_id(
            chat_data,
            conversation_id,
            assistant_message_id,
            cx,
            move |message| {
                message.content = content;
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
        run_persistence: Option<MessageRunPersistence>,
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
                Self::persist_message_snapshot(&message, run_persistence.as_ref(), conn)
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
        run_persistence: Option<MessageRunPersistence>,
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
                Self::persist_message_snapshot(&message, run_persistence.as_ref(), conn)
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
        run_persistence: Option<&MessageRunPersistence>,
        conn: &mut diesel::SqliteConnection,
    ) -> AiChatResult<()> {
        conn.immediate_transaction(|conn| {
            Message::update_content(message.id, &message.content, conn)?;
            match message.status {
                Status::Error => Message::record_error(
                    message.id,
                    message.error.clone().unwrap_or_default(),
                    conn,
                )?,
                status => Message::update_status(message.id, status, conn)?,
            }
            if let Some(run_persistence) = run_persistence {
                Message::replace_run_persistence(message.id, run_persistence, conn)?;
            }
            Ok(())
        })
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
}

struct FetchContext {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
    conversation_id: i32,
    composer_snapshot: ChatFormSnapshot,
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

struct Runner {
    config: AiChatConfig,
    provider_name: String,
    run_request: ProviderRunRequest,
}

impl ProviderRunRunner for Runner {
    fn get_provider(&self) -> &str {
        &self.provider_name
    }

    fn get_config(&self) -> &AiChatConfig {
        &self.config
    }

    fn run_request(&self) -> &ProviderRunRequest {
        &self.run_request
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ContinuationCandidate {
    after_index: usize,
    state: ProviderRunState,
}

fn build_history_messages(
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    user_message_role: Role,
    user_message_content: Vec<LlmContentPart>,
) -> Vec<crate::llm::LlmInputItem> {
    let history = history_messages
        .iter()
        .map(|message| LlmHistoryMessage::new(message.role, message.status, &message.content));
    build_input_items(
        &prompts,
        mode,
        history,
        user_message_role,
        user_message_content,
    )
}

fn template_model(template: &serde_json::Value) -> Option<&str> {
    template.get("model").and_then(serde_json::Value::as_str)
}

fn compatible_openai_run_state(
    provider_name: &str,
    model: &str,
    current_settings: &serde_json::Value,
    current_request_context_key: &serde_json::Value,
    run_state: &MessageRunState,
) -> bool {
    if provider_name != "OpenAI"
        || run_state.provider != provider_name
        || run_state.run_id.as_ref().is_none_or(|id| id.is_empty())
        || run_state.model.as_deref() != Some(model)
        || run_state.settings.as_ref() != Some(current_settings)
    {
        return false;
    }

    provider_run_request_context_key(&run_state.request_body) == *current_request_context_key
}

fn openai_request_context_key(
    provider_name: &str,
    template: &serde_json::Value,
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    current_user_message: (Role, Vec<LlmContentPart>),
) -> AiChatResult<Option<serde_json::Value>> {
    if provider_name != "OpenAI" || mode != Mode::Contextual {
        return Ok(None);
    }
    let history = build_history_messages(
        prompts,
        mode,
        history_messages,
        current_user_message.0,
        current_user_message.1,
    );
    let request = provider_by_name(provider_name)?.build_run_request(template, history)?;
    Ok(Some(provider_run_request_context_key(
        &request.request_body,
    )))
}

fn openai_continuation_candidate(
    provider_name: &str,
    template: &serde_json::Value,
    mode: Mode,
    history_messages: &[Message],
    config: &AiChatConfig,
    current_request_context_key: Option<&serde_json::Value>,
    conn: &mut diesel::SqliteConnection,
) -> AiChatResult<Option<ContinuationCandidate>> {
    if provider_name != "OpenAI" || mode != Mode::Contextual {
        return Ok(None);
    }
    let Some(model) = template_model(template) else {
        return Ok(None);
    };
    let Some(current_settings) = persisted_provider_settings_snapshot(provider_name, config) else {
        return Ok(None);
    };
    let Some(current_request_context_key) = current_request_context_key else {
        return Ok(None);
    };

    for (index, message) in history_messages.iter().enumerate().rev() {
        if message.role != Role::Assistant
            || message.status != Status::Normal
            || message.provider != provider_name
        {
            continue;
        }
        let Some(run_state) = Message::run_state(message.id, conn)? else {
            continue;
        };
        if compatible_openai_run_state(
            provider_name,
            model,
            &current_settings,
            current_request_context_key,
            &run_state,
        ) {
            return Ok(Some(ContinuationCandidate {
                after_index: index,
                state: run_state.to_provider_state(),
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
fn build_run_request(
    provider_name: &str,
    template: &serde_json::Value,
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    user_message_role: Role,
    user_message_content: &str,
) -> AiChatResult<ProviderRunRequest> {
    build_run_request_with_continuation(
        provider_name,
        template,
        prompts,
        mode,
        history_messages,
        (
            user_message_role,
            vec![LlmContentPart::text(user_message_content)],
        ),
        None,
    )
}

fn build_run_request_with_continuation(
    provider_name: &str,
    template: &serde_json::Value,
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    current_user_message: (Role, Vec<LlmContentPart>),
    continuation: Option<ContinuationCandidate>,
) -> AiChatResult<ProviderRunRequest> {
    let state = continuation
        .as_ref()
        .map(|continuation| continuation.state.clone());
    let history_messages = continuation
        .as_ref()
        .map(|continuation| &history_messages[(continuation.after_index + 1)..])
        .unwrap_or(history_messages);
    let history = build_history_messages(
        prompts,
        mode,
        history_messages,
        current_user_message.0,
        current_user_message.1,
    );
    provider_by_name(provider_name)?.build_run_request_with_state(template, history, state)
}

#[cfg(test)]
fn build_request_body(
    provider_name: &str,
    template: &serde_json::Value,
    prompts: Vec<crate::database::ConversationTemplatePrompt>,
    mode: Mode,
    history_messages: &[Message],
    user_message_role: Role,
    user_message_content: &str,
) -> AiChatResult<serde_json::Value> {
    Ok(build_run_request(
        provider_name,
        template,
        prompts,
        mode,
        history_messages,
        user_message_role,
        user_message_content,
    )?
    .request_body)
}

#[cfg(test)]
mod tests;
