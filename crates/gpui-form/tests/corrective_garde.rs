#![cfg(feature = "garde-adapter")]

use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{
    FieldPath, FormItemId, FormModel, FormState as _, GardeAdapter, SubmitError, SubmitTransform,
    ToFormItemId, ValidationReport, ValidationScope, ValidationSource, ValidationTrigger,
    normalize_adapter_report,
};

#[derive(Clone, Debug, PartialEq, Eq, FormModel, garde::Validate)]
struct Row {
    #[garde(skip)]
    id: u64,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
}

static TRANSFORM_CALLS: AtomicUsize = AtomicUsize::new(0);

struct CountingTransform;

impl SubmitTransform<Root> for CountingTransform {
    type Output = Root;

    fn transform(model: &Root) -> Self::Output {
        TRANSFORM_CALLS.fetch_add(1, Ordering::SeqCst);
        model.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel, garde::Validate)]
#[form(
    state = RootForm,
    validation(adapter = "garde"),
    transform(adapter = CountingTransform)
)]
struct Root {
    #[form(array(id = "id"), validate(on_submit))]
    #[garde(dive)]
    rows: Vec<Row>,
}

#[gpui::test]
fn nested_garde_paths_use_stable_item_identity_and_block_transform(cx: &mut TestAppContext) {
    TRANSFORM_CALLS.store(0, Ordering::SeqCst);
    let form = cx.update(|cx| {
        cx.new(|cx| {
            RootForm::from_value_with_validation_context(
                Root {
                    rows: vec![
                        Row {
                            id: 2,
                            label: "valid".into(),
                        },
                        Row {
                            id: 1,
                            label: String::new(),
                        },
                    ],
                },
                (),
                cx,
            )
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
                        && issue.path.as_ref()
                            == Some(
                                &FieldPath::field("rows")
                                    .join_item(FormItemId::new(1))
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

#[derive(Clone, Debug, PartialEq, Eq, FormModel, garde::Validate)]
struct UnmappableRow {
    #[garde(skip)]
    id: OptionalId,
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel, garde::Validate)]
struct UnmappableRoot {
    #[garde(skip)]
    safe: String,
    #[form(array(id = "id"), validate(on_submit))]
    #[garde(dive)]
    rows: Vec<UnmappableRow>,
}

#[gpui::test]
fn garde_mapping_failure_remains_blocking_before_scope_filtering(cx: &mut TestAppContext) {
    let model = UnmappableRoot {
        safe: "safe".into(),
        rows: vec![UnmappableRow {
            id: OptionalId(None),
            label: String::new(),
        }],
    };
    let scope = ValidationScope::Field(FieldPath::field("safe"));

    cx.update(|cx| {
        let report = <GardeAdapter<UnmappableRoot> as gpui_form::ValidationAdapter<
            UnmappableRoot,
        >>::validate(&model, ValidationTrigger::Submit, &scope, &(), cx);
        let issues = normalize_adapter_report(&model, ValidationTrigger::Submit, &scope, report);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].source, ValidationSource::Internal);
        assert_eq!(issues[0].code, "garde_path_mapping");
        assert!(!ValidationReport::new(issues).is_valid());
    });
}
