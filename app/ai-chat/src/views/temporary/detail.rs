use crate::{
    components::{
        chat_input::{ChatInput, Send, input_state},
        message::{MessageView, MessageViewExt},
    },
    config::AiChatConfig,
    database::{Content, ConversationTemplate, Role, Status},
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionContainer,
    fetch::FetchRunner,
    i18n::I18n,
    views::{
        message_preview::{MessagePreview, MessagePreviewExt},
        temporary::TemporaryView,
    },
};
use async_compat::CompatExt;
use fluent_bundle::FluentArgs;
use futures::pin_mut;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Root, WindowExt,
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
use std::rc::Rc;
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
    pub status: Status,
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

    fn id(&self) -> Self::Id {
        self.id
    }

    fn status(&self) -> &Status {
        &self.status
    }

    fn open_view_by_id(id: Self::Id, _window: &mut Window, cx: &mut App) {
        let message = cx.windows().iter().find_map(|window| {
            window
                .downcast::<Root>()
                .and_then(|window_root| window_root.read(cx).ok())
                .and_then(|root| root.view().downgrade().upgrade())
                .and_then(|view| view.downcast::<TemporaryView>().ok())
                .and_then(|temporary_view| {
                    let temporary_view = temporary_view.read(cx);
                    let template_detail = temporary_view.selected_item.as_ref()?.clone();
                    let template_detail = template_detail.read(cx);
                    template_detail
                        .messages
                        .iter()
                        .find(|message| message.id == id)
                        .cloned()
                })
        });
        let Some(message) = message else {
            return;
        };
        let title = {
            let i18n = cx.global::<I18n>();
            let mut args = FluentArgs::new();
            args.set("id", message.id as i64);
            i18n.t_with_args("message-preview-title", &args)
        };
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.), px(600.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let message_view = cx.new(|cx| MessagePreview::new(message.clone(), window, cx));
                cx.new(|cx| Root::new(message_view, window, cx))
            },
        ) {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "open message view window: {}", err);
            }
        };
    }

    fn delete_message_by_id(message_id: Self::Id, window: &mut Window, cx: &mut App) {
        let temporary_view = cx.windows().iter().find_map(|window| {
            window
                .downcast::<Root>()
                .and_then(|window_root| window_root.read(cx).ok())
                .and_then(|root| root.view().downgrade().upgrade())
                .and_then(|view| view.downcast::<TemporaryView>().ok())
        });
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
}

impl MessagePreviewExt for TemporaryMessage {
    fn on_update_content(&self, content: Content, cx: &mut App) -> AiChatResult<()> {
        let temporary_view = cx.windows().iter().find_map(|window| {
            window
                .downcast::<Root>()
                .and_then(|window_root| window_root.read(cx).ok())
                .and_then(|root| root.view().downgrade().upgrade())
                .and_then(|view| view.downcast::<TemporaryView>().ok())
        });
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
    fn update_status(&mut self, status: Status) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.updated_time = now;
        self.end_time = now;
    }
}

pub(crate) struct TemplateDetailView {
    focus_handle: FocusHandle,
    template: ConversationTemplate,
    on_esc: OnEsc,
    messages: Vec<TemporaryMessage>,
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<SearchableVec<String>>>,
    _subscriptions: Vec<Subscription>,
    task: Option<Task<()>>,
    autoincrement_id: usize,
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
        if self.task.is_none() && !text.is_empty() {
            let config = cx.global::<AiChatConfig>().clone();
            let extension_container = cx.global::<ExtensionContainer>().clone();
            self.task = Some(cx.spawn_in(window, async move |this, cx| {
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
            }));
        }
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
        status: Status,
    ) -> &mut TemporaryMessage {
        let id = self.new_id();
        let message = TemporaryMessage {
            id,
            role,
            content,
            status,
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
    fn on_error(&mut self, message_id: usize) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Error);
        }
        self.task = None;
    }
    fn on_success(&mut self, message_id: usize) {
        if let Some(last) = self.messages.iter_mut().find(|m| m.id == message_id) {
            last.update_status(Status::Normal);
        }
        self.task = None;
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
        let extension_runner = match extension_name {
            Some(extension_name) => Some(extension_container.get_extension(extension_name).await?),
            None => None,
        };
        let now = OffsetDateTime::now_utc();
        let user_message_id = state
            .update_in(cx, |this, window, cx| {
                this.input_state.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                this.add_message(
                    now,
                    Role::User,
                    Content::Text(text.to_string()),
                    Status::Loading,
                )
                .id
            })
            .map_err(|_| AiChatError::GpuiError)?;
        let content = Runner::get_new_user_content(text.to_string(), extension_runner).await?;
        state
            .update(cx, |this, _cx| {
                if let Some(message) = this
                    .messages
                    .iter_mut()
                    .find(|message| message.id == user_message_id)
                {
                    message.content = content;
                    message.status = Status::Normal;
                }
            })
            .map_err(|_| AiChatError::GpuiError)?;

        let template = state
            .read_with(cx, |this, _cx| this.template.clone())
            .map_err(|_| AiChatError::GpuiError)?;
        let messages = state
            .read_with(cx, |this, _cx| this.messages.clone())
            .map_err(|_| AiChatError::GpuiError)?;
        let assistant_message_id = state
            .update(cx, |this, _cx| {
                this.add_message(
                    now,
                    Role::Assistant,
                    Content::Text(String::new()),
                    Status::Loading,
                )
                .id
            })
            .map_err(|_| AiChatError::GpuiError)?;
        let runner = Runner {
            config,
            template,
            messages,
        };

        let stream = runner.fetch();
        pin_mut!(stream);
        while let Some(message) = stream.next().await {
            match message {
                Ok(message) => {
                    state
                        .update(cx, |this, _cx| {
                            this.on_message(&message, assistant_message_id);
                        })
                        .map_err(|_| AiChatError::GpuiError)?;
                }
                Err(error) => {
                    event!(Level::ERROR, "Connection Error: {}", error);
                    state
                        .update(cx, |this, _cx| {
                            this.on_error(assistant_message_id);
                        })
                        .map_err(|_| AiChatError::GpuiError)?;
                }
            }
        }
        state
            .update(cx, |this, _cx| {
                this.on_success(assistant_message_id);
            })
            .map_err(|_| AiChatError::GpuiError)?;
        Ok(())
    }
}

struct Runner {
    config: AiChatConfig,
    template: ConversationTemplate,
    messages: Vec<TemporaryMessage>,
}

impl FetchRunner for Runner {
    fn get_adapter(&self) -> &str {
        &self.template.adapter
    }

    fn get_template(&self) -> &serde_json::Value {
        &self.template.template
    }

    fn get_config(&self) -> &crate::config::AiChatConfig {
        &self.config
    }

    fn get_history(&self) -> Vec<crate::fetch::Message> {
        use crate::fetch::Message as FetchMessage;
        let mut prompts = self
            .template
            .prompts
            .iter()
            .map(|prompt| FetchMessage::new(prompt.role, prompt.prompt.clone()))
            .collect::<Vec<_>>();

        let messages = self
            .messages
            .iter()
            .filter(|message| message.status == Status::Normal)
            .map(|message| {
                FetchMessage::new(message.role, message.content.send_content().to_string())
            });

        prompts.extend(messages);
        prompts
    }
}

impl Render for TemplateDetailView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let on_esc = self.on_esc.clone();
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
                    .child(Label::new(&self.template.icon))
                    .child(
                        Label::new(&self.template.name)
                            .text_xl()
                            .when_some(self.template.description.as_ref(), |this, description| {
                                this.secondary(description)
                            }),
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
                            .disabled(self.task.is_some())
                            .on_action(cx.listener(Self::on_send_action)),
                    )
                    .px_2(),
            )
    }
}
