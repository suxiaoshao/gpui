use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    label::Label,
};
use std::rc::Rc;

use crate::i18n::I18n;

type OnConfirm = dyn Fn(&mut Window, &mut App);

pub fn open_delete_confirm_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let title = title.into();
    let message = message.into();
    let (cancel_label, delete_label) = {
        let i18n = cx.global::<I18n>();
        (i18n.t("button-cancel"), i18n.t("button-delete"))
    };
    let on_confirm: Rc<OnConfirm> = Rc::new(on_confirm);

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .child(Label::new(message.clone()))
            .footer({
                let on_confirm = on_confirm.clone();
                let cancel_label = cancel_label.clone();
                let delete_label = delete_label.clone();
                move |_dialog, _state, _window, _cx| {
                    vec![
                        Button::new("cancel").label(cancel_label.clone()).on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        ),
                        Button::new("confirm-delete")
                            .danger()
                            .label(delete_label.clone())
                            .on_click({
                                let on_confirm = on_confirm.clone();
                                move |_, window, cx| {
                                    window.close_dialog(cx);
                                    on_confirm(window, cx);
                                }
                            }),
                    ]
                }
            })
    });
}
