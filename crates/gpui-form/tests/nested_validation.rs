use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use gpui::{App, AppContext as _, Context, Entity, Subscription, TestAppContext};
use gpui_form::typed::{
    FieldPath, FormEvent, FormFieldError, FormItemId, FormModelSchema as _, FormSchemaPathError,
    FormStore as _, GardePathMapper as _, NoValidationContext, SubmitError, SubmitTransform,
    ToFormItemId, TransformReport, ValidationAdapter, ValidationAdapterReport, ValidationContext,
    ValidationIssue, ValidationMessage, ValidationScope, ValidationSource, ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct NestedChild {
    #[form(validate(on_mount, on_change, on_blur, on_dynamic, on_submit))]
    change_value: String,
    #[form(validate(on_submit))]
    submit_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct NestedRow {
    row_id: u64,
    #[form(validate(on_change, on_submit))]
    value: String,
}

#[derive(Clone, Debug, Default)]
struct NestedValidator;

impl ValidationAdapter<NestedRoot> for NestedValidator {
    type Context = NoValidationContext;

    fn validate(
        &self,
        model: &NestedRoot,
        trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut report = ValidationAdapterReport::default();
        if model.child.change_value.is_empty() {
            report.push(adapter_issue(
                FieldPath::field("child").join_field("change_value"),
                trigger,
                "child_change",
            ));
        }
        if model.child.submit_value.is_empty() {
            report.push(adapter_issue(
                FieldPath::field("child").join_field("submit_value"),
                trigger,
                "child_submit",
            ));
        }
        for row in &model.rows {
            if row.value.is_empty() {
                report.push(adapter_issue(
                    FieldPath::field("rows")
                        .join_item(FormItemId::new(row.row_id))
                        .join_field("value"),
                    trigger,
                    "row_value",
                ));
            }
        }
        report
    }
}

fn adapter_issue(
    path: FieldPath,
    trigger: ValidationTrigger,
    code: &'static str,
) -> ValidationIssue {
    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::App("nested-test".into()),
        code,
        ValidationMessage::key(code),
    )
}

static TRANSFORM_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default)]
struct CountingTransform;

impl SubmitTransform<NestedRoot> for CountingTransform {
    type Output = NestedRoot;

    fn transform(&self, model: &NestedRoot) -> Result<Self::Output, TransformReport> {
        TRANSFORM_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(model.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(
    validation(adapter = NestedValidator),
    transform(adapter = CountingTransform)
)]
struct NestedRoot {
    // Ancestors intentionally declare no validation triggers. The nested leaf schema owns them.
    #[form(group)]
    child: NestedChild,
    #[form(array(id = "row_id"))]
    rows: Vec<NestedRow>,
}

fn nested_root() -> NestedRoot {
    NestedRoot {
        child: NestedChild {
            change_value: "valid".into(),
            submit_value: "valid".into(),
        },
        rows: vec![NestedRow {
            row_id: 41,
            value: "valid".into(),
        }],
    }
}

fn new_nested_form(cx: &mut TestAppContext) -> gpui::Entity<NestedRootFormStore> {
    cx.update(|cx| {
        cx.new(|cx| {
            NestedRootFormStore::from_value_with_validation_context(
                nested_root(),
                NoValidationContext,
                cx,
            )
        })
    })
}

fn count_issues_at(
    form: &Entity<NestedRootFormStore>,
    code: &str,
    path: &FieldPath,
    cx: &App,
) -> usize {
    form.read(cx)
        .validation_report()
        .issues()
        .iter()
        .filter(|issue| issue.code == code && issue.path.as_ref() == Some(path))
        .count()
}

struct NestedEventRecorder {
    events: Arc<Mutex<Vec<FormEvent<NestedRootField>>>>,
    notifications: Arc<AtomicUsize>,
    _subscriptions: Vec<Subscription>,
}

impl NestedEventRecorder {
    fn new(form: Entity<NestedRootFormStore>, cx: &mut Context<Self>) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let notifications = Arc::new(AtomicUsize::new(0));
        let observed_events = events.clone();
        let observed_notifications = notifications.clone();
        let event_subscription = cx.subscribe(
            &form,
            move |_recorder, _form, event: &FormEvent<NestedRootField>, _cx| {
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

#[gpui::test]
fn nested_group_leaf_write_uses_the_leaf_change_schema(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let field = NestedChildFormStore::change_value_in(NestedRootFormStore::child_field(&form));

    cx.update(|cx| {
        field.set_user_value(String::new(), cx).unwrap();

        let report = form.read(cx).validation_report();
        assert!(report.issues().iter().any(|issue| {
            issue.code == "child_change"
                && issue.path.as_ref()
                    == Some(&FieldPath::field("child").join_field("change_value"))
                && issue.trigger == ValidationTrigger::Change
        }));
    });
}

#[gpui::test]
fn nested_array_leaf_write_uses_the_leaf_change_schema(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let row = NestedRootFormStore::rows_item(&form, FormItemId::new(41));
    let field = NestedRowFormStore::value_in(row);

    cx.update(|cx| {
        field.set_user_value(String::new(), cx).unwrap();

        let expected = FieldPath::field("rows")
            .join_item(FormItemId::new(41))
            .join_field("value");
        let report = form.read(cx).validation_report();
        assert!(report.issues().iter().any(|issue| {
            issue.code == "row_value"
                && issue.path.as_ref() == Some(&expected)
                && issue.trigger == ValidationTrigger::Change
        }));
    });
}

#[gpui::test]
fn whole_group_array_and_item_writes_use_descendant_leaf_schemas(cx: &mut TestAppContext) {
    let group_form = new_nested_form(cx);
    let array_form = new_nested_form(cx);
    let item_form = new_nested_form(cx);

    cx.update(|cx| {
        NestedRootFormStore::child_field(&group_form)
            .set_user_value(
                NestedChild {
                    change_value: String::new(),
                    submit_value: "valid".into(),
                },
                cx,
            )
            .unwrap();
        let group_leaf = FieldPath::field("child").join_field("change_value");
        assert_eq!(
            count_issues_at(&group_form, "child_change", &group_leaf, cx),
            1
        );

        NestedRootFormStore::rows_field(&array_form)
            .set_user_value(
                vec![NestedRow {
                    row_id: 41,
                    value: String::new(),
                }],
                cx,
            )
            .unwrap();
        let array_leaf = FieldPath::field("rows")
            .join_item(FormItemId::new(41))
            .join_field("value");
        assert_eq!(
            count_issues_at(&array_form, "row_value", &array_leaf, cx),
            1
        );

        NestedRootFormStore::rows_item(&item_form, FormItemId::new(41))
            .set_user_value(
                NestedRow {
                    row_id: 41,
                    value: String::new(),
                },
                cx,
            )
            .unwrap();
        let item_leaf = FieldPath::field("rows")
            .join_item(FormItemId::new(41))
            .join_field("value");
        assert_eq!(count_issues_at(&item_form, "row_value", &item_leaf, cx), 1);
    });
}

#[gpui::test]
fn nested_array_leaf_write_does_not_regenerate_sibling_id_issues(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            NestedRootFormStore::from_value_with_validation_context(
                NestedRoot {
                    child: NestedChild {
                        change_value: "valid".into(),
                        submit_value: "valid".into(),
                    },
                    rows: vec![
                        NestedRow {
                            row_id: 41,
                            value: "editable".into(),
                        },
                        NestedRow {
                            row_id: 99,
                            value: "duplicate-a".into(),
                        },
                        NestedRow {
                            row_id: 99,
                            value: "duplicate-b".into(),
                        },
                    ],
                },
                NoValidationContext,
                cx,
            )
        })
    });
    let row = NestedRootFormStore::rows_item(&form, FormItemId::new(41));
    let field = NestedRowFormStore::value_in(row);

    cx.update(|cx| {
        let duplicate_path = FieldPath::field("rows").join_item(FormItemId::new(99));
        assert_eq!(
            count_issues_at(&form, "duplicate_item_id", &duplicate_path, cx),
            1
        );
        field.set_user_value("changed".into(), cx).unwrap();
        assert_eq!(
            count_issues_at(&form, "duplicate_item_id", &duplicate_path, cx),
            1
        );
    });
}

#[gpui::test]
fn nested_leaf_write_uses_one_transaction_event_and_notification(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let recorder = cx.update(|cx| cx.new(|cx| NestedEventRecorder::new(form.clone(), cx)));
    let field = NestedChildFormStore::change_value_in(NestedRootFormStore::child_field(&form));

    cx.update(|cx| field.set_user_value(String::new(), cx).unwrap());

    let (events, notifications) = cx.update(|cx| {
        recorder.read_with(cx, |recorder, _| {
            (
                recorder.events.lock().unwrap().clone(),
                recorder.notifications.load(Ordering::SeqCst),
            )
        })
    });
    assert_eq!(notifications, 1);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        FormEvent::FieldChanged { path, revision, .. }
            if path == &FieldPath::field("child").join_field("change_value")
                && revision.get() == 1
    ));

    // Equal writes are complete no-ops: no second revision, event, validation pass, or notify.
    cx.update(|cx| field.set_user_value(String::new(), cx).unwrap());
    let (events, notifications) = cx.update(|cx| {
        recorder.read_with(cx, |recorder, _| {
            (
                recorder.events.lock().unwrap().len(),
                recorder.notifications.load(Ordering::SeqCst),
            )
        })
    });
    assert_eq!(events, 1);
    assert_eq!(notifications, 1);
    assert_eq!(form.read_with(cx, |form, _| form.revision().get()), 1);
}

#[gpui::test]
fn nested_leaf_schema_filters_all_validation_triggers(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let field = NestedChildFormStore::change_value_in(NestedRootFormStore::child_field(&form));

    cx.update(|cx| {
        field.set_user_value(String::new(), cx).unwrap();
        for trigger in [
            ValidationTrigger::Mount,
            ValidationTrigger::Change,
            ValidationTrigger::Blur,
            ValidationTrigger::Dynamic,
            ValidationTrigger::Submit,
        ] {
            field.validate(trigger, cx).unwrap();
            assert!(
                form.read(cx)
                    .validation_report()
                    .issues()
                    .iter()
                    .any(|issue| {
                        issue.code == "child_change"
                            && issue.trigger == trigger
                            && issue.path.as_ref()
                                == Some(&FieldPath::field("child").join_field("change_value"))
                    })
            );
        }
    });
}

#[gpui::test]
fn prepare_submit_keeps_nested_submit_issue_and_skips_transform(cx: &mut TestAppContext) {
    TRANSFORM_CALLS.store(0, Ordering::SeqCst);
    let form = new_nested_form(cx);

    cx.update(|cx| {
        let submit_value =
            NestedChildFormStore::submit_value_in(NestedRootFormStore::child_field(&form));
        // `set` still uses the single typed-field transaction, including change validation. The
        // submit-only issue must remain absent until `prepare_submit` runs.
        submit_value.set(String::new(), cx).unwrap();
        assert!(
            form.read(cx)
                .validation_report()
                .issues()
                .iter()
                .all(|issue| issue.code != "child_submit")
        );

        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        assert_eq!(TRANSFORM_CALLS.load(Ordering::SeqCst), 0);
        assert!(
            form.read(cx)
                .validation_report()
                .issues()
                .iter()
                .any(|issue| {
                    issue.code == "child_submit"
                        && issue.trigger == ValidationTrigger::Submit
                        && issue.path.as_ref()
                            == Some(&FieldPath::field("child").join_field("submit_value"))
                })
        );
    });
}

#[test]
fn array_container_and_direct_item_root_use_the_array_schema() {
    let model = nested_root();
    let id = FormItemId::new(41);

    let container = model
        .schema_at_path(FieldPath::field("rows").segments())
        .unwrap();
    let item_root_path = FieldPath::field("rows").join_item(id);
    let item_root = model.schema_at_path(item_root_path.segments()).unwrap();
    let item_leaf_path = item_root_path.join_field("value");
    let item_leaf = model.schema_at_path(item_leaf_path.segments()).unwrap();

    assert_eq!(container.name(), "rows");
    assert_eq!(item_root.name(), "rows");
    assert_eq!(item_leaf.name(), "value");
    assert!(!container.triggers().change);
    assert!(item_leaf.triggers().change);
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct DeepItem {
    item_id: u64,
    #[form(validate(on_change))]
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct DeepGroup {
    #[form(array(id = "item_id"))]
    items: Vec<DeepItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct DeepRow {
    row_id: u64,
    #[form(group)]
    group: DeepGroup,
    #[form(array(id = "item_id"))]
    children: Vec<DeepItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct DeepRoot {
    #[form(group)]
    group: DeepGroup,
    #[form(array(id = "row_id"))]
    rows: Vec<DeepRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct GenericNestedRow<T>
where
    T: Clone + PartialEq + 'static,
{
    row_id: u64,
    value: T,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct GenericArrayRoot<T>
where
    T: Clone + PartialEq + 'static,
{
    #[form(array(id = "row_id"))]
    rows: Vec<GenericNestedRow<T>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OptionalItemId(Option<u64>);

impl ToFormItemId for OptionalItemId {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        self.0.map(FormItemId::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct OptionalIdRow {
    row_id: OptionalItemId,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct OptionalIdRoot {
    #[form(array(id = "row_id"))]
    rows: Vec<OptionalIdRow>,
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
struct GardeNestedChild {
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    value: String,
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
struct GardeNestedRow {
    #[garde(skip)]
    row_id: u64,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    value: String,
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[form(validation(adapter = "garde"))]
struct GardeNestedRoot {
    #[form(group)]
    #[garde(dive)]
    child: GardeNestedChild,
    #[form(array(id = "row_id"))]
    #[garde(dive)]
    rows: Vec<GardeNestedRow>,
}

fn deep_root() -> DeepRoot {
    DeepRoot {
        group: DeepGroup {
            items: vec![DeepItem {
                item_id: 5,
                value: "group-array".into(),
            }],
        },
        rows: vec![DeepRow {
            row_id: 7,
            group: DeepGroup {
                items: vec![DeepItem {
                    item_id: 5,
                    value: "array-group-array".into(),
                }],
            },
            children: vec![DeepItem {
                item_id: 9,
                value: "array-array".into(),
            }],
        }],
    }
}

#[test]
fn recursive_schema_resolution_handles_group_and_array_compositions() {
    let model = deep_root();

    let array_in_group = FieldPath::field("group")
        .join_field("items")
        .join_item(FormItemId::new(5))
        .join_field("value");
    let group_in_array = FieldPath::field("rows")
        .join_item(FormItemId::new(7))
        .join_field("group");
    let nested_array = group_in_array
        .join_field("items")
        .join_item(FormItemId::new(5))
        .join_field("value");
    let array_in_array = FieldPath::field("rows")
        .join_item(FormItemId::new(7))
        .join_field("children")
        .join_item(FormItemId::new(9))
        .join_field("value");

    assert_eq!(
        model
            .schema_at_path(array_in_group.segments())
            .unwrap()
            .name(),
        "value"
    );
    assert_eq!(
        model
            .schema_at_path(group_in_array.segments())
            .unwrap()
            .name(),
        "group"
    );
    assert_eq!(
        model
            .schema_at_path(nested_array.segments())
            .unwrap()
            .name(),
        "value"
    );
    assert_eq!(
        model
            .schema_at_path(array_in_array.segments())
            .unwrap()
            .name(),
        "value"
    );
}

#[test]
fn recursive_garde_mapping_handles_container_item_and_leaf_paths() {
    let root = nested_root();
    assert_eq!(
        root.map_garde_path("rows").unwrap(),
        FieldPath::field("rows")
    );
    assert_eq!(
        root.map_garde_path("rows[0]").unwrap(),
        FieldPath::field("rows").join_item(FormItemId::new(41))
    );
    assert_eq!(
        root.map_garde_path("rows[0].value").unwrap(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(41))
            .join_field("value")
    );

    let model = deep_root();

    assert_eq!(
        model.map_garde_path("group.items").unwrap(),
        FieldPath::field("group").join_field("items")
    );
    assert_eq!(
        model.map_garde_path("rows[0]").unwrap(),
        FieldPath::field("rows").join_item(FormItemId::new(7))
    );
    assert_eq!(
        model.map_garde_path("rows[0].group.items").unwrap(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(7))
            .join_field("group")
            .join_field("items")
    );
    assert_eq!(
        model.map_garde_path("rows[0].group.items[0]").unwrap(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(7))
            .join_field("group")
            .join_field("items")
            .join_item(FormItemId::new(5))
    );
    assert_eq!(
        model
            .map_garde_path("rows[0].group.items[0].value")
            .unwrap(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(7))
            .join_field("group")
            .join_field("items")
            .join_item(FormItemId::new(5))
            .join_field("value")
    );
    assert_eq!(
        model.map_garde_path("rows[0].children[0]").unwrap(),
        FieldPath::field("rows")
            .join_item(FormItemId::new(7))
            .join_field("children")
            .join_item(FormItemId::new(9))
    );
}

#[test]
fn schema_resolution_returns_typed_errors_instead_of_falling_back() {
    let mut model = nested_root();
    let missing = FieldPath::field("rows").join_item(FormItemId::new(99));
    assert_eq!(
        model.schema_at_path(missing.segments()),
        Err(FormSchemaPathError::MissingItem(FormItemId::new(99)))
    );

    model.rows.push(NestedRow {
        row_id: 41,
        value: "duplicate".into(),
    });
    let duplicate = FieldPath::field("rows").join_item(FormItemId::new(41));
    assert_eq!(
        model.schema_at_path(duplicate.segments()),
        Err(FormSchemaPathError::DuplicateItem(FormItemId::new(41)))
    );
    assert_eq!(
        model.schema_at_path(FieldPath::field("unknown").segments()),
        Err(FormSchemaPathError::UnknownField)
    );
    assert_eq!(
        model.schema_at_path(
            FieldPath::field("child")
                .join_projection("display")
                .segments()
        ),
        Err(FormSchemaPathError::Projection)
    );
}

#[test]
fn invalid_adapter_paths_block_even_when_they_are_outside_the_requested_scope() {
    let model = nested_root();
    let invalid_path = FieldPath::field("unknown").join_field("leaf");
    let report = ValidationAdapterReport::new(vec![adapter_issue(
        invalid_path,
        ValidationTrigger::Change,
        "unknown_path",
    )]);
    let issues = gpui_form::typed::normalize_adapter_report(
        &model,
        ValidationTrigger::Change,
        &ValidationScope::Field(FieldPath::field("child").join_field("change_value")),
        report,
    );

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].source, ValidationSource::Internal);
    assert_eq!(issues[0].code, "form_schema_path_resolution");
    assert!(issues[0].path.is_none());
}

#[test]
fn nominal_generic_array_items_resolve_nested_schema() {
    let model = GenericArrayRoot {
        rows: vec![GenericNestedRow {
            row_id: 12,
            value: 7u32,
        }],
    };
    let path = FieldPath::field("rows")
        .join_item(FormItemId::new(12))
        .join_field("value");
    assert_eq!(
        model.schema_at_path(path.segments()).unwrap().name(),
        "value"
    );
}

#[cfg(feature = "garde-adapter")]
#[gpui::test]
fn garde_nested_submit_uses_group_and_array_leaf_schemas(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            GardeNestedRootFormStore::from_value_with_validation_context(
                GardeNestedRoot {
                    child: GardeNestedChild {
                        value: String::new(),
                    },
                    rows: vec![GardeNestedRow {
                        row_id: 41,
                        value: String::new(),
                    }],
                },
                (),
                cx,
            )
        })
    });

    cx.update(|cx| {
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        let issues = form.read(cx).validation_report();
        assert!(issues.issues().iter().any(|issue| {
            issue.source == ValidationSource::Garde
                && issue.trigger == ValidationTrigger::Submit
                && issue.path.as_ref() == Some(&FieldPath::field("child").join_field("value"))
        }));
        assert!(issues.issues().iter().any(|issue| {
            issue.source == ValidationSource::Garde
                && issue.trigger == ValidationTrigger::Submit
                && issue.path.as_ref()
                    == Some(
                        &FieldPath::field("rows")
                            .join_item(FormItemId::new(41))
                            .join_field("value"),
                    )
        }));
        assert!(
            issues
                .issues()
                .iter()
                .all(|issue| issue.source != ValidationSource::Internal)
        );
    });
}

#[gpui::test]
fn identified_item_id_leaf_cannot_change_the_stable_id(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let item = NestedRootFormStore::rows_item(&form, FormItemId::new(41));
    let row_id = NestedRowFormStore::row_id_in(item);

    cx.update(|cx| {
        let before = form.read(cx).value().clone();
        let revision = form.read(cx).revision();

        assert_eq!(
            row_id.set_user_value(99, cx),
            Err(FormFieldError::ItemIdentityChanged)
        );
        assert_eq!(form.read(cx).value(), &before);
        assert_eq!(form.read(cx).revision(), revision);
    });
}

#[gpui::test]
fn identified_item_replacement_cannot_change_the_stable_id(cx: &mut TestAppContext) {
    let form = new_nested_form(cx);
    let item = NestedRootFormStore::rows_item(&form, FormItemId::new(41));

    cx.update(|cx| {
        let before = form.read(cx).value().clone();
        let revision = form.read(cx).revision();

        assert_eq!(
            item.set_user_value(
                NestedRow {
                    row_id: 99,
                    value: "replacement".into(),
                },
                cx,
            ),
            Err(FormFieldError::ItemIdentityChanged)
        );
        assert_eq!(form.read(cx).value(), &before);
        assert_eq!(form.read(cx).revision(), revision);
    });
}

#[gpui::test]
fn identified_item_rejects_an_unconvertible_stable_id(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            OptionalIdRootFormStore::from_value(
                OptionalIdRoot {
                    rows: vec![OptionalIdRow {
                        row_id: OptionalItemId(Some(41)),
                        value: "value".into(),
                    }],
                },
                cx,
            )
        })
    });
    let item = OptionalIdRootFormStore::rows_item(&form, FormItemId::new(41));
    let id_field = OptionalIdRowFormStore::row_id_in(item.clone());

    cx.update(|cx| {
        let before = form.read(cx).value().clone();
        let report = form.read(cx).validation_report();
        let revision = form.read(cx).revision();

        assert_eq!(
            id_field.set_user_value(OptionalItemId(None), cx),
            Err(FormFieldError::ItemIdentityChanged)
        );
        assert_eq!(
            item.set_user_value(
                OptionalIdRow {
                    row_id: OptionalItemId(None),
                    value: "replacement".into(),
                },
                cx,
            ),
            Err(FormFieldError::ItemIdentityChanged)
        );
        assert_eq!(form.read(cx).value(), &before);
        assert_eq!(form.read(cx).validation_report(), report);
        assert_eq!(form.read(cx).revision(), revision);
    });
}
