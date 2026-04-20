use crate::{
    assets::IconName,
    components::{
        add_conversation::{InitialConversationFields, open_add_conversation_dialog_with_fields},
        chat_form::ChatFormSnapshot,
        delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    },
    database::{Content, Conversation, Db, Message, Mode, NewMessage, Role, Status},
    errors::{AiChatError, AiChatResult},
    export::{ExportType, export_conversation_to_path, suggested_export_file_name},
    i18n::I18n,
    llm::{FetchRunner, FetchUpdate, provider_by_name},
    platform::gpui_ext::{AsyncWindowContextResultExt, EntityResultExt, WeakEntityResultExt},
    state::{
        AiChatConfig, ChatData, ChatDataEvent, ChatDataInner, ConversationDraft, WorkspaceStore,
    },
    views::conversation::detail::{ConversationDetailView, ConversationDetailViewExt},
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{
    AsyncWindowContext, Context, Entity, IntoElement, ListAlignment, SharedString, WeakEntity,
    Window,
};
use gpui_component::{
    Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    notification::{Notification, NotificationType},
};
use smol::stream::StreamExt;
use std::{ops::Deref, path::PathBuf};
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

    fn auto_scroll_new_messages_when_at_end(&self) -> bool {
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

pub(crate) fn conversation_export_menu(
    menu: PopupMenu,
    conversation_id: i32,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let i18n = cx.global::<I18n>();
    menu.item(
        PopupMenuItem::new(format!("{} JSON", i18n.t("button-export")))
            .icon(IconName::Share)
            .on_click(move |_, window, cx| {
                open_export_conversation_prompt(conversation_id, ExportType::Json, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(format!("{} CSV", i18n.t("button-export")))
            .icon(IconName::Share)
            .on_click(move |_, window, cx| {
                open_export_conversation_prompt(conversation_id, ExportType::Csv, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(format!("{} TXT", i18n.t("button-export")))
            .icon(IconName::Share)
            .on_click(move |_, window, cx| {
                open_export_conversation_prompt(conversation_id, ExportType::Txt, window, cx);
            }),
    )
}

pub(crate) fn open_export_conversation_prompt(
    conversation_id: i32,
    export_type: ExportType,
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

    let suggested_name = suggested_export_file_name(&conversation, export_type);
    let directory = export_default_directory();
    let path_prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));
    let (success_title, failed_title, sources_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("notify-export-conversation-success"),
            i18n.t("notify-export-conversation-failed"),
            i18n.t("field-sources"),
        )
    };

    window
        .spawn(cx, async move |cx| {
            let selected_path = match path_prompt.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(err)) => {
                    push_export_notification(
                        cx,
                        failed_title.into(),
                        format!("{}: {err}", export_type.label()),
                        NotificationType::Error,
                    );
                    return;
                }
                Err(err) => {
                    push_export_notification(
                        cx,
                        failed_title.into(),
                        format!("{}: {err}", export_type.label()),
                        NotificationType::Error,
                    );
                    return;
                }
            };

            let conversation = match cx.read_global::<ChatData, _>(|chat_data, _window, cx| {
                chat_data
                    .read(cx)
                    .as_ref()
                    .ok()
                    .and_then(|data| data.conversation(conversation_id))
                    .cloned()
            }) {
                Ok(Some(conversation)) => conversation,
                Ok(None) => {
                    push_export_notification(
                        cx,
                        failed_title.into(),
                        format!("{}: conversation not found", export_type.label()),
                        NotificationType::Error,
                    );
                    return;
                }
                Err(err) => {
                    push_export_notification(
                        cx,
                        failed_title.into(),
                        format!("{}: {err}", export_type.label()),
                        NotificationType::Error,
                    );
                    return;
                }
            };

            match export_conversation_to_path(
                &conversation,
                export_type,
                &selected_path,
                &sources_label,
            ) {
                Ok(path) => push_export_notification(
                    cx,
                    success_title.into(),
                    path.display().to_string(),
                    NotificationType::Success,
                ),
                Err(err) => push_export_notification(
                    cx,
                    failed_title.into(),
                    err.to_string(),
                    NotificationType::Error,
                ),
            }
        })
        .detach();
}

fn push_export_notification(
    cx: &mut gpui::AsyncWindowContext,
    title: SharedString,
    message: String,
    notification_type: NotificationType,
) {
    if let Err(err) = cx.window_handle().update(cx, |_, window, cx| {
        window.push_notification(
            Notification::new()
                .title(title)
                .message(message)
                .with_type(notification_type),
            cx,
        );
    }) {
        event!(Level::ERROR, "push export notification failed: {}", err);
    }
}

fn export_default_directory() -> PathBuf {
    dirs_next::document_dir()
        .or_else(dirs_next::home_dir)
        .unwrap_or_default()
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
            Self::persist_message_snapshot(message, conn)?;
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
        Self::persist_message_snapshot(&message, conn)?;
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
        let stream =
            provider.fetch_by_request_body(context.config, settings, &context.request_body);
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(FetchUpdate::ThinkingStarted) => {
                    Self::set_assistant_message_status_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        Status::Thinking,
                        cx,
                    )?;
                }
                Ok(FetchUpdate::ReasoningSummaryDelta(delta)) => {
                    Self::append_assistant_reasoning_summary_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(FetchUpdate::TextDelta(delta)) => {
                    Self::append_assistant_message_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(FetchUpdate::Complete(content)) => {
                    Self::replace_assistant_message_content_by_id(
                        &context.chat_data,
                        context.conversation_id,
                        assistant_message_id,
                        content,
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
                Ok(FetchUpdate::ThinkingStarted) => {
                    Self::set_assistant_message_status(
                        context,
                        assistant_message_id,
                        Status::Thinking,
                        cx,
                    )?;
                }
                Ok(FetchUpdate::ReasoningSummaryDelta(delta)) => {
                    Self::append_assistant_reasoning_summary(
                        context,
                        assistant_message_id,
                        delta,
                        cx,
                    )?;
                }
                Ok(FetchUpdate::TextDelta(delta)) => {
                    Self::append_assistant_message(context, assistant_message_id, delta, cx)?;
                }
                Ok(FetchUpdate::Complete(content)) => {
                    Self::replace_assistant_message_content(
                        context,
                        assistant_message_id,
                        content,
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
                make_message(1, Role::User, Status::Normal, Content::new("u1")),
                make_message(2, Role::Assistant, Status::Normal, Content::new("a1")),
                make_message(3, Role::User, Status::Error, Content::new("bad")),
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
                Content::new("a1"),
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
                make_message(1, Role::User, Status::Normal, Content::new("u1")),
                make_message(2, Role::Assistant, Status::Normal, Content::new("a1")),
                make_message(3, Role::Assistant, Status::Error, Content::new("bad")),
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
            "stream": false
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

    #[test]
    fn pause_message_snapshot_updates_loading_messages() {
        let now = OffsetDateTime::now_utc();
        let mut message = make_message(1, Role::Assistant, Status::Loading, Content::new("a1"));

        assert!(ConversationPanelView::pause_message_snapshot(
            &mut message,
            now
        ));
        assert_eq!(message.status, Status::Paused);
        assert_eq!(message.updated_time, now);
        assert_eq!(message.end_time, now);
    }

    #[test]
    fn pause_message_snapshot_ignores_non_running_messages() {
        let now = OffsetDateTime::now_utc();
        let mut message = make_message(1, Role::Assistant, Status::Normal, Content::new("a1"));
        let original = message.clone();

        assert!(!ConversationPanelView::pause_message_snapshot(
            &mut message,
            now
        ));
        assert_eq!(message, original);
    }
}
