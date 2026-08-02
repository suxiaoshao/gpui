use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use gpui::{AppContext as _, Context, Entity, Subscription, TestAppContext};
use gpui_form::{
    AsyncValidationIssue, FormEvent, FormModel, FormState as _, ValidationMessage,
    ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = TransactionForm)]
struct TransactionInput {
    #[form(required, validate(on_change))]
    title: String,
    count: u32,
}

struct EventRecorder {
    events: Arc<Mutex<Vec<FormEvent>>>,
    notifications: Arc<AtomicUsize>,
    subscriptions: Vec<Subscription>,
}

impl EventRecorder {
    fn new(form: Entity<TransactionForm>, cx: &mut Context<Self>) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let notifications = Arc::new(AtomicUsize::new(0));
        let recorded_events = events.clone();
        let recorded_notifications = notifications.clone();
        let event_subscription = cx.subscribe(&form, move |_, _, event, _| {
            recorded_events.lock().unwrap().push(event.clone());
        });
        let notify_subscription = cx.observe(&form, move |_, _, _| {
            recorded_notifications.fetch_add(1, Ordering::SeqCst);
        });
        Self {
            events,
            notifications,
            subscriptions: vec![event_subscription, notify_subscription],
        }
    }

    fn counts(&self) -> (usize, usize) {
        (
            self.events.lock().unwrap().len(),
            self.notifications.load(Ordering::SeqCst),
        )
    }
}

impl Drop for EventRecorder {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

fn fixture(cx: &mut TestAppContext) -> (Entity<TransactionForm>, Entity<EventRecorder>) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            TransactionForm::from_value(
                TransactionInput {
                    title: "initial".into(),
                    count: 1,
                },
                cx,
            )
        })
    });
    let recorder = cx.update(|cx| cx.new(|cx| EventRecorder::new(form.clone(), cx)));
    (form, recorder)
}

#[gpui::test]
fn transition_effect_publishes_exactly_once_per_changed_write(cx: &mut TestAppContext) {
    let (form, recorder) = fixture(cx);

    cx.update(|cx| {
        TransactionForm::TITLE.set(&form, String::new(), cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(recorder.read(cx).counts(), (1, 1));
        assert!(matches!(
            recorder.read(cx).events.lock().unwrap().last(),
            Some(FormEvent::ValueChanged { path, .. })
                if path == &TransactionForm::TITLE.path()
        ));
        assert_eq!(form.read(cx).revision().get(), 1);
        assert!(!TransactionForm::TITLE.errors(&form, cx).is_empty());

        TransactionForm::TITLE.set(&form, String::new(), cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(recorder.read(cx).counts(), (1, 1));
        assert_eq!(form.read(cx).revision().get(), 1);
    });
}

#[gpui::test]
fn model_replacement_absorbs_validation_effect(cx: &mut TestAppContext) {
    let (form, recorder) = fixture(cx);

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.replace(
                TransactionInput {
                    title: String::new(),
                    count: 2,
                },
                cx,
            )
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(recorder.read(cx).counts(), (1, 1));
        assert!(matches!(
            recorder.read(cx).events.lock().unwrap().last(),
            Some(FormEvent::ModelReplaced { .. })
        ));
    });
}

#[gpui::test]
fn stale_async_completion_is_a_complete_noop(cx: &mut TestAppContext) {
    let (form, recorder) = fixture(cx);

    cx.update(|cx| {
        TransactionForm::TITLE.start_async_validation(
            &form,
            "availability",
            ValidationTrigger::Change,
            |_| async {
                Err(AsyncValidationIssue::new(
                    "unavailable",
                    ValidationMessage::literal("unavailable"),
                ))
            },
            cx,
        );
        assert!(form.read(cx).is_validating());
        TransactionForm::TITLE.set(&form, "changed".into(), cx);
        assert!(!form.read(cx).is_validating());
    });

    let before = cx.update(|cx| recorder.read(cx).counts());
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(recorder.read(cx).counts(), before);
        assert!(TransactionForm::TITLE.errors(&form, cx).is_empty());
    });
}

#[gpui::test]
fn async_start_completion_and_cancel_publish_validation_events(cx: &mut TestAppContext) {
    let (form, recorder) = fixture(cx);

    cx.update(|cx| {
        TransactionForm::TITLE.start_async_validation(
            &form,
            "check",
            ValidationTrigger::Blur,
            |_| async { Ok(()) },
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        assert_eq!(recorder.read(cx).counts(), (2, 2));

        TransactionForm::TITLE.start_async_validation(
            &form,
            "pending",
            ValidationTrigger::Change,
            |_| std::future::pending(),
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|cx| {
        TransactionForm::TITLE.cancel_async_validation(&form, "pending", cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        assert_eq!(recorder.read(cx).counts(), (4, 4));
    });
}
