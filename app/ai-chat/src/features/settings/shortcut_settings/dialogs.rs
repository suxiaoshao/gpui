use crate::{
    database::GlobalShortcutBinding,
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
    v_flex,
};
use std::rc::Rc;

pub(super) type ShortcutAction = Rc<dyn Fn(i32, &mut Window, &mut App) + 'static>;
pub(super) type DeleteShortcutAction = Rc<dyn Fn(i32, &mut Window, &mut App) -> bool + 'static>;
pub(super) type ReloadModelsAction = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

pub(super) struct ShortcutStatusActions {
    pub(super) on_reload_models: ReloadModelsAction,
    pub(super) on_reregister: ShortcutAction,
    pub(super) on_edit: ShortcutAction,
}

#[derive(Clone)]
pub(super) struct ShortcutSummary {
    pub(super) title: String,
    pub(super) subtitle: Option<String>,
    pub(super) icon: String,
    pub(super) hotkey: String,
    pub(super) input_source: String,
}

#[derive(Clone)]
pub(super) struct ShortcutStatusDetails {
    pub(super) status_label: SharedString,
    pub(super) status_message: SharedString,
    pub(super) provider_name: String,
    pub(super) model_id: String,
    pub(super) registration: SharedString,
    pub(super) input_source: SharedString,
    pub(super) runtime_state: SharedString,
    pub(super) preset_state: SharedString,
}

pub(super) fn open_delete_shortcut_dialog(
    binding: GlobalShortcutBinding,
    summary: ShortcutSummary,
    on_delete: DeleteShortcutAction,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, message, cancel_label, delete_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("dialog-delete-shortcut-title"),
            i18n.t("dialog-delete-shortcut-message"),
            i18n.t("button-cancel"),
            i18n.t("button-delete"),
        )
    };
    let binding_id = binding.id;
    window.open_dialog(cx, move |dialog, _window, cx| {
        let on_delete = on_delete.clone();
        dialog
            .title(title.clone())
            .child(
                v_flex()
                    .w(px(460.))
                    .gap_3()
                    .child(Label::new(message.clone()).text_sm())
                    .child(Divider::horizontal())
                    .child(render_shortcut_summary(&summary, cx)),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("delete-shortcut-cancel")
                            .label(cancel_label.clone())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("confirm-delete-shortcut")
                            .danger()
                            .icon(IconName::Trash)
                            .label(delete_label.clone())
                            .on_click({
                                let on_delete = on_delete.clone();
                                move |_, window, cx| {
                                    if on_delete(binding_id, window, cx) {
                                        window.close_dialog(cx);
                                    }
                                }
                            }),
                    ),
            )
    });
}

pub(super) fn open_shortcut_status_dialog(
    binding: GlobalShortcutBinding,
    summary: ShortcutSummary,
    details: ShortcutStatusDetails,
    actions: ShortcutStatusActions,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, close_label, reload_label, reregister_label, edit_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("shortcut-status-dialog-title"),
            i18n.t("button-close"),
            i18n.t("shortcut-action-reload-models"),
            i18n.t("shortcut-action-reregister"),
            i18n.t("button-edit"),
        )
    };
    let binding_id = binding.id;
    let ShortcutStatusActions {
        on_reload_models,
        on_reregister,
        on_edit,
    } = actions;
    window.open_dialog(cx, move |dialog, _window, cx| {
        let on_reload_models = on_reload_models.clone();
        let on_reregister = on_reregister.clone();
        let on_edit = on_edit.clone();
        dialog
            .w(px(640.))
            .title(title.clone())
            .child(
                v_flex()
                    .w_full()
                    .gap_4()
                    .child(render_shortcut_summary(&summary, cx))
                    .child(
                        v_flex()
                            .w_full()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .children(render_status_rows(&details, cx)),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                Button::new("shortcut-status-reload")
                                    .icon(IconName::RefreshCcw)
                                    .label(reload_label.clone())
                                    .on_click({
                                        let on_reload_models = on_reload_models.clone();
                                        move |_, window, cx| on_reload_models(window, cx)
                                    }),
                            )
                            .child(
                                Button::new("shortcut-status-reregister")
                                    .icon(IconName::RefreshCcw)
                                    .label(reregister_label.clone())
                                    .on_click({
                                        let on_reregister = on_reregister.clone();
                                        move |_, window, cx| on_reregister(binding_id, window, cx)
                                    }),
                            ),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("shortcut-status-close")
                            .label(close_label.clone())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("shortcut-status-edit")
                            .primary()
                            .icon(IconName::Edit)
                            .label(edit_label.clone())
                            .on_click({
                                let on_edit = on_edit.clone();
                                move |_, window, cx| {
                                    window.close_dialog(cx);
                                    on_edit(binding_id, window, cx);
                                }
                            }),
                    ),
            )
    });
}

fn render_shortcut_summary(summary: &ShortcutSummary, cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .py_2()
        .child(
            div()
                .flex()
                .size_9()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(cx.theme().border.opacity(0.35))
                .child(Label::new(summary.icon.clone()).text_base()),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    Label::new(summary.title.clone())
                        .text_sm()
                        .font_medium()
                        .truncate(),
                )
                .when_some(summary.subtitle.clone(), |this, subtitle| {
                    this.child(
                        Label::new(subtitle)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    )
                })
                .child(
                    Label::new(format!("{} · {}", summary.hotkey, summary.input_source))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate(),
                ),
        )
        .into_any_element()
}

fn render_status_rows(details: &ShortcutStatusDetails, cx: &mut App) -> Vec<AnyElement> {
    vec![
        status_row(
            cx.global::<I18n>().t("field-status"),
            details.status_label.clone(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("shortcut-status-message"),
            details.status_message.clone(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("field-provider"),
            details.provider_name.clone().into(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("field-model"),
            details.model_id.clone().into(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("shortcut-status-registration"),
            details.registration.clone(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("field-send-content"),
            details.input_source.clone(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("shortcut-status-runtime"),
            details.runtime_state.clone(),
            cx,
        ),
        status_row(
            cx.global::<I18n>().t("shortcut-preset-settings"),
            details.preset_state.clone(),
            cx,
        ),
    ]
}

fn status_row(label: String, value: SharedString, cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Label::new(label)
                .text_xs()
                .flex_none()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Label::new(value).text_sm().truncate()),
        )
        .into_any_element()
}
