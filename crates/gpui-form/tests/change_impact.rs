use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{Form, FormEvent, FormSchema, ModelChange, ModelChangeKind, ResolveError};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Row {
    value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Details {
    note: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Model {
    title: String,
    #[form(child)]
    details: Details,
    #[form(items)]
    rows: Vec<Row>,
}

fn model() -> Model {
    Model {
        title: "initial".into(),
        details: Details {
            note: "note".into(),
        },
        rows: vec![
            Row {
                value: "first".into(),
            },
            Row {
                value: "second".into(),
            },
        ],
    }
}

fn record_model_changes(
    form: &gpui::Entity<Form<Model>>,
    cx: &mut gpui::App,
) -> Rc<RefCell<Vec<ModelChange<Model>>>> {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    cx.subscribe(form, move |_, event: &FormEvent<Model>, _| {
        if let FormEvent::ModelChanged(change) = event {
            recorded.borrow_mut().push(change.clone());
        }
    })
    .detach();
    changes
}

#[gpui::test]
fn frozen_change_impact_does_not_recompute_against_later_model_state(cx: &mut TestAppContext) {
    let (form, changes) = cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let changes = record_model_changes(&form, cx);
        (form, changes)
    });
    let first_value = cx.update(|cx| {
        let rows = Model::ROOT.then(Model::ROWS);
        let first = rows.items(&form, cx).remove(0);
        let first_value = first.clone().then(Row::VALUE);

        assert!(first_value.try_set(&form, "changed".into(), cx).unwrap());
        first_value
    });
    let first_change = changes.borrow()[0].clone();
    assert!(first_change.impact(&first_value).value_changed());
    assert!(!first_change.impact(&Model::TITLE).is_affected());

    cx.update(|cx| {
        assert!(Model::TITLE.set(&form, "later".into(), cx));
    });
    assert!(first_change.impact(&first_value).value_changed());
    assert!(!first_change.impact(&Model::TITLE).is_affected());
}

#[gpui::test]
fn wrong_session_dynamic_target_has_no_impact(cx: &mut TestAppContext) {
    let (first_form, second_form, changes) = cx.update(|cx| {
        let first_form = cx.new(|_| Form::new(model()));
        let second_form = cx.new(|_| Form::new(model()));
        let changes = record_model_changes(&first_form, cx);
        (first_form, second_form, changes)
    });
    let foreign_value = cx.update(|cx| {
        let rows = Model::ROOT.then(Model::ROWS);
        let foreign_item = rows.items(&second_form, cx).remove(0);
        foreign_item.then(Row::VALUE)
    });

    cx.update(|cx| {
        assert!(Model::TITLE.set(&first_form, "changed".into(), cx));
    });
    let impact = changes.borrow()[0].impact(&foreign_value);
    assert!(!impact.is_affected());
    assert!(!changes.borrow()[0].affects(&foreign_value));
}

#[gpui::test]
fn reorder_marks_collection_aggregate_without_marking_unchanged_items_as_values(
    cx: &mut TestAppContext,
) {
    let (form, changes) = cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let changes = record_model_changes(&form, cx);
        (form, changes)
    });
    let (rows, first_value) = cx.update(|cx| {
        let rows = Model::ROOT.then(Model::ROWS);
        let items = rows.items(&form, cx);
        let first = items[0].clone();
        let second = items[1].clone();
        let first_value = first.clone().then(Row::VALUE);

        rows.move_before(&form, &second, &first, cx).unwrap();
        (rows, first_value)
    });
    let change = changes.borrow()[0].clone();
    assert!(change.impact(&rows).value_changed());
    assert!(change.impact(&rows).structure_changed());
    assert!(!change.impact(&first_value).value_changed());
    assert!(!change.impact(&first_value).structure_changed());
    assert!(!change.impact(&first_value).retired());
}

#[gpui::test]
fn collection_composite_and_whole_model_mutations_publish_precise_change_sets(
    cx: &mut TestAppContext,
) {
    let (form, changes) = cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let changes = record_model_changes(&form, cx);
        (form, changes)
    });
    let rows = Model::ROOT.then(Model::ROWS);
    let details = Model::ROOT.then(Model::DETAILS);
    let details_note = details.clone().then(Details::NOTE);
    let original_first = cx.update(|cx| rows.items(&form, cx).remove(0));
    let original_first_value = original_first.clone().then(Row::VALUE);

    let appended = cx.update(|cx| {
        rows.append(
            &form,
            Row {
                value: "appended".into(),
            },
            cx,
        )
        .unwrap()
    });
    assert_eq!(changes.borrow().len(), 1);
    let change = changes.borrow().last().unwrap().clone();
    assert_eq!(change.kind(), ModelChangeKind::Edit);
    assert!(change.impact(&rows).value_changed());
    assert!(change.impact(&rows).structure_changed());
    assert!(!change.impact(&appended).retired());
    assert!(!change.impact(&original_first_value).value_changed());
    assert!(!change.impact(&original_first_value).structure_changed());

    changes.borrow_mut().clear();
    let inserted = cx.update(|cx| {
        rows.insert_before(
            &form,
            &original_first,
            Row {
                value: "inserted".into(),
            },
            cx,
        )
        .unwrap()
    });
    assert_eq!(changes.borrow().len(), 1);
    let change = changes.borrow().last().unwrap().clone();
    assert_eq!(change.kind(), ModelChangeKind::Edit);
    assert!(change.impact(&rows).value_changed());
    assert!(change.impact(&rows).structure_changed());
    assert!(!change.impact(&inserted).retired());
    assert!(!change.impact(&original_first_value).value_changed());

    changes.borrow_mut().clear();
    let retired = inserted.clone();
    cx.update(|cx| {
        rows.remove(&form, inserted, cx).unwrap();
    });
    assert_eq!(changes.borrow().len(), 1);
    let change = changes.borrow().last().unwrap().clone();
    assert_eq!(change.kind(), ModelChangeKind::Edit);
    assert!(change.impact(&rows).value_changed());
    assert!(change.impact(&rows).structure_changed());
    assert!(change.impact(&retired).retired());
    assert!(!change.impact(&original_first_value).value_changed());

    changes.borrow_mut().clear();
    let replaced_item = original_first.clone();
    let replacement = cx.update(|cx| {
        rows.replace_all(
            &form,
            vec![Row {
                value: "replacement".into(),
            }],
            cx,
        )
        .unwrap()
        .remove(0)
    });
    assert_eq!(changes.borrow().len(), 1);
    let change = changes.borrow().last().unwrap().clone();
    assert_eq!(change.kind(), ModelChangeKind::Edit);
    assert!(change.impact(&rows).value_changed());
    assert!(change.impact(&rows).structure_changed());
    assert!(change.impact(&replaced_item).retired());
    assert!(!change.impact(&replacement).retired());

    changes.borrow_mut().clear();
    cx.update(|cx| {
        assert!(Model::DETAILS.set(
            &form,
            Details {
                note: "changed".into(),
            },
            cx,
        ));
    });
    assert_eq!(changes.borrow().len(), 1);
    let change = changes.borrow().last().unwrap().clone();
    assert_eq!(change.kind(), ModelChangeKind::Edit);
    assert!(change.impact(&details).value_changed());
    assert!(change.impact(&details_note).value_changed());
    assert!(!change.impact(&details).structure_changed());
    assert!(!change.impact(&rows).is_affected());

    for (kind, install) in [
        (ModelChangeKind::Replace, 0_u8),
        (ModelChangeKind::Reset, 1),
        (ModelChangeKind::Rebase, 2),
    ] {
        changes.borrow_mut().clear();
        let old_item = cx.update(|cx| rows.items(&form, cx).remove(0));
        cx.update(|cx| match install {
            0 => form.update(cx, |form, cx| form.replace(model(), cx)),
            1 => form.update(cx, |form, cx| form.reset(cx)),
            2 => form.update(cx, |form, cx| {
                let mut next = model();
                next.title = "rebased".into();
                form.rebase(next, cx);
            }),
            _ => unreachable!(),
        });
        assert_eq!(changes.borrow().len(), 1);
        let change = changes.borrow().last().unwrap().clone();
        assert_eq!(change.kind(), kind);
        assert!(change.impact(&Model::TITLE).value_changed());
        assert!(change.impact(&rows).value_changed());
        assert!(change.impact(&rows).structure_changed());
        assert!(change.impact(&old_item).retired());
    }
}

#[gpui::test]
fn successful_mutation_publishes_exactly_one_event_and_notification(cx: &mut TestAppContext) {
    let events = Rc::new(Cell::new(0));
    let notifications = Rc::new(Cell::new(0));
    let form = cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let events = events.clone();
        cx.subscribe(&form, move |_, _: &FormEvent<Model>, _| {
            events.set(events.get() + 1);
        })
        .detach();
        let notifications = notifications.clone();
        cx.observe(&form, move |_, _| {
            notifications.set(notifications.get() + 1);
        })
        .detach();
        form
    });

    cx.update(|cx| {
        Model::ROOT
            .then(Model::ROWS)
            .append(
                &form,
                Row {
                    value: "third".into(),
                },
                cx,
            )
            .unwrap();
    });
    cx.run_until_parked();

    assert_eq!(events.get(), 1);
    assert_eq!(notifications.get(), 1);
}

#[gpui::test]
fn wrong_session_and_retired_mutations_have_no_visible_side_effects(cx: &mut TestAppContext) {
    let events = Rc::new(Cell::new(0));
    let notifications = Rc::new(Cell::new(0));
    let (form, foreign) = cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let foreign = cx.new(|_| Form::new(model()));
        let events = events.clone();
        cx.subscribe(&form, move |_, _: &FormEvent<Model>, _| {
            events.set(events.get() + 1);
        })
        .detach();
        let notifications = notifications.clone();
        cx.observe(&form, move |_, _| {
            notifications.set(notifications.get() + 1);
        })
        .detach();
        (form, foreign)
    });
    let rows = Model::ROOT.then(Model::ROWS);
    let foreign_value = cx.update(|cx| rows.items(&foreign, cx).remove(0).then(Row::VALUE));
    let before = cx.update(|cx| {
        (
            Model::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx)
                .iter()
                .map(|item| item.key())
                .collect::<Vec<_>>(),
        )
    });

    cx.update(|cx| {
        assert!(matches!(
            foreign_value.try_set(&form, "wrong".into(), cx),
            Err(ResolveError::WrongSession { .. })
        ));
    });
    cx.run_until_parked();
    let after = cx.update(|cx| {
        (
            Model::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx)
                .iter()
                .map(|item| item.key())
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(after, before);
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);

    let stale = cx.update(|cx| rows.items(&form, cx).remove(0));
    cx.update(|cx| {
        rows.remove(&form, stale.clone(), cx).unwrap();
    });
    cx.run_until_parked();
    events.set(0);
    notifications.set(0);
    let before = cx.update(|cx| {
        (
            Model::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx)
                .iter()
                .map(|item| item.key())
                .collect::<Vec<_>>(),
        )
    });
    cx.update(|cx| {
        assert!(matches!(
            stale.then(Row::VALUE).try_set(&form, "stale".into(), cx),
            Err(ResolveError::Retired { .. })
        ));
    });
    cx.run_until_parked();
    let after = cx.update(|cx| {
        (
            Model::ROOT.get(&form, cx),
            form.read(cx).revision(),
            form.read(cx).validation_report(),
            form.read(cx).is_validating(),
            rows.items(&form, cx)
                .iter()
                .map(|item| item.key())
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(after, before);
    assert_eq!(events.get(), 0);
    assert_eq!(notifications.get(), 0);
}
