use crate::{
    database::{Content, ConversationTemplatePrompt, Mode, Role, Status},
    llm::{LlmContentPart, LlmInputItem},
};

pub(crate) struct LlmHistoryMessage<'a> {
    pub(crate) role: Role,
    pub(crate) status: Status,
    pub(crate) content: &'a Content,
    pub(crate) content_parts: Option<&'a [LlmContentPart]>,
}

impl<'a> LlmHistoryMessage<'a> {
    pub(crate) fn new(role: Role, status: Status, content: &'a Content) -> Self {
        Self {
            role,
            status,
            content,
            content_parts: None,
        }
    }

    pub(crate) fn with_content_parts(mut self, content_parts: &'a [LlmContentPart]) -> Self {
        self.content_parts = Some(content_parts);
        self
    }

    fn request_content(&self) -> Vec<LlmContentPart> {
        self.content_parts
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| vec![LlmContentPart::text(self.content.send_content())])
    }
}

pub(crate) fn build_input_items<'a>(
    prompts: &[ConversationTemplatePrompt],
    mode: Mode,
    history_messages: impl IntoIterator<Item = LlmHistoryMessage<'a>>,
    user_message_role: Role,
    user_message_content: Vec<LlmContentPart>,
) -> Vec<LlmInputItem> {
    let mut request_messages = prompts
        .iter()
        .map(|prompt| LlmInputItem::from_role_text(prompt.role, prompt.prompt.clone()))
        .collect::<Vec<_>>();

    request_messages.extend(
        history_messages
            .into_iter()
            .filter(|message| message.status == Status::Normal)
            .filter(|message| match mode {
                Mode::Contextual => true,
                Mode::Single => false,
                Mode::AssistantOnly => message.role == Role::Assistant,
            })
            .map(|message| {
                LlmInputItem::from_role_content(message.role, message.request_content())
            }),
    );
    request_messages.push(LlmInputItem::from_role_content(
        user_message_role,
        user_message_content,
    ));
    request_messages
}

#[cfg(test)]
mod tests {
    use super::{LlmHistoryMessage, build_input_items};
    use crate::{
        database::{Content, ConversationTemplatePrompt, Mode, Role, Status},
        llm::{LlmAttachmentRef, LlmContentPart, LlmInputItem},
    };

    #[test]
    fn contextual_history_includes_normal_messages_and_user() {
        let prompts = vec![ConversationTemplatePrompt {
            prompt: "system".to_string(),
            role: Role::Developer,
        }];
        let user = Content::new("u1");
        let assistant = Content::new("a1");
        let failed = Content::new("bad");

        let items = build_input_items(
            &prompts,
            Mode::Contextual,
            [
                LlmHistoryMessage::new(Role::User, Status::Normal, &user),
                LlmHistoryMessage::new(Role::Assistant, Status::Normal, &assistant),
                LlmHistoryMessage::new(Role::User, Status::Error, &failed),
            ],
            Role::User,
            vec![LlmContentPart::text("latest")],
        )
        .into_iter()
        .map(|item| {
            let (role, text) = item.single_text().expect("text item");
            (role, text.to_string())
        })
        .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                ("developer", "system".to_string()),
                ("user", "u1".to_string()),
                ("assistant", "a1".to_string()),
                ("user", "latest".to_string()),
            ]
        );
    }

    #[test]
    fn assistant_only_filters_history_roles() {
        let prompts = Vec::new();
        let user = Content::new("u1");
        let assistant = Content::new("a1");

        let items = build_input_items(
            &prompts,
            Mode::AssistantOnly,
            [
                LlmHistoryMessage::new(Role::User, Status::Normal, &user),
                LlmHistoryMessage::new(Role::Assistant, Status::Normal, &assistant),
            ],
            Role::User,
            vec![LlmContentPart::text("latest")],
        )
        .into_iter()
        .map(|item| {
            let (role, text) = item.single_text().expect("text item");
            (role, text.to_string())
        })
        .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                ("assistant", "a1".to_string()),
                ("user", "latest".to_string()),
            ]
        );
    }

    #[test]
    fn current_user_message_can_use_content_parts() {
        let items = build_input_items(
            &[],
            Mode::Single,
            [],
            Role::User,
            vec![
                LlmContentPart::text("describe"),
                LlmContentPart::ImageRef(LlmAttachmentRef {
                    id: "data:image/png;base64,abc".to_string(),
                    mime_type: Some("image/png".to_string()),
                    name: Some("screenshot.png".to_string()),
                }),
            ],
        );

        assert!(matches!(&items[0], LlmInputItem::User { content } if content.len() == 2));
    }

    #[test]
    fn history_message_can_use_content_parts() {
        let content = Content::new("fallback");
        let parts = vec![
            LlmContentPart::text("describe"),
            LlmContentPart::ImageRef(LlmAttachmentRef {
                id: "data:image/png;base64,abc".to_string(),
                mime_type: Some("image/png".to_string()),
                name: Some("screenshot.png".to_string()),
            }),
        ];

        let items = build_input_items(
            &[],
            Mode::Contextual,
            [LlmHistoryMessage::new(Role::User, Status::Normal, &content)
                .with_content_parts(&parts)],
            Role::User,
            vec![LlmContentPart::text("latest")],
        );

        assert!(matches!(&items[0], LlmInputItem::User { content } if content == &parts));
    }
}
