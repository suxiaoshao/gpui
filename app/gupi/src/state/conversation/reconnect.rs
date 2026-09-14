//! Manual reload hands one session's file from an exited Pi to a new instance.
use super::*;
use crate::foundation::i18n::t;

impl ConversationState {
    pub fn can_reconnect(&self, key: &str, cx: &App) -> bool {
        !self.draining
            && self.sessions.get(key).is_some_and(|s| {
                !s.settings_busy()
                    && !s.core_read.running()
                    && !s.model_change.unconfirmed()
                    && (s.instance.is_none()
                        || (s.state.is_some() || s.core_read.error().is_some())
                            && self.client(key, cx).is_some_and(|client| {
                                matches!(client.state(), ConnectionState::Ready(_))
                            }))
                    && (s.state.is_some() || !s.info.path.as_os_str().is_empty())
            })
    }

    pub fn reconnect(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.can_reconnect(key, cx) {
            return;
        }
        // A live connection with a failed core query only needs another read.
        if self.sessions[key].core_read.error().is_some() && self.client(key, cx).is_some() {
            self.refresh(key, cx);
            return;
        }
        let s = self.sessions.get_mut(key).unwrap();
        let mut options = LaunchOptions::new(self.command.clone(), s.info.cwd.clone());
        if let Some(state) = &s.state {
            if let Some(model) = &state.model {
                options.args.extend([
                    "--provider".into(),
                    model.provider.clone().into(),
                    "--model".into(),
                    model.id.clone().into(),
                ]);
            }
            options
                .args
                .extend(["--thinking".into(), state.thinking_level.clone().into()]);
        }
        let info = s.info.clone();
        let closing = s
            .instance
            .take()
            .map(|id| pi::global(cx).update(cx, |pi, cx| pi.close(id, cx)));
        s.binding += 1;
        let binding = s.binding;
        s.reset_reads();
        s.clear_extension_ui();
        s.error = None;
        let key = key.to_owned();
        let target = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            if let Some(closing) = closing {
                let report = closing.await;
                if report.as_ref().is_none_or(|report| report.status.is_none()) {
                    let _ = owner.update(cx, |this, cx| {
                        if let Some(s) = this
                            .sessions
                            .get_mut(&target)
                            .filter(|s| s.binding == binding)
                        {
                            s.error = Some(t(cx, "conversation-reconnect-unconfirmed"));
                            s.command = SessionCommand::ReconnectUnconfirmed;
                        }
                        cx.notify();
                    });
                    return;
                }
            }
            // Paths recorded in SessionInfo refer to real history, unlike Pi's
            // preallocated sessionFile for a never-persisted empty session.
            let checked = smol::unblock(move || {
                if !info.path.as_os_str().is_empty() {
                    let metadata =
                        session_catalog::read_metadata(&info.path).map_err(|e| e.to_string())?;
                    if metadata.id != info.id || metadata.cwd != info.cwd {
                        return Err("Session identity or working directory changed.".to_owned());
                    }
                    options
                        .args
                        .extend(["--session".into(), info.path.into_os_string()]);
                }
                Ok(options)
            })
            .await;
            let _ = owner.update(cx, |this, cx| {
                if this.draining
                    || this
                        .sessions
                        .get(&target)
                        .is_none_or(|s| s.binding != binding)
                {
                    return;
                }
                let started = checked.and_then(|options| {
                    pi::global(cx)
                        .update(cx, |pi, cx| pi.start(options, cx))
                        .map_err(|e| e.to_string())
                });
                let s = this.sessions.get_mut(&target).unwrap();
                match started {
                    Ok(id) => {
                        s.instance = Some(id);
                        s.state = None;
                        // Reconnecting remains active through the first core
                        // snapshot; normal binding-aware reads complete it.
                        s.connection_purpose = ConnectionPurpose::Conversation;
                    }
                    Err(error) => {
                        s.error = Some(error);
                        s.command.finish();
                    }
                }
                cx.notify();
            });
        });
        self.sessions.get_mut(&key).unwrap().command = SessionCommand::Reconnecting { _task: task };
        cx.notify();
    }
}
