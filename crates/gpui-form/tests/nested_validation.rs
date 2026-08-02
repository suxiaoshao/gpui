use gpui::{App, AppContext as _, TestAppContext};
use gpui_form::{
    FieldAccessError, FormItemId, FormModel, FormState as _, ValidationAdapter,
    ValidationAdapterReport, ValidationIssue, ValidationMessage, ValidationScope, ValidationSource,
    ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
struct NestedChild {
    #[form(required, validate(on_change, on_submit))]
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
struct NestedRow {
    id: u64,
    #[form(required, validate(on_change, on_submit))]
    value: String,
}

struct NestedValidator;

impl ValidationAdapter<NestedRoot> for NestedValidator {
    type Context = ();

    fn validate(
        model: &NestedRoot,
        trigger: ValidationTrigger,
        _scope: &ValidationScope,
        _context: &Self::Context,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        if model.child.value == "reserved" {
            issues.push(ValidationIssue::field(
                NestedChildForm::VALUE.within(NestedRootForm::CHILD).path(),
                trigger,
                ValidationSource::App("nested".into()),
                "reserved",
                ValidationMessage::literal("reserved"),
            ));
        }
        for row in &model.rows {
            if row.value == "reserved" {
                issues.push(ValidationIssue::field(
                    NestedRowForm::VALUE
                        .within(NestedRootForm::ROWS.item(FormItemId::new(row.id)))
                        .path()
                        .clone(),
                    trigger,
                    ValidationSource::App("nested".into()),
                    "reserved",
                    ValidationMessage::literal("reserved"),
                ));
            }
        }
        ValidationAdapterReport::new(issues)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = NestedRootForm, validation(adapter = NestedValidator))]
struct NestedRoot {
    #[form(group)]
    child: NestedChild,
    #[form(array(id = "id"))]
    rows: Vec<NestedRow>,
}

fn nested_form(cx: &mut TestAppContext) -> gpui::Entity<NestedRootForm> {
    cx.update(|cx| {
        cx.new(|cx| {
            NestedRootForm::from_value_with_validation_context(
                NestedRoot {
                    child: NestedChild {
                        value: "reserved".into(),
                    },
                    rows: vec![
                        NestedRow {
                            id: 1,
                            value: "reserved".into(),
                        },
                        NestedRow {
                            id: 2,
                            value: String::new(),
                        },
                    ],
                },
                (),
                cx,
            )
        })
    })
}

#[gpui::test]
fn scoped_validation_replaces_only_intersecting_buckets(cx: &mut TestAppContext) {
    let form = nested_form(cx);
    let child = NestedChildForm::VALUE.within(NestedRootForm::CHILD);
    let first = NestedRowForm::VALUE.within(NestedRootForm::ROWS.item(FormItemId::new(1)));
    let second = NestedRowForm::VALUE.within(NestedRootForm::ROWS.item(FormItemId::new(2)));

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx)
        });
        assert_eq!(child.errors(&form, cx).len(), 1);
        assert_eq!(first.try_errors(&form, cx).unwrap().len(), 1);
        assert_eq!(second.try_errors(&form, cx).unwrap().len(), 1);

        child.set(&form, "available".into(), cx);
        assert!(child.errors(&form, cx).is_empty());
        assert_eq!(first.try_errors(&form, cx).unwrap().len(), 1);
        assert_eq!(second.try_errors(&form, cx).unwrap().len(), 1);
    });
}

#[gpui::test]
fn identified_items_report_missing_duplicate_and_identity_change(cx: &mut TestAppContext) {
    let form = nested_form(cx);
    let item = NestedRootForm::ROWS.item(FormItemId::new(1));

    cx.update(|cx| {
        let mut changed = item.try_value(&form, cx).unwrap();
        changed.id = 9;
        assert_eq!(
            item.try_set(&form, changed, cx),
            Err(gpui_form::FieldMutationError::ItemIdentityChanged)
        );

        let mut duplicate = form.read(cx).value().clone();
        duplicate.rows[1].id = 1;
        form.update(cx, |form, cx| form.replace(duplicate, cx));
        assert_eq!(
            item.try_value(&form, cx),
            Err(FieldAccessError::DuplicateItem(FormItemId::new(1)))
        );

        NestedRootForm::ROWS.set(&form, Vec::new(), cx);
        assert_eq!(
            item.try_value(&form, cx),
            Err(FieldAccessError::MissingItem(FormItemId::new(1)))
        );
    });
}

#[gpui::test]
fn projection_preserves_partial_availability(cx: &mut TestAppContext) {
    let form = nested_form(cx);
    let projected = NestedRootForm::ROWS.project_value(
        "first_value",
        |rows| rows.first().map(|row| row.value.clone()),
        |rows, value| {
            let Some(row) = rows.first_mut() else {
                return false;
            };
            row.value = value;
            true
        },
    );

    cx.update(|cx| {
        assert_eq!(projected.try_value(&form, cx).unwrap(), "reserved");
        NestedRootForm::ROWS.set(&form, Vec::new(), cx);
        assert_eq!(
            projected.try_value(&form, cx),
            Err(FieldAccessError::ValueUnavailable)
        );
    });
}
