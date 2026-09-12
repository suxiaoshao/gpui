use super::*;

impl ConversationState {
    /// A single owned task covers the command and its canonical readback.
    /// None reconciles a previous uncertain result without repeating a write.
    pub(super) fn change_model_setting(
        &mut self,
        key: &str,
        command: Option<protocol::Command>,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client(key, cx) else {
            self.connect(key, cx);
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if self.draining || s.settings_busy() || s.state.is_none() {
            return;
        }
        let binding = s.binding;
        s.model_revision += 1;
        let revision = s.model_revision;
        let session_id = s.state.as_ref().unwrap().session_id.clone();
        s.thinking_levels.reset();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let command_error = if let Some(command) = command {
                client.request(command).await.err().map(|e| e.to_string())
            } else {
                None
            };
            let valid = owner
                .update(cx, |this, cx| {
                    let Some(s) = this
                        .sessions
                        .get_mut(&task_key)
                        .filter(|s| s.binding == binding && s.model_revision == revision)
                    else {
                        return false;
                    };
                    s.model_change.reconciling();
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !valid {
                return;
            }
            let state = client.get_state().await.and_then(|state| {
                if state.session_id != session_id {
                    return Err(pi_rpc::Error::Protocol(
                        "session changed while confirming model settings".into(),
                    ));
                }
                Ok(state)
            });
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding && s.model_revision == revision)
                else {
                    return;
                };
                let confirmed = state.is_ok();
                let refresh_core = match state {
                    Ok(actual) => {
                        let current = s.state.as_mut().unwrap();
                        current.model = actual.model;
                        current.thinking_level = actual.thinking_level;
                        s.model_change.finish(Ok(command_error))
                    }
                    Err(error) => {
                        let error = match command_error {
                            Some(command) => format!("{command}\n{error}"),
                            None => error.to_string(),
                        };
                        s.model_change.finish(Err(error))
                    }
                };
                s.content_revision += 1;
                if confirmed {
                    this.read_thinking(&task_key, cx);
                }
                if refresh_core {
                    this.refresh(&task_key, cx);
                } else if confirmed {
                    this.refresh_stats(&task_key, cx);
                }
                cx.notify();
            });
        });
        let s = self.sessions.get_mut(key).unwrap();
        s.model_change = ModelChange::Applying {
            task,
            refresh_core: false,
        };
        s.content_revision += 1;
        cx.notify();
    }
}
