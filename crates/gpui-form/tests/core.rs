use gpui_form::{
    FieldArrayStore, FieldChangeCause, FieldError, FieldMeta, FieldPath, FormField, FormItemId,
    FormItemIdGenerator, ValidationSeverity, ValidationSource, ValidationTrigger, ValueFieldStore,
};

fn field_error_with_severity(field: &'static str, severity: ValidationSeverity) -> FieldError {
    let mut error = FieldError::new_for_field(
        field,
        ValidationTrigger::Submit,
        ValidationSource::App("core-test".into()),
        "test",
        "core-test-error",
    );
    error.severity = severity;
    error
}

fn refresh_array_meta(array: &mut FieldArrayStore<&'static str>) {
    let values = array
        .items()
        .iter()
        .map(|item| item.item)
        .collect::<Vec<_>>();
    array.refresh_meta_from_values(values, Vec::<FieldMeta>::new());
}

#[test]
fn form_item_id_generator_uses_monotonic_u64_newtype() {
    let mut generator = FormItemIdGenerator::new();

    assert_eq!(generator.generate(), FormItemId::new(1));
    assert_eq!(generator.generate(), FormItemId::new(2));
    assert_eq!(generator.peek(), FormItemId::new(3));
}

#[test]
fn field_array_preserves_ids_on_reorder_and_rebuilds_on_reset() {
    let mut array = FieldArrayStore::new(FieldPath::from_static("headers"), ["a", "b", "c"]);
    let original_ids = array.ids();

    array.move_item(0, 2).unwrap();
    assert_eq!(
        array.ids(),
        vec![original_ids[1], original_ids[2], original_ids[0]]
    );

    let removed = array.remove(1).unwrap();
    assert_eq!(removed.id, original_ids[2]);
    assert_eq!(array.ids(), vec![original_ids[1], original_ids[0]]);

    let appended = array.append("d");
    assert_eq!(appended, FormItemId::new(4));

    array.reset(["x", "y"]);
    assert_eq!(array.ids(), vec![FormItemId::new(1), FormItemId::new(2)]);
}

#[test]
fn field_array_meta_tracks_structural_dirty_against_default_values() {
    let mut array = FieldArrayStore::new(FieldPath::from_static("headers"), ["a", "b"]);
    refresh_array_meta(&mut array);
    assert!(array.meta().is_pristine);
    assert!(array.meta().is_default_value);

    array.append("c");
    refresh_array_meta(&mut array);
    assert!(array.meta().is_dirty);
    assert!(array.meta().is_touched);
    assert!(!array.meta().is_default_value);

    let removed = array.remove(2).unwrap();
    assert_eq!(removed.item, "c");
    refresh_array_meta(&mut array);
    assert!(!array.meta().is_dirty);
    assert!(array.meta().is_pristine);
    assert!(array.meta().is_touched);
    assert!(array.meta().is_default_value);

    array.move_item(0, 1).unwrap();
    refresh_array_meta(&mut array);
    assert!(array.meta().is_dirty);
    assert!(!array.meta().is_default_value);

    array.move_item(1, 0).unwrap();
    refresh_array_meta(&mut array);
    assert!(!array.meta().is_dirty);
    assert!(array.meta().is_touched);
    assert!(array.meta().is_default_value);

    array.replace(Vec::<&'static str>::new());
    assert!(array.meta().is_dirty);
    assert!(array.meta().is_touched);
    assert!(!array.meta().is_default_value);

    array.replace(["a", "b"]);
    assert!(!array.meta().is_dirty);
    assert!(array.meta().is_touched);
    assert!(array.meta().is_default_value);

    array.reset(["x"]);
    assert!(!array.meta().is_dirty);
    assert!(!array.meta().is_touched);
    assert!(array.meta().is_default_value);
    assert_eq!(array.default_values(), &["x"]);
}

#[test]
fn field_path_supports_nested_indices_and_runtime_item_ids() {
    let path = FieldPath::from_static("servers")
        .join_index(2)
        .join_field("headers")
        .join_item(FormItemId::new(9))
        .join_field("value");

    assert_eq!(path.to_string(), "servers[2].headers[#9].value");
    assert_eq!(
        FieldPath::parse_lossy("servers[2].headers[9].value").to_string(),
        "servers[2].headers[9].value"
    );
}

#[test]
fn value_field_updates_meta_from_change_cause() {
    let mut field = ValueFieldStore::new("OpenAI".to_string());

    field.set_value(" Anthropic ".to_string(), FieldChangeCause::UserInput);

    assert_eq!(field.value(), " Anthropic ");
    assert!(field.meta().is_touched);
    assert!(field.meta().is_dirty);
    assert!(!field.meta().is_pristine);

    field.set_value("OpenAI".to_string(), FieldChangeCause::UserInput);
    assert!(!field.meta().is_dirty);
    assert!(field.meta().is_pristine);
}

#[test]
fn field_and_array_validity_only_tracks_error_severity() {
    let mut field = ValueFieldStore::new("OpenAI".to_string());
    field.set_errors(vec![
        field_error_with_severity("provider", ValidationSeverity::Warning),
        field_error_with_severity("provider", ValidationSeverity::Info),
    ]);
    assert_eq!(field.errors().len(), 2);
    assert!(field.meta().is_valid);

    field.set_errors(vec![field_error_with_severity(
        "provider",
        ValidationSeverity::Error,
    )]);
    assert!(!field.meta().is_valid);

    let mut array = FieldArrayStore::new(FieldPath::from_static("headers"), ["a"]);
    array.set_errors(vec![field_error_with_severity(
        "headers",
        ValidationSeverity::Warning,
    )]);
    refresh_array_meta(&mut array);
    assert!(array.meta().is_valid);

    array.set_errors(vec![field_error_with_severity(
        "headers",
        ValidationSeverity::Error,
    )]);
    refresh_array_meta(&mut array);
    assert!(!array.meta().is_valid);
}
