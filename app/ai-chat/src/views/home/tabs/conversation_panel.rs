use crate::{
    components::chat_input::{ChatInput, Pause, Send, input_state},
    config::AiChatConfig,
    database::{
        Content, Conversation, ConversationTemplate, Db, Message, Mode, NewMessage, Role, Status,
    },
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    gpui_ext::{AsyncWindowContextResultExt, EntityResultExt, WeakEntityResultExt},
    llm::FetchRunner,
    store::{ChatData, ChatDataInner},
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{
    AppContext, AsyncWindowContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, WeakEntity, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::{
    h_flex,
    input::InputState,
    label::Label,
    scroll::ScrollableElement,
    select::{SearchableVec, SelectState},
    v_flex,
};
use smol::stream::StreamExt;
use std::ops::Deref;
use tracing::{Instrument, Level, event, span};

pub(crate) struct ConversationPanelView {
    conversation: Conversation,
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<SearchableVec<String>>>,
    _subscriptions: Vec<Subscription>,
    task: Option<RunningTask>,
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

impl ConversationPanelView {
    pub fn new(conversation: &Conversation, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = input_state(window, cx);
        let _subscriptions = vec![];
        let extension_container = cx.global::<ExtensionContainer>();
        let all_extensions = extension_container.get_all_config();
        Self {
            conversation: conversation.clone(),
            input_state,
            _subscriptions,
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
            task: None,
        }
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
        if self.task.is_none() && !text.is_empty() {
            let config = cx.global::<AiChatConfig>().clone();
            let extension_container = cx.global::<ExtensionContainer>().clone();
            let conversation_id = self.conversation.id;
            let chat_data = cx.global::<ChatData>().deref().clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let state = this.clone();
                let context = FetchContext {
                    chat_data,
                    conversation_id,
                    text: text.clone(),
                    extension_name,
                    extension_container,
                    config,
                };
                if let Err(err) = Self::fetch(state, context, cx)
                    .compat()
                    .instrument(span)
                    .await
                {
                    event!(Level::ERROR, "fetch failed: {}", err);
                    let _ = this.update_result(cx, |this, _cx| {
                        this.clear_running_task_for_message(None);
                    });
                }
            });
            self.task = Some(RunningTask::new(task));
        }
    }

    fn on_pause_action(&mut self, _: &Pause, _window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = self.pause_task(cx) {
            event!(Level::ERROR, "pause fetch failed: {}", err);
            cx.notify();
        }
    }

    fn pause_task(&mut self, cx: &mut Context<Self>) -> AiChatResult<()> {
        let Some(task) = self.task.take() else {
            return Ok(());
        };

        let conversation_id = self.conversation.id;
        let paused_messages = {
            let conn = &mut cx.global::<Db>().get()?;
            let mut paused_messages = Vec::new();
            for message_id in task.message_ids().into_iter().flatten() {
                let message = Message::find(message_id, conn)?;
                if message.conversation_id != conversation_id
                    || !matches!(message.status, Status::Loading)
                {
                    continue;
                }
                Message::update_status(message_id, Status::Paused, conn)?;
                paused_messages.push(Message::find(message_id, conn)?);
            }
            paused_messages
        };

        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, move |data, cx| {
            if let Ok(data) = data {
                for message in paused_messages {
                    data.replace_message(conversation_id, message);
                }
                cx.notify();
            }
        });
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

    fn bind_running_task_messages(
        &mut self,
        user_message_id: Option<i32>,
        assistant_message_id: Option<i32>,
    ) {
        if let Some(task) = self.task.as_mut() {
            task.bind_messages(user_message_id, assistant_message_id);
        }
    }

    fn clear_running_task_for_message(&mut self, message_id: Option<i32>) {
        let should_clear = self.task.as_ref().is_some_and(|task| {
            message_id.is_none_or(|message_id| task.contains_message(message_id))
        });
        if should_clear {
            self.task = None;
        }
    }

    async fn fetch(
        state: WeakEntity<Self>,
        context: FetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        event!(Level::INFO, "conversation fetch");
        let request_text = context.text.to_string();
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

    fn clear_input(state: WeakEntity<Self>, cx: &mut AsyncWindowContext) -> AiChatResult<()> {
        state.update_in_result(cx, |this, window, cx| {
            this.input_state.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        })?;
        Ok(())
    }

    async fn prepare_fetch(
        state: WeakEntity<Self>,
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Option<PreparedFetch>> {
        match context.extension_name.as_deref() {
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
        let user_message =
            Self::insert_loading_user_message(context.conversation_id, &request_text, cx)?;
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

        let extension_runner = match context
            .extension_container
            .get_extension(extension_name)
            .await
        {
            Ok(extension_runner) => extension_runner,
            Err(error) => {
                Self::record_extension_error(state, user_message_id, context, error, cx).await?;
                return Ok(None);
            }
        };
        let user_content = match Runner::get_new_user_content(request_text, Some(extension_runner))
            .await
        {
            Ok(content) => content,
            Err(error) => {
                Self::record_extension_error(state, user_message_id, context, error, cx).await?;
                return Ok(None);
            }
        };
        let runner = Self::build_runner(
            context.conversation_id,
            context.config.clone(),
            Role::User,
            user_content.send_content().to_string(),
            cx,
        )?;
        let send_content = runner.request_body()?;
        let (user_message, assistant_message) =
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                Message::update_content(user_message_id, &user_content, conn)?;
                Message::update_send_content(user_message_id, send_content.clone(), conn)?;
                Message::update_status(user_message_id, Status::Normal, conn)?;
                let user_message = Message::find(user_message_id, conn)?;
                let assistant_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        Role::Assistant,
                        Content::Text(String::new()),
                        send_content.clone(),
                        Status::Loading,
                    ),
                    conn,
                )?;
                Ok::<_, AiChatError>((user_message, assistant_message))
            })??;
        Ok(Some(PreparedFetch {
            runner,
            user_message,
            assistant_message,
        }))
    }

    fn prepare_plain_fetch(
        context: &FetchContext,
        request_text: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<PreparedFetch> {
        let user_content = Content::Text(request_text);
        let runner = Self::build_runner(
            context.conversation_id,
            context.config.clone(),
            Role::User,
            user_content.send_content().to_string(),
            cx,
        )?;
        let send_content = runner.request_body()?;
        let (user_message, assistant_message) =
            cx.read_global_result(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                let user_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        Role::User,
                        user_content.clone(),
                        send_content.clone(),
                        Status::Normal,
                    ),
                    conn,
                )?;
                let assistant_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        Role::Assistant,
                        Content::Text(String::new()),
                        send_content.clone(),
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
        conversation_id: i32,
        config: AiChatConfig,
        user_message_role: Role,
        user_message_content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Runner> {
        let (conversation, template) = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            let conversation = Conversation::find(conversation_id, conn)?;
            let template = ConversationTemplate::find(conversation.template_id, conn)?;
            Ok::<_, AiChatError>((conversation, template))
        })??;
        Ok(Runner {
            config,
            template,
            history_messages: conversation.messages,
            user_message_role,
            user_message_content,
        })
    }

    fn insert_loading_user_message(
        conversation_id: i32,
        request_text: &str,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<Message> {
        Ok(cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::insert(
                NewMessage::new(
                    conversation_id,
                    Role::User,
                    Content::Text(request_text.to_string()),
                    serde_json::json!({}),
                    Status::Loading,
                ),
                conn,
            )
        })??)
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
                        context,
                        assistant_message_id,
                        error.to_string(),
                        cx,
                    )?;
                    return Ok(());
                }
            }
        }
        Self::finish_assistant_message(state, context, assistant_message_id, cx)?;
        Ok(())
    }

    fn append_assistant_message(
        context: &FetchContext,
        assistant_message_id: i32,
        content: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::add_content(assistant_message_id, content, conn)?;
            Message::find(assistant_message_id, conn)
        })??;
        Self::replace_chat_message(context, message, false, cx)
    }

    fn record_stream_error(
        state: WeakEntity<Self>,
        context: &FetchContext,
        assistant_message_id: i32,
        error: String,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::record_error(assistant_message_id, error, conn)?;
            Message::find(assistant_message_id, conn)
        })??;
        Self::replace_chat_message(context, message, false, cx)?;
        state.update_result(cx, |this, _cx| {
            this.clear_running_task_for_message(Some(assistant_message_id));
        })?;
        Ok(())
    }

    fn finish_assistant_message(
        state: WeakEntity<Self>,
        context: &FetchContext,
        assistant_message_id: i32,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::update_status(assistant_message_id, Status::Normal, conn)?;
            Message::find(assistant_message_id, conn)
        })??;
        Self::replace_chat_message(context, message, false, cx)?;
        state.update_result(cx, |this, _cx| {
            this.clear_running_task_for_message(Some(assistant_message_id));
        })?;
        Ok(())
    }

    fn replace_chat_message(
        context: &FetchContext,
        message: Message,
        add_when_missing: bool,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        context.chat_data.update_result(cx, |data, cx| {
            if let Ok(data) = data {
                data.replace_message(context.conversation_id, message.clone());
                if add_when_missing {
                    data.add_message(context.conversation_id, message);
                }
                cx.notify();
            }
        })?;
        Ok(())
    }

    async fn record_extension_error(
        state: WeakEntity<Self>,
        user_message_id: i32,
        context: &FetchContext,
        error: AiChatError,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        let error = match context.extension_name.as_deref() {
            Some(extension_name) => format!("extension {extension_name}: {error}"),
            None => error.to_string(),
        };
        let user_message = cx.read_global_result(|db: &Db, _window, _cx| {
            let conn = &mut db.get()?;
            Message::record_error(user_message_id, error, conn)?;
            Message::find(user_message_id, conn)
        })??;
        context.chat_data.update_result(cx, |data, cx| {
            if let Ok(data) = data {
                data.replace_message(context.conversation_id, user_message);
                cx.notify();
            }
        })?;
        state.update_result(cx, |this, _cx| {
            this.clear_running_task_for_message(Some(user_message_id));
        })?;
        Ok(())
    }
}

struct FetchContext {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
    conversation_id: i32,
    text: SharedString,
    extension_name: Option<String>,
    extension_container: ExtensionContainer,
    config: AiChatConfig,
}

struct PreparedFetch {
    runner: Runner,
    user_message: Message,
    assistant_message: Message,
}

impl Render for ConversationPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let chat_data = chat_data.read(cx).as_ref().ok();
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
                    .child(Label::new(&self.conversation.icon))
                    .child(
                        Label::new(&self.conversation.title)
                            .text_xl()
                            .when_some(self.conversation.info.as_ref(), |this, description| {
                                this.secondary(description)
                            }),
                    ),
            )
            .child(
                div()
                    .id("conversation-panel")
                    .flex_1()
                    .overflow_hidden()
                    .when_some(chat_data.map(|x| x.panel_messages()), |this, messages| {
                        this.children(messages)
                    })
                    .child(div().h_2())
                    .overflow_y_scrollbar(),
            )
            .child(
                div()
                    .w_full()
                    .flex_initial()
                    .child(
                        ChatInput::new(&self.input_state, &self.extension_state)
                            .running(self.task.is_some())
                            .on_action(cx.listener(Self::on_send_action))
                            .on_action(cx.listener(Self::on_pause_action)),
                    )
                    .px_2(),
            )
    }
}

struct Runner {
    config: AiChatConfig,
    template: ConversationTemplate,
    history_messages: Vec<Message>,
    user_message_role: Role,
    user_message_content: String,
}

impl FetchRunner for Runner {
    fn get_adapter(&self) -> &str {
        &self.template.adapter
    }

    fn get_template(&self) -> &serde_json::Value {
        &self.template.template
    }

    fn get_config(&self) -> &AiChatConfig {
        &self.config
    }

    fn get_history(&self) -> Vec<crate::llm::Message> {
        use crate::llm::Message as FetchMessage;
        let mut prompts_messages = self
            .template
            .prompts
            .iter()
            .map(|prompt| FetchMessage::new(prompt.role, prompt.prompt.clone()))
            .collect::<Vec<_>>();

        match self.template.mode {
            Mode::Contextual => {
                let history_messages = self
                    .history_messages
                    .iter()
                    .filter(|message| message.status == Status::Normal)
                    .map(|message| {
                        FetchMessage::new(message.role, message.content.send_content().to_string())
                    });
                prompts_messages.extend(history_messages);
            }
            Mode::Single => {}
            Mode::AssistantOnly => {
                let history_messages = self
                    .history_messages
                    .iter()
                    .filter(|message| message.status == Status::Normal)
                    .filter(|message| message.role == Role::Assistant)
                    .map(|message| {
                        FetchMessage::new(message.role, message.content.send_content().to_string())
                    });
                prompts_messages.extend(history_messages);
            }
        }

        prompts_messages.push(FetchMessage::new(
            self.user_message_role,
            self.user_message_content.clone(),
        ));
        prompts_messages
    }
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

    fn make_template(mode: Mode) -> ConversationTemplate {
        let now = OffsetDateTime::now_utc();
        ConversationTemplate {
            id: 1,
            name: "t".to_string(),
            icon: "i".to_string(),
            description: None,
            mode,
            adapter: "openai".to_string(),
            template: serde_json::json!({"model": "gpt-test"}),
            prompts: vec![
                ConversationTemplatePrompt {
                    prompt: "system".to_string(),
                    role: Role::Developer,
                },
                ConversationTemplatePrompt {
                    prompt: "primer".to_string(),
                    role: Role::Assistant,
                },
            ],
            created_time: now,
            updated_time: now,
        }
    }

    #[test]
    fn get_history_contextual_includes_all_normal_messages_and_user() {
        let template = make_template(Mode::Contextual);
        let history_messages = vec![
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
        ];
        let runner = Runner {
            config: AiChatConfig::default(),
            template,
            history_messages,
            user_message_role: Role::User,
            user_message_content: "latest".to_string(),
        };
        let history = runner.get_history();
        let contents = history
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
        let template = make_template(Mode::Single);
        let history_messages = vec![make_message(
            1,
            Role::Assistant,
            Status::Normal,
            Content::Text("a1".to_string()),
        )];
        let runner = Runner {
            config: AiChatConfig::default(),
            template,
            history_messages,
            user_message_role: Role::User,
            user_message_content: "latest".to_string(),
        };
        let history = runner.get_history();
        let contents = history
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
        let template = make_template(Mode::AssistantOnly);
        let history_messages = vec![
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
        ];
        let runner = Runner {
            config: AiChatConfig::default(),
            template,
            history_messages,
            user_message_role: Role::User,
            user_message_content: "latest".to_string(),
        };
        let history = runner.get_history();
        let contents = history
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
}
