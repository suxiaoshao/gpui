use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{AsyncValidationIssue, Form, FormEvent, FormSchema, ValidationMessage};

struct ManualState<T> {
    output: Option<T>,
    waker: Option<Waker>,
}

struct ManualFuture<T> {
    state: Rc<RefCell<ManualState<T>>>,
}

struct ManualCompletion<T> {
    state: Rc<RefCell<ManualState<T>>>,
}

fn manual<T>() -> (ManualCompletion<T>, ManualFuture<T>) {
    let state = Rc::new(RefCell::new(ManualState {
        output: None,
        waker: None,
    }));
    (
        ManualCompletion {
            state: state.clone(),
        },
        ManualFuture { state },
    )
}

impl<T> ManualCompletion<T> {
    fn complete(self, output: T) {
        let waker = {
            let mut state = self.state.borrow_mut();
            state.output = Some(output);
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Future for ManualFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        match state.output.take() {
            Some(output) => Poll::Ready(output),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Pair {
    left: String,
    right: String,
}

fn issue(code: &'static str) -> Result<(), AsyncValidationIssue> {
    Err(AsyncValidationIssue::new(
        code,
        ValidationMessage::literal(code),
    ))
}

fn counters<M: FormSchema>(
    form: &gpui::Entity<Form<M>>,
    cx: &mut gpui::App,
) -> (Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let events = Rc::new(Cell::new(0));
    let event_count = events.clone();
    cx.subscribe(form, move |_, _: &FormEvent<M>, _| {
        event_count.set(event_count.get() + 1);
    })
    .detach();
    let notifications = Rc::new(Cell::new(0));
    let notification_count = notifications.clone();
    cx.observe(form, move |_, _| {
        notification_count.set(notification_count.get() + 1);
    })
    .detach();
    (events, notifications)
}

#[gpui::test]
fn async_completion_requires_a_full_current_version_match(cx: &mut TestAppContext) {
    let (form, events, notifications) = cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(Pair {
                left: "left".into(),
                right: "right".into(),
            })
        });
        let (events, notifications) = counters(&form, cx);
        (form, events, notifications)
    });
    let (completion, future) = manual();
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.start_async_validation(Pair::LEFT, "remote", |_| future, cx)
                .unwrap();
        });
    });
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);

    completion.complete(issue("unavailable"));
    cx.run_until_parked();

    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        let errors = Pair::LEFT.errors(&form, cx);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "unavailable");
    });
    assert_eq!(events.get(), 1);
    assert_eq!(notifications.get(), 1);
}

#[gpui::test]
fn stale_version_completion_has_no_model_report_or_publication_side_effects(
    cx: &mut TestAppContext,
) {
    let (form, events, notifications) = cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(Pair {
                left: "left".into(),
                right: "right".into(),
            })
        });
        let (events, notifications) = counters(&form, cx);
        (form, events, notifications)
    });
    let (completion, future) = manual();
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.start_async_validation(Pair::LEFT, "remote", |_| future, cx)
                .unwrap();
        });
    });
    cx.run_until_parked();
    cx.update(|cx| assert!(Pair::RIGHT.set(&form, "new revision".into(), cx)));
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);
    let before = cx.update(|cx| {
        (
            Pair::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            Pair::ROOT.then(Pair::LEFT).key(form.read(cx)),
        )
    });

    completion.complete(issue("stale"));
    cx.run_until_parked();

    let after = cx.update(|cx| {
        (
            Pair::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            Pair::ROOT.then(Pair::LEFT).key(form.read(cx)),
        )
    });
    assert_eq!(after, before);
    cx.update(|cx| assert!(Pair::LEFT.errors(&form, cx).is_empty()));
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);
}

#[gpui::test]
fn old_generation_completion_cannot_replace_the_new_pending_request(cx: &mut TestAppContext) {
    let (form, events, notifications) = cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(Pair {
                left: "left".into(),
                right: "right".into(),
            })
        });
        let (events, notifications) = counters(&form, cx);
        (form, events, notifications)
    });
    let (old_completion, old_future) = manual();
    let (current_completion, current_future) = manual();
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.start_async_validation(Pair::LEFT, "old", |_| old_future, cx)
                .unwrap();
            form.start_async_validation(Pair::LEFT, "current", |_| current_future, cx)
                .unwrap();
        });
    });
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);

    old_completion.complete(issue("old"));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(form.read(cx).is_validating());
        assert!(Pair::LEFT.errors(&form, cx).is_empty());
    });
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);

    current_completion.complete(issue("current"));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        let errors = Pair::LEFT.errors(&form, cx);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "current");
    });
    assert_eq!(events.get(), 1);
    assert_eq!(notifications.get(), 1);
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Row {
    value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Rows {
    #[form(items)]
    rows: Vec<Row>,
}

#[gpui::test]
fn retired_occurrence_completion_has_no_model_report_or_publication_side_effects(
    cx: &mut TestAppContext,
) {
    let (form, events, notifications) = cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(Rows {
                rows: vec![Row {
                    value: "row".into(),
                }],
            })
        });
        let (events, notifications) = counters(&form, cx);
        (form, events, notifications)
    });
    let rows = Rows::ROOT.then(Rows::ROWS);
    let item = cx.update(|cx| rows.items(&form, cx).remove(0));
    let value = item.clone().then(Row::VALUE);
    let (completion, future) = manual();
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.start_dynamic_async_validation(value, "remote", |_| future, cx)
                .unwrap();
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        rows.remove(&form, item, cx).unwrap();
    });
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);
    let before = cx.update(|cx| {
        (
            Rows::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx).len(),
        )
    });

    completion.complete(issue("retired"));
    cx.run_until_parked();

    let after = cx.update(|cx| {
        (
            Rows::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx).len(),
        )
    });
    assert_eq!(after, before);
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);
}
