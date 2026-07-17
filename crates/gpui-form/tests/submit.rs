use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_form::FormStore as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = SubmitFormStore)]
struct SubmitInput {
    name: String,
}

struct SubmitHarness {
    form: Entity<SubmitFormStore>,
}

impl SubmitHarness {
    fn new(
        input: SubmitInput,
        capture: Rc<RefCell<Option<Entity<SubmitFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| SubmitFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl Render for SubmitHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_submit_form(
    cx: &mut TestAppContext,
    input: SubmitInput,
) -> (Entity<SubmitFormStore>, WindowHandle<SubmitHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| SubmitHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn submit_sync_runs_handler_after_valid_prepare(cx: &mut TestAppContext) {
    let (form, window) = create_submit_form(
        cx,
        SubmitInput {
            name: "OpenAI".to_string(),
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("submit harness root");

    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.submit_sync(
                    move |output: SubmitInput, _window, _cx| {
                        Ok::<_, ()>(output.name.to_uppercase())
                    },
                    window,
                    cx,
                )
            })
        })
    });

    assert_eq!(result, Ok("OPENAI".to_string()));
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.meta().submission_attempts, 1);
        assert!(form.meta().is_submit_successful());
        assert!(!form.is_submitting());
        assert!(form.is_submitted());
        assert!(form.can_attempt_submit());
    });
}

#[gpui::test]
fn submit_async_sets_is_submitting_from_task(cx: &mut TestAppContext) {
    let (form, window) = create_submit_form(
        cx,
        SubmitInput {
            name: "OpenAI".to_string(),
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("submit harness root");

    let start = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.submit_async(
                    move |_output: SubmitInput, _window, cx: &mut App| {
                        Ok::<_, ()>(cx.spawn(async move |_cx| Ok::<_, ()>(())))
                    },
                    window,
                    cx,
                )
            })
        })
    });

    assert_eq!(start, Ok(()));
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(form.is_submitting());
        assert!(!form.is_submitted());
        assert!(!form.can_attempt_submit());
        assert_eq!(form.meta().submission_attempts, 1);
    });

    cx.run_until_parked();

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(!form.is_submitting());
        assert!(form.is_submitted());
        assert!(form.can_attempt_submit());
        assert!(form.meta().is_submit_successful());
    });
}

#[gpui::test]
fn submit_async_rejects_reentrant_submit(cx: &mut TestAppContext) {
    let (form, window) = create_submit_form(
        cx,
        SubmitInput {
            name: "OpenAI".to_string(),
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("submit harness root");

    let (first, second) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                let first =
                    form.submit_async(
                        move |_output: SubmitInput, _window, cx: &mut App| {
                            Ok::<_, ()>(cx.spawn(async move |_cx| {
                                std::future::pending::<Result<(), ()>>().await
                            }))
                        },
                        window,
                        cx,
                    );
                let second = form.submit_async(
                    move |_output: SubmitInput, _window, cx: &mut App| {
                        Ok::<_, ()>(cx.spawn(async move |_cx| Ok::<_, ()>(())))
                    },
                    window,
                    cx,
                );
                (first, second)
            })
        })
    });

    assert_eq!(first, Ok(()));
    assert_eq!(second, Err(gpui_form::SubmitError::Busy));
    assert!(cx.update(|_window, cx| form.read(cx).is_submitting()));
}

#[gpui::test]
fn submit_async_handler_error_does_not_set_is_submitting(cx: &mut TestAppContext) {
    let (form, window) = create_submit_form(
        cx,
        SubmitInput {
            name: "OpenAI".to_string(),
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("submit harness root");

    let start = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.submit_async(
                    move |_output: SubmitInput, _window, _cx: &mut App| {
                        Err::<gpui::Task<Result<(), ()>>, _>("invalid")
                    },
                    window,
                    cx,
                )
            })
        })
    });

    assert_eq!(start, Err(gpui_form::SubmitError::Handler("invalid")));
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(!form.is_submitting());
        assert!(form.is_submitted());
        assert!(form.can_attempt_submit());
        assert_eq!(
            form.meta().last_submit_outcome,
            Some(gpui_form::SubmitOutcome::Failure)
        );
    });
}
