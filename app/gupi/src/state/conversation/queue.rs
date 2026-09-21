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
                match result {
                    Ok(queue) if restore_text => {
                        if let Some(draft) = restored_text(queue, &session.draft) {
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
                this.refresh(&target, cx);
                this.changed(cx);
            });
        });
        self.sessions.get_mut(&key).unwrap().command =
            SessionCommand::ClearingQueue { _task: task };
        cx.notify();
    }
}

fn restored_text(queue: protocol::ClearedQueue, current: &str) -> Option<String> {
    let queued = queue
        .steering
        .into_iter()
        .chain(queue.follow_up)
        .collect::<Vec<_>>()
        .join("\n\n");
    if queued.trim().is_empty() {
        return None;
    }
    Some(if current.trim().is_empty() {
        queued
    } else {
        format!("{queued}\n\n{current}")
    })
}

#[cfg(test)]
mod tests {
    use super::restored_text;

    #[test]
    fn restore_follows_tui_order_without_trimming_message_content() {
        let queue = serde_json::from_value(serde_json::json!({
            "steering": [" first ", "same"], "followUp": ["same", "last"]
        }))
        .unwrap();
        assert_eq!(
            restored_text(queue, "draft"),
            Some(" first \n\nsame\n\nsame\n\nlast\n\ndraft".into())
        );
        let empty =
            serde_json::from_value(serde_json::json!({"steering": [], "followUp": []})).unwrap();
        assert_eq!(restored_text(empty, "draft"), None);
    }
}
