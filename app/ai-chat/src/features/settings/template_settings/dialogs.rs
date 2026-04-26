use crate::{
    components::message::render_role_pill,
    database::ConversationTemplate,
    features::settings::template_settings::{
        TEMPLATE_DIALOG_MARGIN_TOP, TEMPLATE_DIALOG_MAX_HEIGHT, TEMPLATE_DIALOG_WIDTH,
    },
    foundation::{assets::IconName, i18n::I18n},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    divider::Divider,
    h_flex,
    label::Label,
    text::TextView,
    v_flex,
};
use std::rc::Rc;

pub(super) type TemplateAction = Rc<dyn Fn(i32, &mut Window, &mut App) + 'static>;
pub(super) type DeleteTemplateAction = Rc<dyn Fn(i32, &mut Window, &mut App) -> bool + 'static>;

pub(super) fn open_template_view_dialog(
    template: ConversationTemplate,
    on_edit: TemplateAction,
    on_delete: TemplateAction,
    window: &mut Window,
    cx: &mut App,
) {
    let (dialog_title, edit_label, delete_label, id_label, prompts_label): (
        String,
        SharedString,
        SharedString,
        SharedString,
        String,
    ) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("dialog-view-template-title"),
            i18n.t("button-edit").into(),
            i18n.t("button-delete").into(),
            i18n.t("field-id").into(),
            i18n.t("field-prompts"),
        )
    };
    window.open_dialog(cx, move |dialog, _window, cx| {
        let edit_action = on_edit.clone();
        let delete_action = on_delete.clone();
        dialog
            .w(px(TEMPLATE_DIALOG_WIDTH))
            .max_h(px(TEMPLATE_DIALOG_MAX_HEIGHT))
            .margin_top(px(TEMPLATE_DIALOG_MARGIN_TOP))
            .title(dialog_title.clone())
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_4()
                    .child(render_template_dialog_header(
                        &template,
                        id_label.clone(),
                        edit_label.clone(),
                        delete_label.clone(),
                        edit_action,
                        delete_action,
                        cx,
                    ))
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .gap_3()
                            .child(Label::new(prompts_label.clone()).text_sm().font_medium())
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .children(render_prompt_blocks(&template, cx)),
                            ),
                    ),
            )
    });
}

pub(super) fn open_delete_template_dialog(
    template: ConversationTemplate,
    on_delete: DeleteTemplateAction,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, message, cancel_label, delete_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("dialog-delete-template-title"),
            i18n.t("dialog-delete-template-message"),
            i18n.t("button-cancel"),
            i18n.t("button-delete-template"),
        )
    };
    let template_id = template.id;

    window.open_dialog(cx, move |dialog, _window, cx| {
        let on_delete = on_delete.clone();
        dialog
            .title(title.clone())
            .child(
                v_flex()
                    .w(px(420.))
                    .gap_3()
                    .child(Label::new(message.clone()).text_sm())
                    .child(Divider::horizontal())
                    .child(render_template_delete_summary(&template, cx)),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("delete-template-cancel")
                            .label(cancel_label.clone())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("confirm-delete-template")
                            .danger()
                            .label(delete_label.clone())
                            .on_click({
                                let on_delete = on_delete.clone();
                                move |_, window, cx| {
                                    if on_delete(template_id, window, cx) {
                                        window.close_dialog(cx);
                                    }
                                }
                            }),
                    ),
            )
    });
}

fn render_template_dialog_header(
    template: &ConversationTemplate,
    id_label: SharedString,
    edit_label: SharedString,
    delete_label: SharedString,
    on_edit: TemplateAction,
    on_delete: TemplateAction,
    cx: &mut App,
) -> AnyElement {
    let template_id = template.id;
    h_flex()
        .w_full()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            h_flex()
                .items_start()
                .gap_3()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .size_10()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(cx.theme().accent.opacity(0.65))
                        .child(Label::new(template.icon.clone()).text_lg()),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .min_w_0()
                        .child(Label::new(template.name.clone()).text_lg().truncate())
                        .when_some(template.description.clone(), |this, description| {
                            this.child(
                                Label::new(description)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate(),
                            )
                        })
                        .child(
                            Label::new(format!("{id_label} {}", template.id))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
        )
        .child(
            h_flex()
                .flex_none()
                .gap_1()
                .child(
                    Button::new("view-template-edit")
                        .icon(IconName::Edit)
                        .ghost()
                        .tooltip(edit_label)
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            on_edit(template_id, window, cx);
                        }),
                )
                .child(
                    Button::new("view-template-delete")
                        .icon(IconName::Trash)
                        .danger()
                        .tooltip(delete_label)
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            on_delete(template_id, window, cx);
                        }),
                ),
        )
        .into_any_element()
}

fn render_prompt_blocks(template: &ConversationTemplate, cx: &mut App) -> Vec<AnyElement> {
    let mut elements = Vec::with_capacity(template.prompts.len().saturating_mul(2));
    for (index, prompt) in template.prompts.iter().enumerate() {
        if index > 0 {
            elements.push(Divider::horizontal().my_3().into_any_element());
        }
        elements.push(render_prompt_block(template.id, index, prompt, cx));
    }
    elements
}

fn render_prompt_block(
    template_id: i32,
    index: usize,
    prompt: &crate::database::ConversationTemplatePrompt,
    cx: &mut App,
) -> AnyElement {
    let text_id: SharedString = format!("settings-template-prompt-{template_id}-{index}").into();
    v_flex()
        .w_full()
        .min_w_0()
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(render_role_pill(prompt.role, cx)),
        )
        .child(
            TextView::markdown(text_id, &prompt.prompt)
                .selectable(true)
                .overflow_x_hidden(),
        )
        .into_any_element()
}

fn render_template_delete_summary(template: &ConversationTemplate, cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .py_2()
        .child(
            div()
                .flex()
                .size_8()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(cx.theme().radius)
                .bg(cx.theme().border.opacity(0.35))
                .child(Label::new(template.icon.clone()).text_base()),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(Label::new(template.name.clone()).text_sm().truncate())
                .when_some(template.description.clone(), |this, description| {
                    this.child(
                        Label::new(description)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    )
                }),
        )
        .into_any_element()
}
