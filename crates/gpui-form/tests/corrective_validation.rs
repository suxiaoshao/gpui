use gpui_form::typed::{
    ErrorParamValue, FieldPath, FieldPathSegment, FormItemId, FormModelSchema as _,
    FormSchemaPathError, StructuralValidate as _, ToFormItemId, ValidationAdapterReport,
    ValidationIssue, ValidationMessage, ValidationReport, ValidationScope, ValidationSource,
    ValidationTrigger, normalize_adapter_report,
};

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct SchemaChild {
    #[form(validate(on_submit))]
    leaf: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct SchemaRow {
    row_id: u64,
    #[form(validate(on_change))]
    leaf: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct SchemaRoot {
    plain: String,
    #[form(group, validate(on_change))]
    child: SchemaChild,
    #[form(array(id = "row_id"), validate(on_change))]
    rows: Vec<SchemaRow>,
}

fn schema_root() -> SchemaRoot {
    SchemaRoot {
        plain: "plain".into(),
        child: SchemaChild {
            leaf: "child".into(),
        },
        rows: vec![
            SchemaRow {
                row_id: 1,
                leaf: "first".into(),
            },
            SchemaRow {
                row_id: 2,
                leaf: "duplicate-a".into(),
            },
            SchemaRow {
                row_id: 2,
                leaf: "duplicate-b".into(),
            },
        ],
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
        ValidationSource::App("corrective-validation".into()),
        code,
        ValidationMessage::key(code),
    )
}

fn assert_resolution_failure_is_one_blocking_internal(
    model: &SchemaRoot,
    path: FieldPath,
    expected: FormSchemaPathError,
) {
    assert_eq!(model.schema_at_path(path.segments()), Err(expected));

    let original_code = "original_adapter_issue";
    let issues = normalize_adapter_report(
        model,
        ValidationTrigger::Dynamic,
        &ValidationScope::Form,
        ValidationAdapterReport::new(vec![adapter_issue(
            path.clone(),
            ValidationTrigger::Dynamic,
            original_code,
        )]),
    );

    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert_eq!(issue.source, ValidationSource::Internal);
    assert_eq!(issue.code, "form_schema_path_resolution");
    assert_eq!(issue.path, None);
    assert_eq!(issue.trigger, ValidationTrigger::Dynamic);
    assert_ne!(issue.code, original_code);
    assert!(!ValidationReport::new(issues.clone()).is_valid());

    let ValidationMessage::Key { key, params } = &issue.message else {
        panic!("schema resolution failures must use the internal message key");
    };
    assert_eq!(key, "gpui-form-error-internal");
    assert_eq!(
        params.get("path"),
        Some(&ErrorParamValue::String(path.to_string().into()))
    );
    assert_eq!(
        params.get("reason"),
        Some(&ErrorParamValue::String(expected.to_string().into()))
    );
}

#[test]
fn every_schema_resolver_error_normalizes_to_one_blocking_internal_issue() {
    let model = schema_root();

    let cases = [
        (FieldPath::root(), FormSchemaPathError::EmptyPath),
        (
            FieldPath::field("unknown"),
            FormSchemaPathError::UnknownField,
        ),
        (
            FieldPath::from_segments([FieldPathSegment::Item(FormItemId::new(1))]),
            FormSchemaPathError::UnexpectedItem,
        ),
        (
            FieldPath::field("rows").join_item(FormItemId::new(99)),
            FormSchemaPathError::MissingItem(FormItemId::new(99)),
        ),
        (
            FieldPath::field("rows").join_item(FormItemId::new(2)),
            FormSchemaPathError::DuplicateItem(FormItemId::new(2)),
        ),
        (
            FieldPath::field("child").join_projection("computed"),
            FormSchemaPathError::Projection,
        ),
        (
            FieldPath::field("plain").join_field("trailing"),
            FormSchemaPathError::TrailingSegments,
        ),
    ];

    for (path, expected) in cases {
        assert_resolution_failure_is_one_blocking_internal(&model, path, expected);
    }
}

#[test]
fn invalid_adapter_path_is_retained_before_out_of_scope_filtering() {
    let model = schema_root();
    let invalid_path = FieldPath::field("unknown").join_field("leaf");
    let issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Change,
        &ValidationScope::Field(FieldPath::field("child").join_field("leaf")),
        ValidationAdapterReport::new(vec![adapter_issue(
            invalid_path,
            ValidationTrigger::Change,
            "out_of_scope_original",
        )]),
    );

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].source, ValidationSource::Internal);
    assert_eq!(issues[0].code, "form_schema_path_resolution");
    assert!(issues[0].path.is_none());
}

#[test]
fn descendant_issue_uses_its_exact_schema_instead_of_parent_triggers() {
    let model = schema_root();
    let parent_path = FieldPath::field("child");
    let leaf_path = parent_path.join_field("leaf");
    let change_issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Change,
        &ValidationScope::Form,
        ValidationAdapterReport::new(vec![
            adapter_issue(
                parent_path.clone(),
                ValidationTrigger::Change,
                "parent_change",
            ),
            adapter_issue(leaf_path.clone(), ValidationTrigger::Change, "leaf_change"),
        ]),
    );

    assert_eq!(change_issues.len(), 1);
    assert_eq!(change_issues[0].code, "parent_change");
    assert_eq!(change_issues[0].path.as_ref(), Some(&parent_path));

    let submit_issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Submit,
        &ValidationScope::Form,
        ValidationAdapterReport::new(vec![adapter_issue(
            leaf_path.clone(),
            ValidationTrigger::Submit,
            "leaf_submit",
        )]),
    );
    assert_eq!(submit_issues.len(), 1);
    assert_eq!(submit_issues[0].code, "leaf_submit");
    assert_eq!(submit_issues[0].path.as_ref(), Some(&leaf_path));
}

#[test]
fn item_leaf_scope_includes_own_ancestors_and_excludes_sibling_item() {
    let model = SchemaRoot {
        rows: vec![
            SchemaRow {
                row_id: 1,
                leaf: "first".into(),
            },
            SchemaRow {
                row_id: 2,
                leaf: "second".into(),
            },
        ],
        ..schema_root()
    };
    let array_path = FieldPath::field("rows");
    let own_item_path = array_path.join_item(FormItemId::new(1));
    let own_leaf_path = own_item_path.join_field("leaf");
    let sibling_item_path = array_path.join_item(FormItemId::new(2));
    let sibling_leaf_path = sibling_item_path.join_field("leaf");

    let issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Change,
        &ValidationScope::Field(own_leaf_path.clone()),
        ValidationAdapterReport::new(vec![
            adapter_issue(array_path.clone(), ValidationTrigger::Change, "array"),
            adapter_issue(own_item_path.clone(), ValidationTrigger::Change, "own_item"),
            adapter_issue(own_leaf_path.clone(), ValidationTrigger::Change, "own_leaf"),
            adapter_issue(sibling_item_path, ValidationTrigger::Change, "sibling_item"),
            adapter_issue(sibling_leaf_path, ValidationTrigger::Change, "sibling_leaf"),
        ]),
    );

    assert_eq!(
        issues
            .iter()
            .map(|issue| issue.code.as_ref())
            .collect::<Vec<_>>(),
        ["array", "own_item", "own_leaf"]
    );
    assert_eq!(issues[0].path.as_ref(), Some(&array_path));
    assert_eq!(issues[1].path.as_ref(), Some(&own_item_path));
    assert_eq!(issues[2].path.as_ref(), Some(&own_leaf_path));
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct NestedRow {
    row_id: u64,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct Section {
    section_id: u64,
    #[form(array(id = "row_id"), validate(on_submit))]
    rows: Vec<NestedRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct NestedRoot {
    #[form(array(id = "section_id"), validate(on_change))]
    sections: Vec<Section>,
}

fn nested_root() -> NestedRoot {
    NestedRoot {
        sections: vec![Section {
            section_id: 10,
            rows: vec![NestedRow {
                row_id: 20,
                value: "nested".into(),
            }],
        }],
    }
}

#[test]
fn nested_item_root_uses_the_nearest_array_schema() {
    let model = nested_root();
    let outer_item_path = FieldPath::field("sections").join_item(FormItemId::new(10));
    let nested_item_path = outer_item_path
        .join_field("rows")
        .join_item(FormItemId::new(20));

    assert_eq!(
        model
            .schema_at_path(outer_item_path.segments())
            .unwrap()
            .name(),
        "sections"
    );
    assert_eq!(
        model
            .schema_at_path(nested_item_path.segments())
            .unwrap()
            .name(),
        "rows"
    );

    let submit_issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Submit,
        &ValidationScope::Form,
        ValidationAdapterReport::new(vec![
            adapter_issue(
                outer_item_path.clone(),
                ValidationTrigger::Submit,
                "outer_submit",
            ),
            adapter_issue(
                nested_item_path.clone(),
                ValidationTrigger::Submit,
                "nested_submit",
            ),
        ]),
    );
    assert_eq!(submit_issues.len(), 1);
    assert_eq!(submit_issues[0].code, "nested_submit");
    assert_eq!(submit_issues[0].path.as_ref(), Some(&nested_item_path));

    let change_issues = normalize_adapter_report(
        &model,
        ValidationTrigger::Change,
        &ValidationScope::Form,
        ValidationAdapterReport::new(vec![
            adapter_issue(
                outer_item_path.clone(),
                ValidationTrigger::Change,
                "outer_change",
            ),
            adapter_issue(nested_item_path, ValidationTrigger::Change, "nested_change"),
        ]),
    );
    assert_eq!(change_issues.len(), 1);
    assert_eq!(change_issues[0].code, "outer_change");
    assert_eq!(change_issues[0].path.as_ref(), Some(&outer_item_path));
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OptionalId(Option<u64>);

impl ToFormItemId for OptionalId {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        self.0.map(FormItemId::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct StructuralRow {
    item_id: OptionalId,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct StructuralGroup {
    #[form(array(id = "item_id"))]
    rows: Vec<StructuralRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct StructuralSection {
    section_id: u64,
    #[form(array(id = "item_id"))]
    rows: Vec<StructuralRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct StructuralRoot {
    #[form(group)]
    settings: StructuralGroup,
    #[form(array(id = "section_id"))]
    sections: Vec<StructuralSection>,
}

fn structural_rows(duplicate: u64) -> Vec<StructuralRow> {
    vec![
        StructuralRow {
            item_id: OptionalId(Some(duplicate)),
            value: "duplicate-a".into(),
        },
        StructuralRow {
            item_id: OptionalId(Some(duplicate)),
            value: "duplicate-b".into(),
        },
        StructuralRow {
            item_id: OptionalId(None),
            value: "invalid".into(),
        },
    ]
}

#[test]
fn nested_structural_id_issues_use_the_exact_owning_array_paths() {
    let model = StructuralRoot {
        settings: StructuralGroup {
            rows: structural_rows(1),
        },
        sections: vec![
            StructuralSection {
                section_id: 10,
                rows: structural_rows(2),
            },
            StructuralSection {
                section_id: 20,
                rows: vec![StructuralRow {
                    item_id: OptionalId(Some(3)),
                    value: "valid-sibling".into(),
                }],
            },
        ],
    };
    let mut issues = Vec::new();
    model.structural_issues(
        &FieldPath::root(),
        ValidationTrigger::Submit,
        &ValidationScope::Form,
        &mut issues,
    );

    assert_eq!(issues.len(), 4);
    assert!(issues.iter().all(|issue| {
        issue.source == ValidationSource::Internal && issue.trigger == ValidationTrigger::Submit
    }));
    let settings_rows = FieldPath::field("settings").join_field("rows");
    let nested_rows = FieldPath::field("sections")
        .join_item(FormItemId::new(10))
        .join_field("rows");
    let expected = [
        ("invalid_item_id", settings_rows.clone()),
        (
            "duplicate_item_id",
            settings_rows.join_item(FormItemId::new(1)),
        ),
        ("invalid_item_id", nested_rows.clone()),
        (
            "duplicate_item_id",
            nested_rows.join_item(FormItemId::new(2)),
        ),
    ];

    for (code, path) in expected {
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == code && issue.path.as_ref() == Some(&path))
                .count(),
            1,
            "missing or duplicated structural issue {code} at {path}"
        );
    }
    assert!(!ValidationReport::new(issues).is_valid());
}
