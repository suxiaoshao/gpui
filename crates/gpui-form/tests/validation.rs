#![cfg(feature = "garde-adapter")]

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{
    DefaultGardeMessageProvider, ErrorParamValue, FormModel, FormState as _, GardeAdapter,
    GardeMessageProvider, GardeRule, ValidationMessage, ValidationScope, ValidationTrigger,
    garde_error,
};

struct SemanticMessageProvider;

impl GardeMessageProvider for SemanticMessageProvider {
    fn message(rule: GardeRule) -> ValidationMessage {
        match rule {
            GardeRule::LengthLowerThan { min } => ValidationMessage::key("validation-length-min")
                .with_param("min", u64::try_from(min).expect("usize fits into u64")),
            rule => DefaultGardeMessageProvider::message(rule),
        }
    }
}

#[derive(Clone, Debug, PartialEq, FormModel, garde::Validate)]
#[form(
    state = ValidatedForm,
    validation(adapter = "garde", messages = SemanticMessageProvider)
)]
struct ValidatedInput {
    #[form(validate(on_submit))]
    #[garde(length(min = 3))]
    value: String,
}

#[gpui::test]
fn garde_policy_is_static_and_preserves_semantic_messages(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| ValidatedForm::from_value(ValidatedInput { value: "x".into() }, cx))
    });

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx)
        });
        let issues = ValidatedForm::VALUE.errors(&form, cx);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].message,
            ValidationMessage::key("validation-length-min").with_param("min", 3u64)
        );
    });
}

struct StructuredInput;

impl garde::Validate for StructuredInput {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        report.append(
            parent().join("value"),
            garde_error(
                ValidationMessage::key("structured")
                    .with_param("string", "value")
                    .with_param("integer", -7i64)
                    .with_param("unsigned", 9u64)
                    .with_param("float", 1.5f64)
                    .with_param("bool", true),
            ),
        );
    }
}

impl gpui_form::GardePathMapper for StructuredInput {
    fn map_garde_path(
        &self,
        path: &str,
    ) -> Result<gpui_form::FieldPath, gpui_form::GardePathError> {
        match path {
            "value" => Ok(gpui_form::FieldPath::field("value")),
            _ => Err(gpui_form::GardePathError::UnknownField {
                path: path.to_owned(),
            }),
        }
    }
}

#[gpui::test]
fn garde_envelope_round_trips_typed_parameters(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let report = <GardeAdapter<StructuredInput> as gpui_form::ValidationAdapter<
            StructuredInput,
        >>::validate(
            &StructuredInput,
            ValidationTrigger::Submit,
            &ValidationScope::Form,
            &(),
            cx,
        );
        let ValidationMessage::Key { params, .. } = &report.issues()[0].message else {
            panic!("expected structured message")
        };
        assert_eq!(params["integer"], ErrorParamValue::Integer(-7));
        assert_eq!(params["unsigned"], ErrorParamValue::Unsigned(9));
        assert_eq!(params["bool"], ErrorParamValue::Bool(true));
    });
}
