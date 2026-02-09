use crate::{
    components::chat_input::{ChatInput, Send, input_state},
    config::AiChatConfig,
    database::{
        Content, Conversation, ConversationTemplate, Db, Message, Mode, NewMessage, Role, Status,
    },
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    fetch::FetchRunner,
    store::{ChatData, ChatDataInner},
};
use async_compat::CompatExt;
use futures::pin_mut;
use gpui::{
    AppContext, AsyncWindowContext, Context, Entity, IntoElement, InteractiveElement, ParentElement,
    Render, SharedString, Styled, Subscription, Task, WeakEntity, Window, div,
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
    task: Option<Task<()>>,
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
            self.task = Some(cx.spawn_in(window, async move |this, cx| {
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
                    let _ = this.update(cx, |this, _cx| {
                        this.task = None;
                    });
                }
            }));
        }
    }

    async fn fetch(
        state: WeakEntity<Self>,
        context: FetchContext,
        cx: &mut AsyncWindowContext,
    ) -> AiChatResult<()> {
        event!(Level::INFO, "conversation fetch");
        let extension_runner = match context.extension_name.as_ref() {
            Some(extension_name) => Some(
                context
                    .extension_container
                    .get_extension(extension_name)
                    .await?,
            ),
            None => None,
        };
        state
            .update_in(cx, |this, window, cx| {
                this.input_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            })
            .map_err(|_| AiChatError::GpuiError)?;

        let (conversation, template, user_message, assistant_message) = cx
            .read_global(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                let conversation = Conversation::find(context.conversation_id, conn)?;
                let template = ConversationTemplate::find(conversation.template_id, conn)?;
                let user_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        Role::User,
                        Content::Text(context.text.to_string()),
                        serde_json::Value::Null,
                        Status::Normal,
                    ),
                    conn,
                )?;
                let assistant_message = Message::insert(
                    NewMessage::new(
                        context.conversation_id,
                        Role::Assistant,
                        Content::Text(String::new()),
                        serde_json::Value::Null,
                        Status::Loading,
                    ),
                    conn,
                )?;
                Ok::<_, AiChatError>((conversation, template, user_message, assistant_message))
            })
            .map_err(|_| AiChatError::GpuiError)??;
        let user_message_id = user_message.id;
        let assistant_message_id = assistant_message.id;

        context
            .chat_data
            .update(cx, |data, cx| {
                if let Ok(data) = data {
                    data.add_message(context.conversation_id, user_message);
                    data.add_message(context.conversation_id, assistant_message);
                    cx.notify();
                }
            })
            .map_err(|_| AiChatError::GpuiError)?;

        let user_content =
            Runner::get_new_user_content(context.text.to_string(), extension_runner).await?;
        let user_message = cx
            .read_global(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                Message::update_content(user_message_id, &user_content, conn)?;
                Message::find(user_message_id, conn)
            })
            .map_err(|_| AiChatError::GpuiError)??;
        let user_message_role = user_message.role;
        let user_message_content = user_message.content.send_content().to_string();
        context
            .chat_data
            .update(cx, |data, cx| {
                if let Ok(data) = data {
                    data.replace_message(context.conversation_id, user_message);
                    cx.notify();
                }
            })
            .map_err(|_| AiChatError::GpuiError)?;

        let runner = Runner {
            config: context.config,
            template,
            history_messages: conversation.messages,
            user_message_role,
            user_message_content,
        };

        let stream = runner.fetch();
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(message) => {
                    let message = cx
                        .read_global(|db: &Db, _window, _cx| {
                            let conn = &mut db.get()?;
                            Message::add_content(assistant_message_id, message, conn)?;
                            Message::find(assistant_message_id, conn)
                        })
                        .map_err(|_| AiChatError::GpuiError)??;
                    context
                        .chat_data
                        .update(cx, |data, cx| {
                            if let Ok(data) = data {
                                data.replace_message(context.conversation_id, message);
                                cx.notify();
                            }
                        })
                        .map_err(|_| AiChatError::GpuiError)?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    let message = cx
                        .read_global(|db: &Db, _window, _cx| {
                            let conn = &mut db.get()?;
                            Message::update_status(assistant_message_id, Status::Error, conn)?;
                            Message::find(assistant_message_id, conn)
                        })
                        .map_err(|_| AiChatError::GpuiError)??;
                    context
                        .chat_data
                        .update(cx, |data, cx| {
                            if let Ok(data) = data {
                                data.replace_message(context.conversation_id, message);
                                cx.notify();
                            }
                        })
                        .map_err(|_| AiChatError::GpuiError)?;
                    state
                        .update(cx, |this, _cx| {
                            this.task = None;
                        })
                        .map_err(|_| AiChatError::GpuiError)?;
                    return Ok(());
                }
            }
        }

        let message = cx
            .read_global(|db: &Db, _window, _cx| {
                let conn = &mut db.get()?;
                Message::update_status(assistant_message_id, Status::Normal, conn)?;
                Message::find(assistant_message_id, conn)
            })
            .map_err(|_| AiChatError::GpuiError)??;
        context
            .chat_data
            .update(cx, |data, cx| {
                if let Ok(data) = data {
                    data.replace_message(context.conversation_id, message);
                    cx.notify();
                }
            })
            .map_err(|_| AiChatError::GpuiError)?;
        state
            .update(cx, |this, _cx| {
                this.task = None;
            })
            .map_err(|_| AiChatError::GpuiError)?;
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
                            .disabled(self.task.is_some())
                            .on_action(cx.listener(Self::on_send_action)),
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

    fn get_history(&self) -> Vec<crate::fetch::Message> {
        use crate::fetch::Message as FetchMessage;
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
            status,
            created_time: now,
            updated_time: now,
            start_time: now,
            end_time: now,
        }
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
            make_message(1, Role::User, Status::Normal, Content::Text("u1".to_string())),
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
            make_message(3, Role::User, Status::Error, Content::Text("bad".to_string())),
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
            make_message(1, Role::User, Status::Normal, Content::Text("u1".to_string())),
            make_message(2, Role::Assistant, Status::Normal, Content::Text("a1".to_string())),
            make_message(3, Role::Assistant, Status::Error, Content::Text("bad".to_string())),
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
