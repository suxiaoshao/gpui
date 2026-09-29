//! Persistent plugin messages share message primitives and preserve Pi block order.
use super::*;

impl HomeView {
    pub(super) fn render_plugin(
        &self,
        key: &str,
        row: &str,
        message: &DisplayMessage,
        cx: &App,
    ) -> AnyElement {
        let mut content = MessageContent::new();
        if let Some(parts) = message.value["content"].as_array() {
            for (index, part) in parts.iter().enumerate() {
                let id = format!("plugin-{}-{index}", message.id);
                match part["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = part["text"].as_str().filter(|text| !text.is_empty()) {
                            content = content.child(
                                div()
                                    .id(format!("{key}-{id}"))
                                    .test_support()
                                    .child(self.text_view(key, row, id, text.to_owned())),
                            );
                        }
                    }
                    Some("image") => {
                        content = content.child(images::MessageImage {
                            preview_host: self.image_preview.clone(),
                            id: format!("{key}-{id}"),
                            mime: part["mimeType"].as_str().unwrap_or_default().into(),
                            data: part["data"].as_str().unwrap_or_default().into(),
                        });
                    }
                    _ => {}
                }
            }
        } else if let Some(text) = message.value["content"].as_str() {
            content = content.child(self.text_view(
                key,
                row,
                format!("plugin-{}", message.id),
                text.to_owned(),
            ));
        }
        let title = message.value["customType"]
            .as_str()
            .filter(|title| !title.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| t(cx, "conversation-plugin-message"));
        div()
            .id(format!("plugin-message-{key}-{}", message.id))
            .test_support()
            .w_full()
            .child(
                Message::new()
                    .header(MessageHeader::new().content_inset(false).child(title))
                    .content(content)
                    .when(!message.text().is_empty(), |message_view| {
                        message_view.footer(MessageFooter::new().content_inset(false).child(
                            actions::MessageActions {
                                id: format!("{key}-{}", message.id),
                                message: message.clone(),
                                text: message.text(),
                                before_copy: None,
                            },
                        ))
                    }),
            )
            .into_any_element()
    }
}
