use gpui::{App, AppContext as _, TestAppContext};
use gpui_form::typed::{
    SubmitTransform, TransformReport, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationScope, ValidationTrigger,
};

#[derive(Clone, Debug, Default)]
struct Context {
    suffix: String,
}

#[derive(Clone, Debug, Default)]
struct Validator;

impl ValidationAdapter<Input> for Validator {
    type Context = Context;

    fn validate(
        &self,
        _model: &Input,
        _trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::default()
    }
}

#[derive(Clone, Debug, Default)]
struct Transform;

impl SubmitTransform<Input> for Transform {
    type Output = Input;

    fn transform(&self, model: &Input) -> Result<Input, TransformReport> {
        Ok(Input {
            value: format!("{}-normalized", model.value),
        })
    }
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    validation(adapter = Validator, context = Context),
    transform(adapter = Transform)
)]
struct Input {
    #[form(validate(on_submit))]
    value: String,
}

#[gpui::test]
fn custom_validation_and_transform_use_the_same_model_version(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            InputFormStore::from_value_with_validation_context(
                Input {
                    value: "input".into(),
                },
                Context {
                    suffix: "-normalized".into(),
                },
                cx,
            )
        })
    });

    cx.update(|cx| {
        let output = form
            .update(cx, |form, cx| {
                assert_eq!(form.validation_context().suffix, "-normalized");
                gpui_form::typed::FormStore::prepare_submit(form, cx)
            })
            .unwrap();
        assert_eq!(output.value, "input-normalized");
    });
}
