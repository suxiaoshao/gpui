use super::*;
use crate::foundation::session_file;

impl ConversationState {
    fn rename_target(&self, key: &str, cx: &App) -> Option<(String, SessionInfo)> {
        if self.draining {
            return None;
        }
        let info = self.sessions.get(key).map(|s| &s.info).or_else(|| {
            self.catalog
                .data()?
                .sessions
                .iter()
                .find(|info| info.key() == key)
        })?;
        // A catalog menu can outlive opening the same file under a local draft key.
        let key = self
            .sessions
            .iter()
            .find(|(_, s)| {
                !info.path.as_os_str().is_empty()
                    && s.info.path == info.path
                    && s.instance.is_some()
            })
            .map(|(key, _)| key.as_str())
            .unwrap_or(key);
        if self.sessions.iter().any(|(other, s)| {
            (other == key || !info.path.as_os_str().is_empty() && s.info.path == info.path)
                && (s.settings_busy() || s.core_read.running())
        }) {
            return None;
        }
        if let Some(s) = self.sessions.get(key) {
            if s.instance.is_some() {
                if s.state.is_none()
                    || !matches!(self.client(key, cx)?.state(), ConnectionState::Ready(_))
                {
                    return None;
                }
            } else if info.path.as_os_str().is_empty() {
                return None;
            }
        } else if info.path.as_os_str().is_empty() {
            return None;
        }
        Some((key.to_owned(), info.clone()))
    }

    pub fn can_rename(&self, key: &str, cx: &App) -> bool {
        self.rename_target(key, cx).is_some()
    }

    /// Returns whether the operation was accepted; rejected confirmation keeps the dialog open.
    pub fn rename(&mut self, key: &str, name: String, cx: &mut Context<Self>) -> bool {
        let name = name
            .split(['\r', '\n'])
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned();
        if name.is_empty() {
            return false;
        }
        let Some((key, info)) = self.rename_target(key, cx) else {
            return false;
        };
        let client = self.client(&key, cx);
        let connected = client.is_some();
        self.sessions
            .entry(key.clone())
            .or_insert_with(|| Session::new(info.clone(), String::new()));
        let binding = self.sessions[&key].binding;
        let target = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result = if let Some(client) = client {
                client
                    .set_session_name(name.clone())
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                let info = info.clone();
                let name = name.clone();
                smol::unblock(move || session_file::rename(&info, &name))
                    .await
                    .map_err(|e| e.to_string())
            };
            let _ = owner.update(cx, |this, cx| {
                let Some(s) = this.sessions.get_mut(&key).filter(|s| s.binding == binding) else {
                    return;
                };
                s.command.finish();
                match result {
                    Ok(()) => {
                        for (other, s) in &mut this.sessions {
                            if other == &key
                                || !info.path.as_os_str().is_empty() && s.info.path == info.path
                            {
                                s.info.name = Some(name.clone());
                                if let Some(state) = &mut s.state {
                                    state.session_name = Some(name.clone());
                                }
                                s.error = None;
                            }
                        }
                        if connected {
                            this.refresh(&key, cx);
                        }
                        this.request_scan(cx);
                    }
                    Err(error) => {
                        s.error = Some(error.clone());
                        cx.emit(ConversationEvent::Notify {
                            message: error,
                            error: true,
                        });
                    }
                }
                // Opening the session while its metadata write is running waits
                // on that write before starting the normal connection path.
                if !connected
                    && this.selected.as_ref() == Some(&key)
                    && matches!(this.sessions[&key].transcript, Transcript::Unloaded)
                {
                    this.connect(&key, cx);
                }
                this.changed(cx);
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Renaming { _task: task };
        cx.notify();
        true
    }
}
