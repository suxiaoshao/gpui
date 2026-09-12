use super::*;
use crate::foundation::i18n::t_with_args;
use fluent_bundle::FluentArgs;

impl ConversationState {
    fn deletion_target(&self, key: &str) -> Option<SessionInfo> {
        if self.draining {
            return None;
        }
        let info = self.infos().into_iter().find(|(k, _)| k == key)?.1;
        if info.path.as_os_str().is_empty()
            || self.sessions.values().any(|s| {
                s.info.path == info.path
                    && (s.settings_busy()
                        || s.core_read.running()
                        || s.instance.is_some() && s.state.is_none())
            })
        {
            return None;
        }
        Some(info)
    }

    pub fn can_delete(&self, key: &str) -> bool {
        self.deletion_target(key).is_some()
    }

    pub fn delete(&mut self, key: &str, cx: &mut Context<Self>) {
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
        for s in self
            .sessions
            .values_mut()
            .filter(|s| s.info.path == info.path)
        {
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
                let target = info.clone();
                smol::unblock(move || remove(&target)).await
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this.sessions.get_mut(&task_key) {
                    s.command.finish();
                }
                match result {
                    Ok(()) => {
                        this.sessions.retain(|_, s| s.info.path != info.path);
                        this.catalog
                            .transition(CatalogMessage::RemoveSession(info.path));
                        this.insert_draft(Some(info.cwd));
                        this.request_scan(cx);
                        this.changed(cx);
                        cx.emit(ConversationEvent::Deleted);
                    }
                    Err(error) => {
                        let mut args = FluentArgs::new();
                        args.set("error", error);
                        cx.emit(ConversationEvent::Notify {
                            message: t_with_args(cx, "conversation-delete-failed", &args),
                            error: true,
                        });
                        cx.notify();
                    }
                }
            });
        });
        self.sessions.get_mut(&key).unwrap().command = SessionCommand::Deleting { task };
        cx.notify();
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
