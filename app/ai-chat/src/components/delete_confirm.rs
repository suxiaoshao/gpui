use gpui::*;
use gpui_component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    label::Label,
};
use std::rc::Rc;

use crate::i18n::I18n;

type OnConfirm = dyn Fn(&mut Window, &mut App);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DestructiveAction {
    Delete,
    Clear,
    Regenerate,
}

impl DestructiveAction {
    fn confirm_label_key(self) -> &'static str {
        match self {
            Self::Delete => "button-delete",
            Self::Clear => "button-clear",
            Self::Regenerate => "button-regenerate",
        }
    }
}

pub fn open_delete_confirm_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        on_confirm,
        window,
        cx,
    );
}

pub(crate) fn open_destructive_confirm_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    action: DestructiveAction,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let title = title.into();
    let message = message.into();
    let (cancel_label, confirm_label) = {
        let i18n = cx.global::<I18n>();
        (i18n.t("button-cancel"), i18n.t(action.confirm_label_key()))
    };
    let on_confirm: Rc<OnConfirm> = Rc::new(on_confirm);

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .child(Label::new(message.clone()))
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(Button::new("cancel").label(cancel_label.clone())),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("confirm-delete")
                                .danger()
                                .label(confirm_label.clone())
                                .on_click({
                                    let on_confirm = on_confirm.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        on_confirm(window, cx);
                                    }
                                }),
                        ),
                    ),
            )
    });
}

#[cfg(test)]
mod tests {
    use super::DestructiveAction;

    #[test]
    fn destructive_actions_use_specific_confirm_button_labels() {
        assert_eq!(
            DestructiveAction::Delete.confirm_label_key(),
            "button-delete"
        );
        assert_eq!(DestructiveAction::Clear.confirm_label_key(), "button-clear");
        assert_eq!(
            DestructiveAction::Regenerate.confirm_label_key(),
            "button-regenerate"
        );
    }
}
