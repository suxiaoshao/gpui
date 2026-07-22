#![cfg(feature = "garde-adapter")]

use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicUsize, Ordering},
};

use gpui::{App, AppContext as _, TestAppContext};
use gpui_form::typed::{
    FieldPath, FormItemId, FormModelSchema as _, FormStore as _, GardeAdapter, SubmitError,
    SubmitTransform, ToFormItemId, TransformReport, ValidationAdapter as _, ValidationContext,
    ValidationIssue, ValidationReport, ValidationScope, ValidationSource, ValidationTrigger,
    normalize_adapter_report,
};

fn invalid_direct_row(value: &DirectRow, _context: &()) -> Result<(), garde::Error> {
    if value.invalid {
        Err(garde::Error::new("invalid direct row"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[garde(custom(invalid_direct_row))]
struct DirectRow {
    #[garde(skip)]
    row_id: u64,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
    #[garde(skip)]
    invalid: bool,
}

fn invalid_nested_row(value: &NestedRow, _context: &()) -> Result<(), garde::Error> {
    if value.invalid {
        Err(garde::Error::new("invalid nested row"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[garde(custom(invalid_nested_row))]
struct NestedRow {
    #[garde(skip)]
    row_id: u64,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
    #[garde(skip)]
    invalid: bool,
}

fn invalid_section(value: &Section, _context: &()) -> Result<(), garde::Error> {
    if value.invalid {
        Err(garde::Error::new("invalid section"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[garde(custom(invalid_section))]
struct Section {
    #[garde(skip)]
    section_id: u64,
    #[form(array(id = "row_id"), validate(on_submit))]
    #[garde(length(max = 0), dive)]
    rows: Vec<NestedRow>,
    #[garde(skip)]
    invalid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
struct Catalog {
    #[form(array(id = "section_id"), validate(on_submit))]
    #[garde(length(max = 0), dive)]
    sections: Vec<Section>,
}

static TRANSFORM_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default)]
struct CountingTransform;

impl SubmitTransform<CorrectiveRoot> for CountingTransform {
    type Output = CorrectiveRoot;

    fn transform(&self, model: &CorrectiveRoot) -> Result<Self::Output, TransformReport> {
        TRANSFORM_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(model.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[form(
    validation(adapter = "garde"),
    transform(adapter = CountingTransform)
)]
struct CorrectiveRoot {
    #[form(array(id = "row_id"), validate(on_submit))]
    #[garde(length(max = 0), dive)]
    rows: Vec<DirectRow>,
    #[form(group)]
    #[garde(dive)]
    catalog: Catalog,
}

fn corrective_root() -> CorrectiveRoot {
    CorrectiveRoot {
        // Both arrays intentionally start in a non-ID order. The invalid logical item is not at
        // the same index as its stable ID, so an index-path fallback cannot satisfy the checks.
        rows: vec![
            DirectRow {
                row_id: 2,
                label: "valid".into(),
                invalid: false,
            },
            DirectRow {
                row_id: 1,
                label: String::new(),
                invalid: true,
            },
        ],
        catalog: Catalog {
            sections: vec![
                Section {
                    section_id: 20,
                    rows: vec![
                        NestedRow {
                            row_id: 201,
                            label: "valid".into(),
                            invalid: false,
                        },
                        NestedRow {
                            row_id: 200,
                            label: String::new(),
                            invalid: true,
                        },
                    ],
                    invalid: true,
                },
                Section {
                    section_id: 10,
                    rows: vec![NestedRow {
                        row_id: 100,
                        label: "valid".into(),
                        invalid: false,
                    }],
                    invalid: false,
                },
            ],
        },
    }
}

fn normalized_submit_issues(model: &CorrectiveRoot, cx: &App) -> Vec<ValidationIssue> {
    let report = GardeAdapter::<CorrectiveRoot>::default().validate(
        model,
        ValidationTrigger::Submit,
        ValidationScope::Form,
        ValidationContext { external: &() },
        cx,
    );
    normalize_adapter_report(
        model,
        ValidationTrigger::Submit,
        &ValidationScope::Form,
        report,
    )
}

fn issue_paths(issues: &[ValidationIssue]) -> BTreeSet<FieldPath> {
    issues
        .iter()
        .map(|issue| {
            assert_eq!(issue.source, ValidationSource::Garde);
            issue
                .path
                .clone()
                .expect("all fixture failures are field-scoped Garde rules")
        })
        .collect()
}

#[gpui::test]
fn real_garde_report_normalizes_every_array_path_shape_after_nested_reorder(
    cx: &mut TestAppContext,
) {
    let original = corrective_root();
    let mut reordered = original.clone();
    reordered.rows.reverse();
    reordered.catalog.sections.reverse();
    for section in &mut reordered.catalog.sections {
        section.rows.reverse();
    }

    cx.update(|cx| {
        let original_paths = issue_paths(&normalized_submit_issues(&original, cx));
        let reordered_paths = issue_paths(&normalized_submit_issues(&reordered, cx));

        // Reordering both the outer and inner vectors changes every Garde index, but not the
        // normalized stable paths.
        assert_eq!(reordered_paths, original_paths);

        let expected = [
            // Root array: container, direct item root, and item leaf.
            FieldPath::field("rows"),
            FieldPath::field("rows").join_item(FormItemId::new(1)),
            FieldPath::field("rows")
                .join_item(FormItemId::new(1))
                .join_field("label"),
            // group -> outer array: container and direct item root.
            FieldPath::field("catalog").join_field("sections"),
            FieldPath::field("catalog")
                .join_field("sections")
                .join_item(FormItemId::new(20)),
            // group -> outer array -> inner array: nested container, item root, and leaf.
            FieldPath::field("catalog")
                .join_field("sections")
                .join_item(FormItemId::new(20))
                .join_field("rows"),
            FieldPath::field("catalog")
                .join_field("sections")
                .join_item(FormItemId::new(20))
                .join_field("rows")
                .join_item(FormItemId::new(200)),
            FieldPath::field("catalog")
                .join_field("sections")
                .join_item(FormItemId::new(20))
                .join_field("rows")
                .join_item(FormItemId::new(200))
                .join_field("label"),
        ];
        for path in expected {
            assert!(
                original_paths.contains(&path),
                "missing normalized path {path}"
            );
        }
    });
}

#[gpui::test]
fn real_nested_submit_error_blocks_transform(cx: &mut TestAppContext) {
    TRANSFORM_CALLS.store(0, Ordering::SeqCst);
    let form = cx.update(|cx| {
        cx.new(|cx| {
            CorrectiveRootFormStore::from_value_with_validation_context(corrective_root(), (), cx)
        })
    });

    cx.update(|cx| {
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        assert_eq!(TRANSFORM_CALLS.load(Ordering::SeqCst), 0);
        assert!(
            form.read(cx)
                .validation_report()
                .issues()
                .iter()
                .any(|issue| {
                    issue.source == ValidationSource::Garde
                        && issue.trigger == ValidationTrigger::Submit
                        && issue.path.as_ref()
                            == Some(
                                &FieldPath::field("catalog")
                                    .join_field("sections")
                                    .join_item(FormItemId::new(20))
                                    .join_field("rows")
                                    .join_item(FormItemId::new(200))
                                    .join_field("label"),
                            )
                })
        );
    });
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OptionalId(Option<u64>);

impl ToFormItemId for OptionalId {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        self.0.map(FormItemId::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
struct UnmappableRow {
    #[garde(skip)]
    row_id: OptionalId,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate)]
#[form(validation(adapter = "garde"))]
struct UnmappableRoot {
    #[garde(skip)]
    safe: String,
    #[form(array(id = "row_id"), validate(on_submit))]
    #[garde(dive)]
    rows: Vec<UnmappableRow>,
}

#[gpui::test]
fn garde_mapping_failure_stays_blocking_outside_field_scope(cx: &mut TestAppContext) {
    let model = UnmappableRoot {
        safe: "valid".into(),
        rows: vec![UnmappableRow {
            row_id: OptionalId(None),
            label: String::new(),
        }],
    };
    let scope = ValidationScope::Field(FieldPath::field("safe"));

    cx.update(|cx| {
        let report = GardeAdapter::<UnmappableRoot>::default().validate(
            &model,
            ValidationTrigger::Submit,
            scope.clone(),
            ValidationContext { external: &() },
            cx,
        );
        let issues = normalize_adapter_report(&model, ValidationTrigger::Submit, &scope, report);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].source, ValidationSource::Internal);
        assert_eq!(issues[0].code, "garde_path_mapping");
        assert!(issues[0].path.is_none());
        assert!(!ValidationReport::new(issues).is_valid());
    });
}

#[derive(Clone, PartialEq, gpui_form::FormStore, garde::Validate)]
struct GenericGroup<T>
where
    T: Clone + PartialEq + 'static,
{
    #[garde(skip)]
    value: T,
}

#[derive(Clone, PartialEq, gpui_form::FormStore, garde::Validate)]
struct GenericRow<T>
where
    T: Clone + PartialEq + 'static,
{
    #[garde(skip)]
    row_id: u64,
    #[garde(skip)]
    value: T,
}

#[derive(Clone, PartialEq, gpui_form::FormStore, garde::Validate)]
struct GenericRoot<T>
where
    T: Clone + PartialEq + 'static,
{
    #[form(group)]
    #[garde(dive)]
    group: GenericGroup<T>,
    #[form(array(id = "row_id"))]
    #[garde(dive)]
    rows: Vec<GenericRow<T>>,
}

#[test]
fn generic_group_and_nominal_generic_row_derive_and_resolve_schema() {
    let model = GenericRoot {
        group: GenericGroup { value: 7u32 },
        rows: vec![GenericRow {
            row_id: 41,
            value: 9u32,
        }],
    };

    assert_eq!(
        model
            .schema_at_path(FieldPath::field("group").join_field("value").segments(),)
            .unwrap()
            .name(),
        "value"
    );
    assert_eq!(
        model
            .schema_at_path(
                FieldPath::field("rows")
                    .join_item(FormItemId::new(41))
                    .join_field("value")
                    .segments(),
            )
            .unwrap()
            .name(),
        "value"
    );
}
