use super::*;
use crate::foundation::attachments::{self, Attachment, Content};
use gpui_kit::component::{
    Icon,
    attachment::{
        Attachment as AttachmentView, AttachmentActions, AttachmentContent, AttachmentDescription,
        AttachmentGroup, AttachmentMedia, AttachmentTitle,
    },
};

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
            let task = cx.spawn(async move |state, cx| {
                let result = future.await;
                let _ = state.update(cx, |state, cx| {
                    if let Some(session) = state.sessions.get_mut(&target) {
                        session.attachments_read = None;
                        match result {
                            Ok(items) => session.attachments.extend(items),
                            Err(error) => session.error = Some(error),
                        }
                        crate::state::conversation::notify_session(&target, cx);
                    }
                });
            });
            if let Some(session) = state.sessions.get_mut(&key) {
                session.attachments_read = Some(task);
            }
            crate::state::conversation::notify_session(&key, cx);
        });
    }
    pub(super) fn render_attachments(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let session = self.state.read(cx).current()?;
        if session.attachments.is_empty() {
            return None;
        }
        let key = self.shown_key.clone().unwrap_or_default();
        let editable = session.can_edit_draft()
            && self
                .views
                .get(&key)
                .and_then(|view| view.preview.as_deref())
                .is_none_or(|id| session.history().on_current_path(id));
        Some(
            AttachmentGroup::new(SharedString::from(format!("composer-attachments-{key}")))
                .children(session.attachments.iter().map(|attachment| {
                    let target = key.clone();
                    let id = attachment.id.clone();
                    let content = attachment.content.clone();
                    let name = attachment.name.clone();
                    let media = match &content {
                        Content::File { .. } => {
                            AttachmentMedia::new().child(Icon::new(IconName::FileText))
                        }
                        Content::Image { image } => AttachmentMedia::new().src(image.path()),
                    };
                    let format = attachment
                        .format_name()
                        .unwrap_or_else(|| t(cx, "attachment-file"));
                    let description =
                        format!("{format} · {}", format_file_size(attachment.byte_len()));
                    let host = self.image_preview.clone();
                    let button_host = host.clone();
                    let button_content = content.clone();
                    let button_name = name.clone();
                    let card = AttachmentView::new()
                        .id(SharedString::from(format!("attachment-{id}")))
                        .small()
                        .media(media)
                        .content(
                            AttachmentContent::new()
                                .child(
                                    // The card's pointer preview has a keyboard-accessible
                                    // counterpart; Attachment's whole-card layer is pointer-only.
                                    Button::new(SharedString::from(format!("preview-{id}")))
                                        .ghost()
                                        .xsmall()
                                        .self_start()
                                        .px_0()
                                        .min_w_0()
                                        .max_w_full()
                                        .accessibility_label(name.clone())
                                        .tooltip(name.clone())
                                        .child(AttachmentTitle::new(name.clone()))
                                        .on_click(move |_, window, cx| {
                                            show_preview(
                                                &button_host,
                                                &button_content,
                                                &button_name,
                                                window,
                                                cx,
                                            )
                                        }),
                                )
                                .description(AttachmentDescription::new(description)),
                        )
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            show_preview(&host, &content, &name, window, cx);
                        });
                    card.actions(
                        AttachmentActions::new().child(
                            Button::new(SharedString::from(format!("remove-{id}")))
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .disabled(!editable)
                                .tooltip(t(cx, "attachment-remove"))
                                .accessibility_label(t(cx, "attachment-remove"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.state.update(cx, |state, cx| {
                                        if let Some(session) = state
                                            .sessions
                                            .get_mut(&target)
                                            .filter(|s| s.can_edit_draft())
                                        {
                                            session.attachments.retain(|a| a.id != id);
                                            crate::state::conversation::notify_session(&target, cx);
                                        }
                                    });
                                })),
                        ),
                    )
                }))
                .into_any_element(),
        )
    }
}

fn show_preview(
    host: &Entity<super::image_preview::PreviewHost>,
    content: &Content,
    name: &str,
    window: &mut Window,
    cx: &mut App,
) {
    match content {
        Content::File { path, .. } => cx.open_with_system(path),
        Content::Image { image } => {
            super::image_preview::open(host, image.clone(), name.to_owned(), window, cx);
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1_000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let mut unit = 0;
    while value >= 1_000. && unit < units.len() - 1 {
        value /= 1_000.;
        unit += 1;
    }
    let number = format!("{value:.1}");
    format!("{} {}", number.trim_end_matches(".0"), units[unit])
}
