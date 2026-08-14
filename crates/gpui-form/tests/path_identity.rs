use gpui::{AppContext as _, TestAppContext};
use gpui_form::{Form, FormSchema};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Row {
    value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Model {
    #[form(items)]
    rows: Vec<Row>,
}

fn model() -> Model {
    Model {
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

#[gpui::test]
fn path_keys_are_opaque_session_local_and_reused_for_one_active_address(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let first_form = cx.new(|_| Form::new(model()));
        let second_form = cx.new(|_| Form::new(model()));
        let rows = Model::ROOT.then(Model::ROWS);

        let first = rows.items(&first_form, cx).remove(0).key();
        let repeated = rows.items(&first_form, cx).remove(0).key();
        let other_session = rows.items(&second_form, cx).remove(0).key();

        assert_eq!(first, repeated);
        assert_ne!(first, other_session);
        assert!(!format!("{first:?}").contains("rows"));

        let element: gpui::ElementId = first.into();
        assert!(!format!("{element:?}").contains("rows"));
    });
}

#[gpui::test]
fn item_occurrence_survives_same_parent_reorder_but_not_remove_reinsert(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::new(model()));
        let rows = Model::ROOT.then(Model::ROWS);
        let items = rows.items(&form, cx);
        let first = items[0].clone();
        let second = items[1].clone();
        let first_key = first.key();

        rows.move_before(&form, &second, &first, cx).unwrap();
        assert_eq!(first.key(), first_key);

        let removed = rows.remove(&form, first, cx).unwrap();
        let reinserted = rows.append(&form, removed, cx).unwrap();
        assert_ne!(reinserted.key(), first_key);
    });
}
