use super::*;
use crate::foundation::i18n::t_with_args;
use fluent_bundle::FluentArgs;

impl ConversationState {
    pub fn can_export(&self, key: &str, cx: &App) -> bool {
        !self.draining
            && self.sessions.get(key).is_some_and(|s| {
                s.state.is_some()
                    && !s.settings_busy()
                    && !s.model_change.unconfirmed()
                    && !s.core_read.running()
                    && !s.info.path.as_os_str().is_empty()
            })
            && self
                .client(key, cx)
                .is_some_and(|client| matches!(client.state(), ConnectionState::Ready(_)))
    }

    pub fn export_html(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.can_export(key, cx) {
            return;
        }
        let client = self.client(key, cx).unwrap();
        let session = &self.sessions[key];
        let suggested = session.info.path.with_extension("html");
        let suggested = suggested.file_name().unwrap().to_string_lossy();
        let prompt = cx.prompt_for_new_path(&session.info.cwd, Some(&suggested));
        let binding = session.binding;
        let target = key.to_owned();
        let task_key = target.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result: Result<Option<protocol::ExportResult>, String> = async {
                let Some(path) = prompt
                    .await
                    .map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())?
                else {
                    return Ok(None);
                };
                client
                    .export_html(path.to_string_lossy().into_owned())
                    .await
                    .map(Some)
                    .map_err(|error| error.to_string())
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                let Some(session) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding)
                else {
                    return;
                };
                session.command.finish();
                match result {
                    Ok(Some(export)) => {
                        let mut args = FluentArgs::new();
                        args.set("path", export.path);
                        cx.emit(ConversationEvent::Notify {
                            message: t_with_args(cx, "conversation-exported", &args),
                            error: false,
                        });
                    }
                    Ok(None) => {}
                    Err(error) => cx.emit(ConversationEvent::Notify {
                        message: error,
                        error: true,
                    }),
                }
                cx.notify();
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Exporting { _task: task };
        cx.notify();
    }
}
