use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    label::Label,
};
use std::rc::Rc;

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
    let on_confirm: Rc<OnConfirm> = Rc::new(on_confirm);

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .child(Label::new(message.clone()))
            .footer({
                let on_confirm = on_confirm.clone();
                move |_dialog, _state, _window, _cx| {
                    vec![
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("confirm-delete")
                            .danger()
                            .label("Delete")
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
