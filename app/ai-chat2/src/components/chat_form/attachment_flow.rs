use super::ChatForm;
use crate::{
    components::image_preview::{self, ImagePreviewAttachment},
    foundation, state,
    state::attachments::{
        AttachmentAddResult, ComposerAttachment, ComposerAttachmentKind,
        ModelAttachmentSupportIssue, add_attachments_from_clipboard as attachments_from_clipboard,
        add_attachments_from_paths, model_support_issue,
    },
};
use fluent_bundle::FluentArgs;
use gpui::*;
use gpui_component::{
    WindowExt as _,
    notification::{Notification, NotificationType},
};
use std::path::PathBuf;
use tracing::{Level, event};

impl ChatForm {
    pub(super) fn add_attachments_from_clipboard(
        &mut self,
        item: ClipboardItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match attachments_from_clipboard(item, &mut self.next_attachment_id, cx) {
            Ok(result) => self.apply_attachment_add_result(result, window, cx),
            Err(err) => self.push_form_notification(
                "chat-form-attachment-paste-failed",
                err.to_string(),
                NotificationType::Error,
                window,
                cx,
            ),
        }
    }

    pub(super) fn add_attachments_from_current_clipboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = cx.read_from_clipboard() else {
            let message = cx
                .global::<foundation::I18n>()
                .t("chat-form-attachment-clipboard-empty");
            self.push_form_notification(
                "chat-form-attachment-paste-failed",
                message,
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        };
        if !state::attachments::clipboard_item_has_attachments(&item) {
            let message = cx
                .global::<foundation::I18n>()
                .t("chat-form-attachment-clipboard-empty");
            self.push_form_notification(
                "chat-form-attachment-paste-failed",
                message,
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }
        self.add_attachments_from_clipboard(item, window, cx);
    }

    pub(super) fn add_attachment_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match add_attachments_from_paths(paths, &mut self.next_attachment_id) {
            Ok(result) => self.apply_attachment_add_result(result, window, cx),
            Err(err) => self.push_form_notification(
                "chat-form-attachment-add-failed",
                err.to_string(),
                NotificationType::Error,
                window,
                cx,
            ),
        }
    }

    fn apply_attachment_add_result(
        &mut self,
        result: AttachmentAddResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.attachments.extend(result.attachments);
        for rejected in result.rejected {
            self.push_form_notification(
                "chat-form-attachment-add-failed",
                format!("{}: {}", rejected.label, rejected.reason),
                NotificationType::Warning,
                window,
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn remove_attachment(&mut self, local_id: u64, cx: &mut Context<Self>) {
        self.attachments
            .retain(|attachment| attachment.local_id != local_id);
        if self
            .preview_attachment
            .as_ref()
            .is_some_and(|attachment| attachment.local_id == local_id)
        {
            self.preview_attachment = None;
        }
        cx.notify();
    }

    pub(super) fn open_add_attachment_prompt(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let failed_title = cx
            .global::<foundation::I18n>()
            .t("chat-form-attachment-add-failed");
        let path_prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        let form = cx.entity().downgrade();

        window
            .spawn(cx, async move |cx| {
                let paths = match path_prompt.await {
                    Ok(Ok(Some(paths))) => paths,
                    Ok(Ok(None)) => return,
                    Ok(Err(err)) => {
                        push_chat_form_notification_async(
                            cx,
                            failed_title.into(),
                            err.to_string(),
                            NotificationType::Error,
                        );
                        return;
                    }
                    Err(err) => {
                        push_chat_form_notification_async(
                            cx,
                            failed_title.into(),
                            err.to_string(),
                            NotificationType::Error,
                        );
                        return;
                    }
                };
                if let Err(err) = form.update_in(cx, |form, window, cx| {
                    form.add_attachment_paths(paths, window, cx);
                }) {
                    event!(Level::ERROR, error = ?err, "add attachment files failed");
                }
            })
            .detach();
    }

    pub(super) fn open_attachment(
        &mut self,
        attachment: ComposerAttachment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match attachment.kind {
            ComposerAttachmentKind::Image => {
                self.open_image_preview(attachment, window, cx);
            }
            ComposerAttachmentKind::File => {
                self.open_file_preview(&attachment, window, cx);
            }
        }
    }

    fn open_image_preview(
        &mut self,
        attachment: ComposerAttachment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        image_preview::open_image_preview_dialog(
            ImagePreviewAttachment {
                path: attachment.path.clone(),
                name: attachment.name.clone(),
                width: attachment.width,
                height: attachment.height,
            },
            window,
            cx,
        );
        self.preview_attachment = Some(attachment);
    }

    fn open_file_preview(
        &mut self,
        attachment: &ComposerAttachment,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = window_ext::preview_file_with_quick_look(&attachment.path) {
            event!(Level::WARN, error = ?err, path = %attachment.path.display(), "quick look preview failed");
            cx.open_with_system(&attachment.path);
        }
    }

    fn push_form_notification(
        &self,
        title_key: &str,
        message: impl Into<SharedString>,
        notification_type: NotificationType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = cx.global::<foundation::I18n>().t(title_key);
        window.push_notification(
            Notification::new()
                .title(title)
                .message(message.into())
                .with_type(notification_type),
            cx,
        );
    }

    pub(super) fn attachment_support_issue(&self) -> Option<ModelAttachmentSupportIssue> {
        model_support_issue(&self.attachments, self.selected_model_capabilities())
    }

    pub(super) fn attachment_support_message(&self, cx: &Context<Self>) -> Option<SharedString> {
        let issue = self.attachment_support_issue()?;
        let i18n = cx.global::<foundation::I18n>();
        let message = match issue {
            ModelAttachmentSupportIssue::ImagesUnsupported => {
                i18n.t("chat-form-attachment-model-no-images")
            }
            ModelAttachmentSupportIssue::FilesUnsupported => {
                i18n.t("chat-form-attachment-model-no-files")
            }
            ModelAttachmentSupportIssue::TooManyImages { max_images } => {
                let mut args = FluentArgs::new();
                args.set("max", max_images.to_string());
                i18n.t_with_args("chat-form-attachment-model-too-many-images", &args)
            }
            ModelAttachmentSupportIssue::TooManyFiles { max_files } => {
                let mut args = FluentArgs::new();
                args.set("max", max_files.to_string());
                i18n.t_with_args("chat-form-attachment-model-too-many-files", &args)
            }
            ModelAttachmentSupportIssue::UnsupportedFileType { name } => {
                let mut args = FluentArgs::new();
                args.set("name", name);
                i18n.t_with_args("chat-form-attachment-runtime-unsupported-file", &args)
            }
        };
        Some(message.into())
    }
}

fn push_chat_form_notification(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    notification_type: NotificationType,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(notification_type),
        cx,
    );
}

fn push_chat_form_notification_async(
    cx: &mut AsyncWindowContext,
    title: SharedString,
    message: String,
    notification_type: NotificationType,
) {
    if let Err(err) = cx.window_handle().update(cx, |_, window, cx| {
        push_chat_form_notification(window, cx, title, message, notification_type);
    }) {
        event!(Level::ERROR, error = ?err, "push chat form notification failed");
    }
}
