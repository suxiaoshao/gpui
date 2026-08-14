use std::{cell::Cell, rc::Rc};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{Form, FormEvent, FormSchema};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Draft {
    value: String,
}

fn draft(value: &str) -> Draft {
    Draft {
        value: value.into(),
    }
}

#[gpui::test]
fn prepared_map_preserves_its_session_bound_version(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::new(draft("initial")));
        let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        let version = prepared.version();
        let mapped = prepared.map(|draft| draft.value);

        assert_eq!(mapped.version(), version);
        assert_eq!(mapped.value(), "initial");
        let (parts_version, value) = mapped.into_parts();
        assert_eq!(parts_version, version);
        assert_eq!(value, "initial");
    });
}

#[gpui::test]
fn rebase_if_current_rejects_stale_and_cross_session_versions_without_mutation(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(Cell::new(0));
    let notifications = Rc::new(Cell::new(0));
    let (first, second, current, foreign) = cx.update(|cx| {
        let first = cx.new(|_| Form::new(draft("first")));
        let second = cx.new(|_| Form::new(draft("second")));
        let event_count = events.clone();
        cx.subscribe(&first, move |_, _: &FormEvent<Draft>, _| {
            event_count.set(event_count.get() + 1);
        })
        .detach();
        let notification_count = notifications.clone();
        cx.observe(&first, move |_, _| {
            notification_count.set(notification_count.get() + 1);
        })
        .detach();

        let current = first.update(cx, |form, cx| form.prepare(cx)).unwrap();
        let foreign = second.update(cx, |form, cx| form.prepare(cx)).unwrap();
        (first, second, current, foreign)
    });
    let mapped = current.clone().map(|value| value.value);
    cx.update(|cx| {
        assert!(first.update(cx, |form, cx| {
            form.rebase_if_current(mapped.version(), draft("saved"), cx)
        }));
        assert_eq!(Draft::VALUE.get(&first, cx), "saved");
    });
    cx.run_until_parked();

    cx.update(|cx| {
        first
            .update(cx, |form, cx| {
                form.start_async_validation(Draft::VALUE, "pending", |_| std::future::pending(), cx)
            })
            .unwrap();
    });
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);

    let before = cx.update(|cx| {
        let path = Draft::ROOT.then(Draft::VALUE);
        (
            Draft::ROOT.get(&first, cx),
            first.read(cx).revision(),
            first.read(cx).is_dirty(),
            first.read(cx).validation_report(),
            first.read(cx).is_validating(),
            first.read(cx).first_error_path(),
            path.key(first.read(cx)),
        )
    });

    cx.update(|cx| {
        let stale = current.version();
        assert!(!first.update(cx, |form, cx| {
            form.rebase_if_current(stale, draft("stale"), cx)
        }));
        assert!(!first.update(cx, |form, cx| {
            form.rebase_if_current(foreign.version(), draft("foreign"), cx)
        }));
    });
    cx.run_until_parked();

    let after = cx.update(|cx| {
        let path = Draft::ROOT.then(Draft::VALUE);
        (
            Draft::ROOT.get(&first, cx),
            first.read(cx).revision(),
            first.read(cx).is_dirty(),
            first.read(cx).validation_report(),
            first.read(cx).is_validating(),
            first.read(cx).first_error_path(),
            path.key(first.read(cx)),
        )
    });
    assert_eq!(after, before);
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);

    drop(second);
}
