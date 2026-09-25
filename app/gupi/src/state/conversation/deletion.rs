use super::*;
use crate::foundation::i18n::t_with_args;
use fluent_bundle::FluentArgs;

impl ConversationState {
    fn deletion_target(&self, key: &str) -> Option<SessionInfo> {
        if self.draining {
            return None;
        }
        let info = self.infos().into_iter().find(|(k, _)| k == key)?.1;
        if self.sessions.iter().any(|(candidate, session)| {
            matches_target(key, &info, candidate, session)
                && (session.settings_busy()
                    || session.core_read.running()
                    || session.instance.is_some() && session.state.is_none())
        }) {
            return None;
        }
        Some(info)
    }

    pub fn can_delete(&self, key: &str) -> bool {
        if self.temporary {
            return self.can_delete_temporary(key);
        }
        self.deletion_target(key).is_some()
    }

    pub fn delete(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.temporary {
            self.delete_temporary(key, cx);
            return;
        }
        self.delete_with(key, move_to_trash, cx);
    }

    pub(super) fn delete_with(
        &mut self,
        key: &str,
        remove: impl FnOnce(&SessionInfo) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(info) = self.deletion_target(key) else {
            return;
        };
        // Catalog-only sessions do not need a Pi process to be removed.
        self.sessions
            .entry(key.to_owned())
            .or_insert_with(|| Session::new(info.clone(), String::new()));
        let mut closing = Vec::new();
        for (candidate, s) in &mut self.sessions {
            if !matches_target(key, &info, candidate, s) {
                continue;
            }
            s.binding += 1;
            s.reset_reads();
            s.state = None;
            s.clear_extension_ui();
            if let Some(id) = s.instance.take() {
                closing.push(pi::global(cx).update(cx, |pi, cx| pi.close(id, cx)));
            }
        }
        let key = key.to_owned();
        let task_key = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result = async {
                for close in closing {
                    if let Some(report) = close.await
                        && report.status.is_none()
                    {
                        return Err("Pi process exit could not be confirmed".to_owned());
                    }
                }
                if info.path.as_os_str().is_empty() {
                    Ok(())
                } else {
                    let target = info.clone();
                    smol::unblock(move || remove(&target)).await
                }
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this.sessions.get_mut(&task_key) {
                    s.command.finish();
                }
                match result {
                    Ok(()) => {
                        let removed: Vec<_> = this
                            .sessions
                            .iter()
                            .filter(|(key, session)| matches_target(&task_key, &info, key, session))
                            .map(|(key, _)| key.clone())
                            .collect();
                        let selected_deleted = this
                            .selected
                            .as_ref()
                            .is_some_and(|key| removed.contains(key));
                        let saved_draft = removed
                            .iter()
                            .any(|key| !this.sessions[key].draft.is_empty());
                        for key in removed {
                            this.sessions.remove(&key);
                            notify_session(&key, cx);
                        }
                        if !info.path.as_os_str().is_empty() {
                            notify_session(&info.key(), cx);
                            this.catalog
                                .transition(CatalogMessage::RemoveSession(info.path));
                            this.discover_pending_projects(cx);
                        }
                        if selected_deleted {
                            this.selected = None;
                            notify_selection(cx);
                        }
                        if saved_draft {
                            this.save_changes(cx);
                        }
                        notify_progress(cx);
                    }
                    Err(error) => {
                        let mut args = FluentArgs::new();
                        args.set("error", error);
                        cx.emit(ConversationEvent::Notify {
                            message: t_with_args(cx, "conversation-delete-failed", &args),
                            error: true,
                        });
                        notify_session(&task_key, cx);
                    }
                }
            });
        });
        self.sessions.get_mut(&key).unwrap().command = SessionCommand::Deleting { task };
        notify_session(&key, cx);
    }
}

// Unsaved sessions all have an empty path, but remain distinct conversations.
// Persisted sessions may also have a live draft key for the same file.
fn matches_target(key: &str, info: &SessionInfo, candidate: &str, session: &Session) -> bool {
    if info.path.as_os_str().is_empty() {
        candidate == key
    } else {
        session.info.path == info.path
    }
}

fn move_to_trash(info: &SessionInfo) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(&info.path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Session path is no longer a regular file".into());
    }
    let current = session_catalog::read_metadata(&info.path).map_err(|e| e.to_string())?;
    if current.id != info.id || current.cwd != info.cwd {
        return Err(
            "Session identity or working directory changed; refresh the session catalog".into(),
        );
    }
    let context = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    let context = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = context;
        context.set_delete_method(DeleteMethod::NsFileManager);
        context
    };
    context.delete(&info.path).map_err(|e| e.to_string())
}
