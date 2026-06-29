use gpui_form::{
    FieldArrayStore, FieldChangeCause, FieldPath, FormField, FormItemId, FormItemIdGenerator,
    ValueFieldStore,
};

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
