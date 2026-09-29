use super::*;

impl ConversationState {
    pub fn can_clear_queue(&self, key: &str, cx: &App) -> bool {
        self.can_submit(key, cx)
            && self
                .sessions
                .get(key)
                .is_some_and(|s| s.pending_count > 0 && !s.stopping)
    }

    /// Pi returns text only. Existing draft attachments remain owned by the editor.
    pub fn clear_queue(&mut self, key: &str, restore_text: bool, cx: &mut Context<Self>) {
        if !self.can_clear_queue(key, cx) {
            return;
        }
        let client = self.client(key, cx).unwrap();
        let binding = self.sessions[key].binding;
        let key = key.to_owned();
        let target = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result = client.clear_queue().await;
            let _ = owner.update(cx, |this, cx| {
                let Some(session) = this
                    .sessions
                    .get_mut(&target)
                    .filter(|s| s.binding == binding)
                else {
                    return;
                };
                session.command.finish();
                let mut draft_changed = false;
                match result {
                    Ok(queue) if restore_text => {
                        if let Some(draft) = restored_content(queue, &session.draft) {
                            draft_changed = session.draft != draft;
                            session.draft = draft;
                            session.draft_revision += 1;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        cx.emit(ConversationEvent::Notify {
                            message: error.to_string(),
                            error: true,
                        });
                    }
                }
                // Queue events are authoritative; a later enqueue must not be
                // erased by this response. The snapshot also refreshes the count.
                this.read_session(&target, ReadScope::State, cx);
                if draft_changed {
                    this.save_changes(cx);
                }
                notify_session(&target, cx);
            });
        });
        self.sessions.get_mut(&key).unwrap().command =
            SessionCommand::ClearingQueue { _task: task };
        notify_session(&key, cx);
    }
}

fn restored_content(queue: protocol::ClearedQueue, current: &InputContent) -> Option<InputContent> {
    let queued = queue
        .steering
        .into_iter()
        .chain(queue.follow_up)
        .collect::<Vec<_>>()
        .join("\n\n");
    if queued.trim().is_empty() {
        return None;
    }
    Some(if current.text().trim().is_empty() {
        queued.into()
    } else {
        let offset = queued.len() + 2;
        let mut content = InputContent::new(format!("{queued}\n\n{}", current.text()));
        for span in current.tokens() {
            let range = span.range();
            content = content
                .with_token(
                    range.start + offset..range.end + offset,
                    span.token().clone(),
                )
                .expect("prefixing text preserves existing token boundaries");
        }
        content
    })
}

#[cfg(test)]
mod tests {
    use super::restored_content;

    #[test]
    fn restore_follows_tui_order_without_trimming_message_content() {
        let queue = serde_json::from_value(serde_json::json!({
            "steering": [" first ", "same"], "followUp": ["same", "last"]
        }))
        .unwrap();
        assert_eq!(
            restored_content(queue, &"draft".into()),
            Some(" first \n\nsame\n\nsame\n\nlast\n\ndraft".into())
        );
        let empty =
            serde_json::from_value(serde_json::json!({"steering": [], "followUp": []})).unwrap();
        assert_eq!(restored_content(empty, &"draft".into()), None);
    }

    #[test]
    fn restoring_queue_keeps_existing_inline_file_references() {
        let token = crate::foundation::composer_resources::file_token(
            std::path::Path::new("/tmp/report.md"),
            false,
        );
        let current = super::InputContent::new(format!("review {}next", token.text()))
            .with_token(7..7 + token.text().len(), token)
            .unwrap();
        let queue = serde_json::from_value(serde_json::json!({"steering":["first"],"followUp":[]}))
            .unwrap();
        let result = restored_content(queue, &current).unwrap();
        assert_eq!(
            result.text().as_ref(),
            "first\n\nreview @/tmp/report.md next"
        );
        assert_eq!(result.tokens()[0].range().start, 14);
        assert_eq!(result.tokens()[0].token(), current.tokens()[0].token());
    }
}
