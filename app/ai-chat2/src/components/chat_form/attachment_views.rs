use super::{
    COMPOSER_BUTTON_ICON_SIZE, COMPOSER_BUTTON_RADIUS, COMPOSER_BUTTON_SIZE, ChatForm, attachments,
};
use crate::{
    foundation,
    foundation::assets::IconName,
    state::attachments::{ComposerAttachment, ComposerAttachmentKind},
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    popover::Popover,
    v_flex,
};

impl ChatForm {
    pub(super) fn render_add_attachment_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<foundation::I18n>();
        let tooltip = i18n.t("chat-form-add-tooltip");
        let add_files = i18n.t("chat-form-attachment-add-files");
        let add_from_clipboard = i18n.t("chat-form-attachment-add-from-clipboard");
        let trigger = Button::new("chat-form-add")
            .ghost()
            .selected(self.attachment_menu_open)
            .with_size(px(COMPOSER_BUTTON_SIZE))
            .size(px(COMPOSER_BUTTON_SIZE))
            .p(px(0.))
            .rounded(px(COMPOSER_BUTTON_RADIUS))
            .child(Icon::new(IconName::Plus).with_size(px(COMPOSER_BUTTON_ICON_SIZE)))
            .tooltip(tooltip);

        Popover::new("chat-form-attachment-menu")
            .anchor(Anchor::TopLeft)
            .appearance(false)
            .open(self.attachment_menu_open)
            .on_open_change(cx.listener(|form, open: &bool, _window, cx| {
                form.set_attachment_menu_open(*open, cx);
            }))
            .trigger(trigger)
            .child(
                v_flex()
                    .w(px(220.))
                    .occlude()
                    .mb_1p5()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .p_1()
                    .gap_0p5()
                    .child(
                        Button::new("chat-form-attachment-add-files-action")
                            .ghost()
                            .small()
                            .w_full()
                            .icon(IconName::Paperclip)
                            .label(add_files)
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.open_add_attachment_prompt(window, cx);
                            })),
                    )
                    .child(
                        Button::new("chat-form-attachment-add-clipboard-action")
                            .ghost()
                            .small()
                            .w_full()
                            .icon(IconName::Clipboard)
                            .label(add_from_clipboard)
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.add_attachments_from_current_clipboard(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_attachments_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let cards = self
            .attachments
            .iter()
            .cloned()
            .map(|attachment| self.render_attachment_card(attachment, cx))
            .collect::<Vec<_>>();

        div()
            .id("chat-form-attachments-strip")
            .w_full()
            .overflow_x_scroll()
            .child(
                h_flex()
                    .items_end()
                    .gap(px(attachments::STRIP_GAP))
                    .children(cards),
            )
            .into_any_element()
    }

    fn render_attachment_card(
        &self,
        attachment: ComposerAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match attachment.kind {
            ComposerAttachmentKind::Image => self.render_image_attachment_card(attachment, cx),
            ComposerAttachmentKind::File => self.render_file_attachment_card(attachment, cx),
        }
    }

    fn render_image_attachment_card(
        &self,
        attachment: ComposerAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let remove_tooltip = cx
            .global::<foundation::I18n>()
            .t("chat-form-attachment-remove");
        let local_id = attachment.local_id;
        div()
            .id(format!("chat-form-attachment-image-{local_id}"))
            .relative()
            .flex_none()
            .size(px(attachments::IMAGE_THUMBNAIL_SIZE))
            .rounded(px(attachments::CARD_RADIUS))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.28))
            .cursor(CursorStyle::PointingHand)
            .hover(|this| this.border_color(cx.theme().primary.opacity(0.55)))
            .on_click({
                let attachment = attachment.clone();
                cx.listener(move |form, _, window, cx| {
                    form.open_attachment(attachment.clone(), window, cx);
                })
            })
            .child(
                div()
                    .absolute()
                    .top(px(1.))
                    .right(px(1.))
                    .bottom(px(1.))
                    .left(px(1.))
                    .rounded(px(attachments::CARD_RADIUS - 1.))
                    .overflow_hidden()
                    .child(
                        img(attachment.path.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    ),
            )
            .child(self.render_remove_attachment_button(
                local_id,
                remove_tooltip,
                "chat-form-remove-image",
                cx,
            ))
            .into_any_element()
    }

    fn render_file_attachment_card(
        &self,
        attachment: ComposerAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let remove_tooltip = cx
            .global::<foundation::I18n>()
            .t("chat-form-attachment-remove");
        let local_id = attachment.local_id;
        h_flex()
            .id(format!("chat-form-attachment-file-{local_id}"))
            .relative()
            .flex_none()
            .w(px(attachments::FILE_CARD_WIDTH))
            .h(px(attachments::FILE_CARD_HEIGHT))
            .gap_2()
            .p_2()
            .rounded(px(attachments::CARD_RADIUS))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.22))
            .cursor(CursorStyle::PointingHand)
            .hover(|this| this.border_color(cx.theme().primary.opacity(0.55)))
            .on_click({
                let attachment = attachment.clone();
                cx.listener(move |form, _, window, cx| {
                    form.open_attachment(attachment.clone(), window, cx);
                })
            })
            .child(
                div()
                    .flex_none()
                    .size(px(32.))
                    .rounded(px(6.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::File)
                            .with_size(px(18.))
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .flex_1()
                    .gap(px(2.))
                    .child(Label::new(attachment.name.clone()).text_sm().truncate())
                    .child(
                        Label::new(attachments::format_file_size(attachment.size_bytes))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .child(self.render_remove_attachment_button(
                local_id,
                remove_tooltip,
                "chat-form-remove-file",
                cx,
            ))
            .into_any_element()
    }

    fn render_remove_attachment_button(
        &self,
        local_id: u64,
        tooltip: String,
        id_prefix: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Button::new(format!("{id_prefix}-{local_id}"))
            .ghost()
            .absolute()
            .top(px(4.))
            .right(px(4.))
            .with_size(px(attachments::REMOVE_BUTTON_SIZE))
            .size(px(attachments::REMOVE_BUTTON_SIZE))
            .p_0()
            .rounded(px(999.))
            .bg(cx.theme().background.opacity(0.86))
            .child(Icon::new(IconName::X).with_size(px(14.)))
            .tooltip(tooltip)
            .on_click(cx.listener(move |form, _, _, cx| {
                cx.stop_propagation();
                form.remove_attachment(local_id, cx);
            }))
            .into_any_element()
    }

    pub(super) fn render_attachment_support_message(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.attachment_support_message(cx).map(|message| {
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(
                    Icon::new(IconName::CircleAlert)
                        .size_4()
                        .flex_none()
                        .text_color(cx.theme().danger),
                )
                .child(
                    Label::new(message)
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .truncate(),
                )
                .into_any_element()
        })
    }
}
