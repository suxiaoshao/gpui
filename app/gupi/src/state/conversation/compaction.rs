use super::*;

impl ConversationState {
    pub fn can_compact(&self, key: &str, cx: &App) -> bool {
        !self.draining
            && self.sessions.get(key).is_some_and(|s| {
                s.state.is_some()
                    && !s.compacting
                    && !s.stopping
                    && !s.submitting()
                    && !s.command.running()
                    && !s.model_change.running()
                    && !s.model_change.unconfirmed()
                    && !s.core_read.running()
                    && s.pending_ui.is_empty()
            })
            && self
                .client(key, cx)
                .is_some_and(|client| matches!(client.state(), ConnectionState::Ready(_)))
    }

    pub fn compact(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.can_compact(key, cx) {
            return;
        }
        let client = self.client(key, cx).unwrap();
        let binding = self.sessions[key].binding;
        let target = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            // Pi stops an active turn before compacting, as in its TUI.
            let result = client.compact().await;
            let _ = owner.update(cx, |this, cx| {
                let Some(session) = this
                    .sessions
                    .get_mut(&target)
                    .filter(|s| s.binding == binding)
                else {
                    return;
                };
                session.command.finish();
                if let Err(error) = result {
                    cx.emit(ConversationEvent::Notify {
                        message: error.to_string(),
                        error: true,
                    });
                }
                this.refresh(&target, cx);
                cx.notify();
            });
        });
        let session = self.sessions.get_mut(key).unwrap();
        session.interrupted = false;
        session.command = SessionCommand::Compacting { _task: task };
        cx.notify();
    }
}
