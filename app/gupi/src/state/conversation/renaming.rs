use super::*;

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
        let connected_client = self.client(&key, cx);
        self.sessions
            .entry(key.clone())
            .or_insert_with(|| Session::new(info.clone(), String::new()));
        // Reuse the normal connection owner and extension event consumer. Pi owns
        // both loading/migrating the session and writing its metadata.
        if self.sessions[&key].instance.is_none() {
            self.connect(&key, cx);
        }
        let initial_binding = self.sessions[&key].binding;
        let target = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let connection = if let Some(client) = connected_client {
                Ok((client, initial_binding))
            } else {
                loop {
                    let next = owner.read_with(cx, |this, cx| {
                        let s = this
                            .sessions
                            .get(&key)
                            .ok_or((initial_binding, "Session was removed".to_owned()))?;
                        if s.binding > initial_binding + 1 {
                            return Err((initial_binding, "Session connection changed".to_owned()));
                        }
                        if let Some(error) = s.error.as_deref().or(s.core_read.error()) {
                            return Err((s.binding, error.to_owned()));
                        }
                        Ok((s.state.is_some() && !s.core_read.running())
                            .then(|| this.client(&key, cx).map(|client| (client, s.binding)))
                            .flatten())
                    });
                    match next {
                        Ok(Ok(Some(connection))) => break Ok(connection),
                        Ok(Ok(None)) => {
                            smol::Timer::after(std::time::Duration::from_millis(20)).await
                        }
                        Ok(Err(error)) => break Err(error),
                        Err(_) => return,
                    };
                }
            };
            let (binding, result) = match connection {
                Ok((client, binding)) => {
                    let result = async {
                        client.ready().await.map_err(|e| e.to_string())?;
                        client
                            .set_session_name(name.clone())
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    }
                    .await;
                    (binding, result)
                }
                Err((binding, error)) => (binding, Err(error)),
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
                                notify_session(other, cx);
                            }
                        }
                        this.sessions.get_mut(&key).unwrap().history_dirty = true;
                    }
                    Err(error) => {
                        s.error = Some(error.clone());
                        cx.emit(ConversationEvent::Notify {
                            message: error,
                            error: true,
                        });
                    }
                }
                notify_session(&key, cx);
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Renaming { _task: task };
        notify_session(&target, cx);
        true
    }
}
