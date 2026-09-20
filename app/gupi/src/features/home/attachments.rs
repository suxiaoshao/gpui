use super::*;
use crate::foundation::attachments::{self, Attachment, Content};
use gpui_kit::prelude::FluentBuilder;

impl HomeView {
    fn attachment_target(&self, cx: &App) -> Option<String> {
        let key = self.shown_key.as_ref()?;
        let session = self.state.read(cx).sessions.get(key)?;
        (session.can_edit_draft() && session.attachments_read.is_none()).then(|| key.clone())
    }
    pub(super) fn choose_attachments(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.attachment_target(cx) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        self.load_attachments(
            key,
            async move {
                let paths = paths
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                match paths {
                    Some(paths) => smol::unblock(move || attachments::from_paths(paths)).await,
                    None => Ok(Vec::new()),
                }
            },
            cx,
        );
    }
    pub(super) fn attach_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(key) = self.attachment_target(cx) else {
            return;
        };
        self.load_attachments(
            key,
            async move { smol::unblock(move || attachments::from_paths(paths)).await },
            cx,
        );
    }
    pub(super) fn paste_attachments(
        &mut self,
        item: &ClipboardItem,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(paths) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::ExternalPaths(paths) => Some(paths.paths().to_vec()),
            _ => None,
        }) {
            self.attach_paths(paths, cx);
            true
        } else if let Some(bytes) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image.bytes().to_vec()),
            _ => None,
        }) {
            let Some(key) = self.attachment_target(cx) else {
                return true;
            };
            let name = t(cx, "attachment-clipboard");
            self.load_attachments(
                key,
                async move {
                    smol::unblock(move || Attachment::from_image(name, &bytes).map(|a| vec![a]))
                        .await
                },
                cx,
            );
            true
        } else {
            false
        }
    }
    fn load_attachments(
        &mut self,
        key: String,
        future: impl Future<Output = Result<Vec<Attachment>, String>> + 'static,
        cx: &mut Context<Self>,
    ) {
        let state = self.state.clone();
        state.update(cx, |state, cx| {
            let target = key.clone();
            let cwd = state.sessions[&key].info.cwd.clone();
            let task = cx.spawn(async move |state, cx| {
                let result = match future.await {
                    Ok(items) => {
                        if items
                            .iter()
                            .any(|a| matches!(a.content, Content::Image { .. }))
                        {
                            smol::unblock(move || {
                                attachments::check_image_policy(&cwd).map(|_| items)
                            })
                            .await
                        } else {
                            Ok(items)
                        }
                    }
                    Err(e) => Err(e),
                };
                let _ = state.update(cx, |state, cx| {
                    if let Some(session) = state.sessions.get_mut(&target) {
                        session.attachments_read = None;
                        match result {
                            Ok(items) => session.attachments.extend(items),
                            Err(error) => session.error = Some(error),
                        }
                        cx.notify();
                    }
                });
            });
            if let Some(session) = state.sessions.get_mut(&key) {
                session.attachments_read = Some(task);
            }
            cx.notify();
        });
    }
    pub(super) fn render_attachments(&self, cx: &Context<Self>) -> AnyElement {
        let Some(session) = self.state.read(cx).current() else {
            return div().into_any_element();
        };
        let key = self.shown_key.clone().unwrap_or_default();
        let editable = session.can_edit_draft();
        h_flex()
            .flex_wrap()
            .gap_2()
            .children(session.attachments.iter().map(|attachment| {
                let target = key.clone();
                let id = attachment.id.clone();
                let content = attachment.content.clone();
                let name = attachment.name.clone();
                h_flex()
                    .gap_1()
                    .child(
                        Button::new(format!("preview-{id}"))
                            .small()
                            .label(name.clone())
                            .when(matches!(content, Content::File(_)), |button| {
                                button.icon(IconName::FileText)
                            })
                            .when_some(
                                match &content {
                                    Content::Image { preview, .. } => Some(preview.clone()),
                                    _ => None,
                                },
                                |button, preview| {
                                    button
                                        .child(img(preview).size_6().object_fit(ObjectFit::Contain))
                                },
                            )
                            .on_click(move |_, window, cx| match &content {
                                Content::File(path) => cx.open_with_system(path),
                                Content::Image { preview, .. } => {
                                    let preview = preview.clone();
                                    let name = name.clone();
                                    window.open_dialog(cx, move |dialog, _, _| {
                                        dialog.title(name.clone()).child(
                                            img(preview.clone())
                                                .w_full()
                                                .h_64()
                                                .object_fit(ObjectFit::Contain),
                                        )
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new(format!("remove-{id}"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .disabled(!editable)
                            .tooltip(t(cx, "attachment-remove"))
                            .accessibility_label(t(cx, "attachment-remove"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.update(cx, |state, cx| {
                                    if let Some(session) = state.sessions.get_mut(&target) {
                                        session.attachments.retain(|a| a.id != id);
                                        cx.notify();
                                    }
                                });
                            })),
                    )
            }))
            .into_any_element()
    }
}
