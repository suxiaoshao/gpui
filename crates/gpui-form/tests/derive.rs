use gpui::{AppContext as _, Entity, TestAppContext};
use gpui_form::{
    FieldAccessError, FieldPath, FormItemId, FormModel, FormRevision, FormState as _,
    ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = ProfileForm)]
struct ProfileInput {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,
    enabled: bool,
    port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = GenericForm)]
struct GenericInput<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
struct ChildInput {
    #[form(required)]
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
struct RowInput {
    row_id: u64,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = ParentForm)]
struct ParentInput {
    #[form(group)]
    child: ChildInput,
    #[form(array(id = "row_id"))]
    rows: Vec<RowInput>,
}

fn profile(cx: &mut TestAppContext) -> Entity<ProfileForm> {
    cx.update(|cx| {
        cx.new(|cx| {
            ProfileForm::from_value(
                ProfileInput {
                    name: "OpenAI".into(),
                    enabled: true,
                    port: 443,
                },
                cx,
            )
        })
    })
}

#[gpui::test]
fn derive_generates_named_state_and_static_total_descriptors(cx: &mut TestAppContext) {
    let form = profile(cx);

    assert_eq!(ProfileForm::NAME.path(), FieldPath::field("name"));
    assert!(ProfileForm::NAME.schema().is_required());
    assert_eq!(ProfileForm::PORT.schema().name(), "port");

    cx.update(|cx| {
        assert_eq!(ProfileForm::NAME.value(&form, cx), "OpenAI");
        assert_eq!(ProfileForm::PORT.value(&form, cx), 443);
        ProfileForm::NAME.set(&form, "Anthropic".into(), cx);
        ProfileForm::PORT.set(&form, 8443, cx);
        assert_eq!(form.read(cx).value().name, "Anthropic");
        assert_eq!(form.read(cx).value().port, 8443);
        assert!(form.read(cx).is_dirty());
    });
}

#[gpui::test]
fn derive_preserves_generics_and_custom_state_names(cx: &mut TestAppContext) {
    let form =
        cx.update(|cx| cx.new(|cx| GenericForm::from_value(GenericInput { value: 7u32 }, cx)));

    cx.update(|cx| {
        assert_eq!(GenericForm::VALUE.value(&form, cx), 7);
        GenericForm::VALUE.set(&form, 9, cx);
        assert_eq!(form.read(cx).value().value, 9);
    });
}

#[gpui::test]
fn total_and_partial_composition_preserve_availability(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ParentForm::from_value(
                ParentInput {
                    child: ChildInput {
                        value: "nested".into(),
                    },
                    rows: vec![RowInput {
                        row_id: 41,
                        value: "row".into(),
                    }],
                },
                cx,
            )
        })
    });

    let child_value = ChildInputForm::VALUE.within(ParentForm::CHILD);
    let row = ParentForm::ROWS.item(FormItemId::new(41));
    let row_value = RowInputForm::VALUE.within(row.clone());

    cx.update(|cx| {
        assert_eq!(child_value.value(&form, cx), "nested");
        child_value.set(&form, "changed".into(), cx);
        assert_eq!(row_value.try_value(&form, cx).unwrap(), "row");
        row_value.try_set(&form, "updated".into(), cx).unwrap();
        assert_eq!(form.read(cx).value().rows[0].value, "updated");

        ParentForm::ROWS.set(&form, Vec::new(), cx);
        assert_eq!(
            row_value.try_value(&form, cx),
            Err(FieldAccessError::MissingItem(FormItemId::new(41)))
        );
    });
}

#[gpui::test]
fn replacement_and_revision_contract_is_atomic(cx: &mut TestAppContext) {
    let form = profile(cx);
    let initial = form.read_with(cx, |form, _| form.value().clone());

    cx.update(|cx| {
        assert_eq!(form.read(cx).revision(), FormRevision::INITIAL);
        ProfileForm::NAME.set(&form, "OpenAI".into(), cx);
        assert_eq!(form.read(cx).revision(), FormRevision::INITIAL);

        form.update(cx, |form, cx| form.replace(initial.clone(), cx));
        assert_eq!(form.read(cx).revision().get(), 1);
        form.update(cx, |form, cx| form.reset(cx));
        assert_eq!(form.read(cx).revision().get(), 2);
        form.update(cx, |form, cx| form.rebase(initial.clone(), cx));
        assert_eq!(form.read(cx).revision().get(), 3);

        let before = form.read(cx).value().clone();
        assert!(!form.update(cx, |form, cx| {
            form.rebase_if_revision(
                FormRevision::INITIAL,
                ProfileInput {
                    name: "stale".into(),
                    enabled: false,
                    port: 80,
                },
                cx,
            )
        }));
        assert_eq!(form.read(cx).value(), &before);
        assert_eq!(form.read(cx).revision().get(), 3);
    });
}

#[gpui::test]
fn total_validation_has_no_structural_result(cx: &mut TestAppContext) {
    let form = profile(cx);
    cx.update(|cx| {
        ProfileForm::NAME.set(&form, String::new(), cx);
        ProfileForm::NAME.validate(&form, ValidationTrigger::Blur, cx);
        assert!(!ProfileForm::NAME.errors(&form, cx).is_empty());
    });
}
