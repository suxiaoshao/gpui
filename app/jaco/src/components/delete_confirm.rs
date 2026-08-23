use crate::foundation::I18n;
use gpui::*;
use gpui_component::{
    Disableable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    label::Label,
};
use std::rc::Rc;

type OnConfirm = dyn Fn(&mut Window, &mut App);
type AsyncOnConfirm = dyn Fn(&mut Window, &mut App) -> Task<bool>;

struct AsyncDestructiveConfirmState {
    cancel_label: SharedString,
    confirm_label: SharedString,
    on_confirm: Rc<AsyncOnConfirm>,
    task: Option<Task<()>>,
}

impl AsyncDestructiveConfirmState {
    fn confirm(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.task.is_some() {
            return;
        }

        let action = (self.on_confirm)(window, cx);
        let state = cx.entity().downgrade();
        self.task = Some(window.spawn(cx, async move |cx| {
            let should_close = action.await;
            let _ = state.update_in(cx, |state, window, cx| {
                state.task = None;
                if should_close {
                    window.close_dialog(cx);
                } else {
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn cancel(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.task.is_none() {
            window.close_dialog(cx);
        }
    }

    fn is_pending(&self) -> bool {
        self.task.is_some()
    }
}

impl Render for AsyncDestructiveConfirmState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = self.is_pending();
        DialogFooter::new()
            .child(
                Button::new("cancel")
                    .label(self.cancel_label.clone())
                    .disabled(pending)
                    .on_click(cx.listener(Self::cancel)),
            )
            .child(
                Button::new("confirm-destructive-action")
                    .danger()
                    .label(self.confirm_label.clone())
                    .loading(pending)
                    .on_click(cx.listener(Self::confirm)),
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DestructiveAction {
    Archive,
    Delete,
}

impl DestructiveAction {
    fn confirm_label_key(self) -> &'static str {
        match self {
            Self::Archive => "button-archive",
            Self::Delete => "button-delete",
        }
    }
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
                            Button::new("confirm-destructive-action")
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

pub(crate) fn open_async_destructive_confirm_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    action: DestructiveAction,
    on_confirm: impl Fn(&mut Window, &mut App) -> Task<bool> + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let title = title.into();
    let message = message.into();
    let (cancel_label, confirm_label) = {
        let i18n = cx.global::<I18n>();
        (i18n.t("button-cancel"), i18n.t(action.confirm_label_key()))
    };
    let state = cx.new(|_| AsyncDestructiveConfirmState {
        cancel_label: cancel_label.into(),
        confirm_label: confirm_label.into(),
        on_confirm: Rc::new(on_confirm),
        task: None,
    });

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let cancel_state = state.clone();
        let confirm_state = state.clone();
        dialog
            .title(title.clone())
            .close_button(false)
            .on_cancel(move |_, _, cx| !cancel_state.read(cx).is_pending())
            .on_ok(move |event, window, cx| {
                confirm_state.update(cx, |state, cx| state.confirm(event, window, cx));
                false
            })
            .child(Label::new(message.clone()))
            .footer(state.clone())
    });
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use gpui::{
        AppContext as _, ClickEvent, IntoElement, Render, TestAppContext, Window, WindowHandle, div,
    };
    #[cfg(not(target_os = "macos"))]
    use gpui::{ParentElement as _, Task, VisualTestContext};
    #[cfg(not(target_os = "macos"))]
    use gpui_component::{Root, WindowExt, dialog::ConfirmDialog};
    use tokio::sync::oneshot;

    #[cfg(not(target_os = "macos"))]
    use super::open_async_destructive_confirm_dialog;
    use super::{AsyncDestructiveConfirmState, DestructiveAction};

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
        }
    }

    #[cfg(not(target_os = "macos"))]
    struct DialogTestView;

    #[cfg(not(target_os = "macos"))]
    impl Render for DialogTestView {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div().children(Root::render_dialog_layer(window, cx))
        }
    }

    #[test]
    fn destructive_delete_uses_delete_button_label() {
        assert_eq!(
            DestructiveAction::Delete.confirm_label_key(),
            "button-delete"
        );
    }

    #[test]
    fn destructive_archive_uses_archive_button_label() {
        assert_eq!(
            DestructiveAction::Archive.confirm_label_key(),
            "button-archive"
        );
    }

    #[gpui::test]
    fn async_destructive_confirmation_owns_task_and_blocks_repeated_submit(
        cx: &mut TestAppContext,
    ) {
        let window = open_test_window(cx);
        let invocations = Rc::new(Cell::new(0));
        let (sender, receiver) = oneshot::channel();
        let receiver = Rc::new(RefCell::new(Some(receiver)));
        let state = window
            .update(cx, |_view, _window, cx| {
                let invocations = invocations.clone();
                let receiver = receiver.clone();
                cx.new(|_| AsyncDestructiveConfirmState {
                    cancel_label: "Cancel".into(),
                    confirm_label: "Archive".into(),
                    on_confirm: Rc::new(move |window, cx| {
                        invocations.set(invocations.get() + 1);
                        let receiver = receiver
                            .borrow_mut()
                            .take()
                            .expect("confirmation starts once");
                        window.spawn(cx, async move |_| receiver.await.unwrap_or(false))
                    }),
                    task: None,
                })
            })
            .expect("create async confirmation state");

        window
            .update(cx, |_view, window, cx| {
                state.update(cx, |state, cx| {
                    state.confirm(&ClickEvent::default(), window, cx)
                });
                state.update(cx, |state, cx| {
                    state.confirm(&ClickEvent::default(), window, cx)
                });
                state.update(cx, |state, cx| {
                    state.cancel(&ClickEvent::default(), window, cx)
                });
                assert!(state.read(cx).is_pending());
            })
            .expect("start async confirmation");
        assert_eq!(invocations.get(), 1);

        sender.send(false).expect("finish confirmation");
        cx.run_until_parked();
        window
            .update(cx, |_view, _window, cx| {
                assert!(!state.read(cx).is_pending());
            })
            .expect("inspect completed confirmation");
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui::test]
    fn async_destructive_confirmation_closes_only_after_success(cx: &mut TestAppContext) {
        let window = open_dialog_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            open_async_destructive_confirm_dialog(
                "Delete",
                "Delete this item?",
                DestructiveAction::Delete,
                |_, _| Task::ready(true),
                window,
                cx,
            );
        });
        cx.run_until_parked();

        cx.update(|window, cx| {
            window.dispatch_action(Box::new(ConfirmDialog), cx);
            assert!(window.has_active_dialog(cx));
        });
        cx.run_until_parked();
        cx.update(|window, cx| assert!(!window.has_active_dialog(cx)));
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_window, cx| cx.new(|_| TestView))
                .expect("open destructive confirmation test window")
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn open_dialog_test_window(cx: &mut TestAppContext) -> WindowHandle<Root> {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::foundation::init_i18n(cx);
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| DialogTestView);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("open destructive confirmation test window")
        })
    }
}
