use crate::{
    database::{Content, ConversationTemplatePrompt, Mode, Role, Status},
    llm::LlmInputItem,
};

pub(crate) struct LlmHistoryMessage<'a> {
    pub(crate) role: Role,
    pub(crate) status: Status,
    pub(crate) content: &'a Content,
}

impl<'a> LlmHistoryMessage<'a> {
    pub(crate) fn new(role: Role, status: Status, content: &'a Content) -> Self {
        Self {
            role,
            status,
            content,
        }
    }
}

pub(crate) fn build_input_items<'a>(
    prompts: &[ConversationTemplatePrompt],
    mode: Mode,
    history_messages: impl IntoIterator<Item = LlmHistoryMessage<'a>>,
    user_message_role: Role,
    user_message_content: &str,
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
                LlmInputItem::from_role_text(message.role, message.content.send_content())
            }),
    );
    request_messages.push(LlmInputItem::from_role_text(
        user_message_role,
        user_message_content,
    ));
    request_messages
}

#[cfg(test)]
mod tests {
    use super::{LlmHistoryMessage, build_input_items};
    use crate::database::{Content, ConversationTemplatePrompt, Mode, Role, Status};

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
            "latest",
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
            "latest",
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
}
