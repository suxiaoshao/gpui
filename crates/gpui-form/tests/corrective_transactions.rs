use std::{
    fmt::Debug,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use gpui::{App, AppContext as _, Context, Entity, Subscription, TestAppContext};
use gpui_form::typed::{
    AsyncValidationIssue, FieldPath, FormEvent, FormField, FormFieldError, FormFieldId, FormItemId,
    FormRevision, FormStore as _, SubmitError, ToFormItemId, ValidationReport, ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct TransactionDetails {
    note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct TransactionChild {
    child_id: u64,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct TransactionRow {
    row_id: u64,
    value: String,
    #[form(group)]
    details: TransactionDetails,
    #[form(array(id = "child_id"))]
    children: Vec<TransactionChild>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct TransactionGroup {
    title: String,
    #[form(array(id = "row_id"))]
    rows: Vec<TransactionRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct TransactionRoot {
    #[form(required, validate(on_change))]
    title: String,
    #[form(group)]
    group: TransactionGroup,
    #[form(array(id = "row_id"))]
    rows: Vec<TransactionRow>,
}

fn row(row_id: u64, value: &str, child_id: u64) -> TransactionRow {
    TransactionRow {
        row_id,
        value: value.into(),
        details: TransactionDetails {
            note: format!("{value}-note"),
        },
        children: vec![TransactionChild {
            child_id,
            value: format!("{value}-child"),
        }],
    }
}

fn root_model() -> TransactionRoot {
    TransactionRoot {
        title: "root".into(),
        group: TransactionGroup {
            title: "group".into(),
            rows: vec![row(10, "group-row", 110)],
        },
        rows: vec![row(1, "first", 101), row(2, "second", 102)],
    }
}

fn new_root_form(cx: &mut TestAppContext) -> Entity<TransactionRootFormStore> {
    cx.update(|cx| cx.new(|cx| TransactionRootFormStore::from_value(root_model(), cx)))
}

struct EventRecorder<Field>
where
    Field: FormFieldId,
{
    events: Arc<Mutex<Vec<FormEvent<Field>>>>,
    notifications: Arc<AtomicUsize>,
    _subscriptions: Vec<Subscription>,
}

impl<Field> EventRecorder<Field>
where
    Field: FormFieldId,
{
    fn new<Form>(form: Entity<Form>, cx: &mut Context<Self>) -> Self
    where
        Form: gpui_form::typed::FormStore<Field = Field>,
    {
        let events = Arc::new(Mutex::new(Vec::new()));
        let notifications = Arc::new(AtomicUsize::new(0));
        let observed_events = events.clone();
        let observed_notifications = notifications.clone();
        let event_subscription = cx.subscribe(
            &form,
            move |_recorder, _form, event: &FormEvent<Field>, _cx| {
                observed_events.lock().unwrap().push(event.clone());
            },
        );
        let notify_subscription = cx.observe(&form, move |_recorder, _form, _cx| {
            observed_notifications.fetch_add(1, Ordering::SeqCst);
        });
        Self {
            events,
            notifications,
            _subscriptions: vec![event_subscription, notify_subscription],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StoreSnapshot<Model> {
    value: Model,
    baseline: Model,
    revision: FormRevision,
    validation_report: ValidationReport,
    is_validating: bool,
}

fn store_snapshot<Form>(form: &Entity<Form>, cx: &App) -> StoreSnapshot<Form::Model>
where
    Form: gpui_form::typed::FormStore,
{
    form.read_with(cx, |form, _| StoreSnapshot {
        value: form.value().clone(),
        baseline: form.baseline().clone(),
        revision: form.revision(),
        validation_report: form.validation_report(),
        is_validating: form.is_validating(),
    })
}

fn recorder_counts<Form>(recorder: &Entity<EventRecorder<Form::Field>>, cx: &App) -> (usize, usize)
where
    Form: gpui_form::typed::FormStore,
{
    recorder.read_with(cx, |recorder, _| {
        (
            recorder.events.lock().unwrap().len(),
            recorder.notifications.load(Ordering::SeqCst),
        )
    })
}

fn assert_successful_field_write<Form, T>(
    cx: &mut TestAppContext,
    form: &Entity<Form>,
    recorder: &Entity<EventRecorder<Form::Field>>,
    field: FormField<Form, T>,
    expected_path: FieldPath,
    expected_validation_path: FieldPath,
    value: T,
) where
    Form: gpui_form::typed::FormStore,
    T: Clone + PartialEq + 'static,
{
    assert_eq!(field.path(), &expected_path);
    assert_eq!(field.validation_path(), &expected_validation_path);
    let before_revision = cx.update(|cx| form.read(cx).revision());
    let before_counts = cx.update(|cx| recorder_counts::<Form>(recorder, cx));

    cx.update(|cx| field.set_user_value(value, cx).unwrap());

    cx.update(|cx| {
        let revision = form.read(cx).revision();
        assert_eq!(revision.get(), before_revision.get() + 1);
        let (events, notifications) = recorder_counts::<Form>(recorder, cx);
        assert_eq!(events, before_counts.0 + 1);
        assert_eq!(notifications, before_counts.1 + 1);
        recorder.read_with(cx, |recorder, _| {
            assert!(matches!(
                recorder.events.lock().unwrap().last(),
                Some(FormEvent::FieldChanged { path, revision: event_revision, .. })
                    if path == &expected_path && *event_revision == revision
            ));
        });
    });
}

fn assert_field_write_noop<Form, T>(
    cx: &mut TestAppContext,
    form: &Entity<Form>,
    recorder: &Entity<EventRecorder<Form::Field>>,
    field: FormField<Form, T>,
    value: T,
    expected: Result<(), FormFieldError>,
) where
    Form: gpui_form::typed::FormStore,
    Form::Model: Debug,
    T: Clone + PartialEq + 'static,
{
    let before = cx.update(|cx| store_snapshot(form, cx));
    let before_counts = cx.update(|cx| recorder_counts::<Form>(recorder, cx));

    assert_eq!(cx.update(|cx| field.set_user_value(value, cx)), expected);

    cx.update(|cx| {
        assert_eq!(store_snapshot(form, cx), before);
        assert_eq!(recorder_counts::<Form>(recorder, cx), before_counts);
    });
}

fn assert_model_replaced_once<Form>(
    cx: &mut TestAppContext,
    form: &Entity<Form>,
    recorder: &Entity<EventRecorder<Form::Field>>,
    action: impl FnOnce(&mut Form, &mut Context<Form>),
) where
    Form: gpui_form::typed::FormStore,
{
    let before_revision = cx.update(|cx| form.read(cx).revision());
    let before_counts = cx.update(|cx| recorder_counts::<Form>(recorder, cx));

    cx.update(|cx| form.update(cx, action));

    cx.update(|cx| {
        let revision = form.read(cx).revision();
        assert_eq!(revision.get(), before_revision.get() + 1);
        let (events, notifications) = recorder_counts::<Form>(recorder, cx);
        assert_eq!(events, before_counts.0 + 1);
        assert_eq!(notifications, before_counts.1 + 1);
        recorder.read_with(cx, |recorder, _| {
            assert!(matches!(
                recorder.events.lock().unwrap().last(),
                Some(FormEvent::ModelReplaced { revision: event_revision })
                    if *event_revision == revision
            ));
        });
    });
}

#[gpui::test]
fn composed_writes_use_one_transaction_and_the_exact_handle_paths(cx: &mut TestAppContext) {
    let form = new_root_form(cx);
    let recorder = cx
        .update(|cx| cx.new(|cx| EventRecorder::new::<TransactionRootFormStore>(form.clone(), cx)));

    let group_path = FieldPath::field("group");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionRootFormStore::group_field(&form),
        group_path.clone(),
        group_path.clone(),
        TransactionGroup {
            title: "group-replaced".into(),
            rows: vec![row(10, "group-row-replaced", 110)],
        },
    );

    let root_array_path = FieldPath::field("rows");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionRootFormStore::rows_field(&form),
        root_array_path.clone(),
        root_array_path.clone(),
        vec![row(1, "first-array", 101), row(2, "second", 102)],
    );

    let item_path = root_array_path.join_item(FormItemId::new(1));
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionRootFormStore::rows_item(&form, FormItemId::new(1)),
        item_path.clone(),
        item_path.clone(),
        row(1, "first-item", 101),
    );

    let item_leaf_path = item_path.join_field("value");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionRowFormStore::value_in(TransactionRootFormStore::rows_item(
            &form,
            FormItemId::new(1),
        )),
        item_leaf_path.clone(),
        item_leaf_path,
        "first-leaf".into(),
    );

    let array_in_group_path = group_path
        .join_field("rows")
        .join_item(FormItemId::new(10))
        .join_field("value");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionRowFormStore::value_in(TransactionGroupFormStore::rows_item_in(
            TransactionRootFormStore::group_field(&form),
            FormItemId::new(10),
        )),
        array_in_group_path.clone(),
        array_in_group_path,
        "array-in-group".into(),
    );

    let group_in_array_path = root_array_path
        .join_item(FormItemId::new(1))
        .join_field("details")
        .join_field("note");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionDetailsFormStore::note_in(TransactionRowFormStore::details_in(
            TransactionRootFormStore::rows_item(&form, FormItemId::new(1)),
        )),
        group_in_array_path.clone(),
        group_in_array_path,
        "group-in-array".into(),
    );

    let nested_array_path = root_array_path
        .join_item(FormItemId::new(1))
        .join_field("children")
        .join_item(FormItemId::new(101))
        .join_field("value");
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        TransactionChildFormStore::value_in(TransactionRowFormStore::children_item_in(
            TransactionRootFormStore::rows_item(&form, FormItemId::new(1)),
            FormItemId::new(101),
        )),
        nested_array_path.clone(),
        nested_array_path,
        "nested-array".into(),
    );

    let projection_path = group_path.join_projection("title_alias");
    let projection = TransactionRootFormStore::group_field(&form).project_value(
        "title_alias",
        |group| Some(group.title.clone()),
        |group, value| {
            group.title = value;
            true
        },
    );
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        projection,
        projection_path,
        group_path,
        "projected-title".into(),
    );

    cx.update(|cx| {
        assert_eq!(form.read(cx).revision().get(), 8);
        assert_eq!(
            recorder_counts::<TransactionRootFormStore>(&recorder, cx),
            (8, 8)
        );
    });
}

#[gpui::test]
fn equal_and_failed_lens_writes_are_complete_noops(cx: &mut TestAppContext) {
    let form = new_root_form(cx);
    let recorder = cx
        .update(|cx| cx.new(|cx| EventRecorder::new::<TransactionRootFormStore>(form.clone(), cx)));
    let title = TransactionRootFormStore::title_field(&form);

    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        title.clone(),
        FieldPath::field("title"),
        FieldPath::field("title"),
        String::new(),
    );
    cx.update(|cx| {
        assert!(!form.read(cx).validation_report().is_valid());
    });

    assert_field_write_noop(cx, &form, &recorder, title, String::new(), Ok(()));

    let missing = TransactionRowFormStore::value_in(TransactionRootFormStore::rows_item(
        &form,
        FormItemId::new(999),
    ));
    assert_field_write_noop(
        cx,
        &form,
        &recorder,
        missing,
        "missing".into(),
        Err(FormFieldError::ValueUnavailable),
    );

    let rejected_projection = TransactionRootFormStore::group_field(&form).project_value(
        "rejected",
        |group| Some(group.title.clone()),
        |_group, _value| false,
    );
    assert_field_write_noop(
        cx,
        &form,
        &recorder,
        rejected_projection,
        "rejected".into(),
        Err(FormFieldError::ValueUnavailable),
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OptionalId(Option<u64>);

impl ToFormItemId for OptionalId {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        self.0.map(FormItemId::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct OptionalIdRow {
    row_id: OptionalId,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct OptionalIdRoot {
    #[form(array(id = "row_id"))]
    rows: Vec<OptionalIdRow>,
}

#[gpui::test]
fn identified_item_and_id_leaf_reject_different_or_unconvertible_ids_without_side_effects(
    cx: &mut TestAppContext,
) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            OptionalIdRootFormStore::from_value(
                OptionalIdRoot {
                    rows: vec![OptionalIdRow {
                        row_id: OptionalId(Some(41)),
                        value: "value".into(),
                    }],
                },
                cx,
            )
        })
    });
    let recorder = cx
        .update(|cx| cx.new(|cx| EventRecorder::new::<OptionalIdRootFormStore>(form.clone(), cx)));
    let item = OptionalIdRootFormStore::rows_item(&form, FormItemId::new(41));
    let id_leaf = OptionalIdRowFormStore::row_id_in(item.clone());

    cx.update(|cx| {
        id_leaf
            .start_async_validation(
                "pending-id-check",
                ValidationTrigger::Change,
                |_| std::future::pending::<Result<(), AsyncValidationIssue>>(),
                cx,
            )
            .unwrap();
        assert!(form.read(cx).is_validating());
    });

    for next in [OptionalId(Some(99)), OptionalId(None)] {
        assert_field_write_noop(
            cx,
            &form,
            &recorder,
            item.clone(),
            OptionalIdRow {
                row_id: next.clone(),
                value: "replacement".into(),
            },
            Err(FormFieldError::ItemIdentityChanged),
        );
        assert_field_write_noop(
            cx,
            &form,
            &recorder,
            id_leaf.clone(),
            next,
            Err(FormFieldError::ItemIdentityChanged),
        );
    }

    cx.update(|cx| {
        OptionalIdRootFormStore::rows_field(&form)
            .set_user_value(
                vec![
                    OptionalIdRow {
                        row_id: OptionalId(None),
                        value: "invalid".into(),
                    },
                    OptionalIdRow {
                        row_id: OptionalId(Some(7)),
                        value: "duplicate-a".into(),
                    },
                    OptionalIdRow {
                        row_id: OptionalId(Some(7)),
                        value: "duplicate-b".into(),
                    },
                ],
                cx,
            )
            .unwrap();
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        let report = form.read(cx).validation_report();
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.code == "invalid_item_id")
        );
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.code == "duplicate_item_id")
        );
    });
}

#[gpui::test]
fn whole_array_and_form_lifecycle_keep_stable_identity_nominal(cx: &mut TestAppContext) {
    let form = new_root_form(cx);
    let recorder = cx
        .update(|cx| cx.new(|cx| EventRecorder::new::<TransactionRootFormStore>(form.clone(), cx)));
    let rows = TransactionRootFormStore::rows_field(&form);
    let old_item = TransactionRootFormStore::rows_item(&form, FormItemId::new(1));
    let old_leaf = TransactionRowFormStore::value_in(old_item.clone());
    let old_leaf_path = old_leaf.path().clone();

    cx.update(|cx| {
        old_leaf
            .start_async_validation(
                "pending-old-item-check",
                ValidationTrigger::Change,
                |_| std::future::pending::<Result<(), AsyncValidationIssue>>(),
                cx,
            )
            .unwrap();
        assert!(form.read(cx).is_validating_at(&old_leaf_path));
    });

    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        rows.clone(),
        FieldPath::field("rows"),
        FieldPath::field("rows"),
        vec![row(2, "second", 102), row(3, "inserted", 103)],
    );
    cx.update(|cx| {
        assert_eq!(old_item.value(cx), Err(FormFieldError::ValueUnavailable));
        assert_eq!(old_leaf.value(cx), Err(FormFieldError::ValueUnavailable));
        assert!(!form.read(cx).is_validating_at(&old_leaf_path));
    });

    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        rows.clone(),
        FieldPath::field("rows"),
        FieldPath::field("rows"),
        vec![
            row(3, "inserted", 103),
            row(1, "reinserted", 101),
            row(2, "second", 102),
        ],
    );
    cx.update(|cx| {
        assert_eq!(old_item.value(cx).unwrap().value, "reinserted");
        assert_eq!(old_leaf.value(cx).unwrap(), "reinserted");
    });

    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        rows,
        FieldPath::field("rows"),
        FieldPath::field("rows"),
        vec![
            row(2, "second", 102),
            row(1, "reordered", 101),
            row(3, "inserted", 103),
        ],
    );
    assert_successful_field_write(
        cx,
        &form,
        &recorder,
        old_leaf.clone(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(1))
            .join_field("value"),
        FieldPath::field("rows")
            .join_item(FormItemId::new(1))
            .join_field("value"),
        "written-after-reorder".into(),
    );
    cx.update(|cx| {
        let rows = &form.read(cx).value().rows;
        assert_eq!(rows[0].row_id, 2);
        assert_eq!(rows[1].row_id, 1);
        assert_eq!(rows[1].value, "written-after-reorder");
    });

    assert_model_replaced_once(cx, &form, &recorder, |form, cx| form.reset(cx));
    cx.update(|cx| {
        assert_eq!(old_leaf.value(cx).unwrap(), "first");
        assert_eq!(form.read(cx).baseline(), &root_model());
    });

    let mut rebased = root_model();
    rebased.rows[0].value = "rebased".into();
    assert_model_replaced_once(cx, &form, &recorder, move |form, cx| {
        form.rebase(rebased, cx)
    });
    cx.update(|cx| {
        assert_eq!(old_leaf.value(cx).unwrap(), "rebased");
        assert_eq!(form.read(cx).value(), form.read(cx).baseline());
    });

    let expected = cx.update(|cx| form.read(cx).revision());
    let mut saved = cx.update(|cx| form.read(cx).value().clone());
    saved.rows[0].value = "cas-saved".into();
    assert_model_replaced_once(cx, &form, &recorder, move |form, cx| {
        assert!(form.rebase_if_revision(expected, saved, cx));
    });
    cx.update(|cx| {
        assert_eq!(old_leaf.value(cx).unwrap(), "cas-saved");
        assert_eq!(form.read(cx).value(), form.read(cx).baseline());
    });

    let before = cx.update(|cx| store_snapshot(&form, cx));
    let before_counts = cx.update(|cx| recorder_counts::<TransactionRootFormStore>(&recorder, cx));
    let mut stale_value = before.value.clone();
    stale_value.rows.retain(|row| row.row_id != 1);
    assert!(!cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.rebase_if_revision(expected, stale_value, cx)
        })
    }));
    cx.update(|cx| {
        assert_eq!(store_snapshot(&form, cx), before);
        assert_eq!(
            recorder_counts::<TransactionRootFormStore>(&recorder, cx),
            before_counts
        );
        assert_eq!(old_leaf.value(cx).unwrap(), "cas-saved");
    });
}
