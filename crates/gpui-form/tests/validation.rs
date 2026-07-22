#![cfg(feature = "garde-adapter")]

use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::typed::{
    ErrorParamValue, FieldPath, FormStore as _, GardeAdapter, GardeMessageProvider, GardePathError,
    GardePathMapper, GardeRule, ValidationAdapter as _, ValidationContext, ValidationMessage,
    ValidationScope, ValidationSource, ValidationTrigger, garde_error,
};

struct SemanticMessageProvider;

impl GardeMessageProvider for SemanticMessageProvider {
    fn message(rule: GardeRule) -> ValidationMessage {
        match rule {
            GardeRule::LengthLowerThan { min } => ValidationMessage::key("validation-length-min")
                .with_param(
                    "min",
                    u64::try_from(min).expect("usize fits into u64 on supported platforms"),
                ),
            rule => {
                <gpui_form::typed::DefaultGardeMessageProvider as GardeMessageProvider>::message(
                    rule,
                )
            }
        }
    }
}

#[derive(garde::Validate)]
struct BuiltinRuleInput {
    #[garde(length(min = 3))]
    value: String,
}

impl GardePathMapper for BuiltinRuleInput {
    fn map_garde_path(&self, path: &str) -> Result<FieldPath, GardePathError> {
        match path {
            "value" => Ok(FieldPath::field("value")),
            _ => Err(GardePathError::UnknownField {
                path: path.to_owned(),
            }),
        }
    }
}

#[gpui::test]
fn garde_message_provider_preserves_key_and_params(cx: &mut TestAppContext) {
    let model = BuiltinRuleInput { value: "x".into() };

    cx.update(|cx| {
        let report = GardeAdapter::<BuiltinRuleInput, SemanticMessageProvider>::default().validate(
            &model,
            ValidationTrigger::Submit,
            ValidationScope::Form,
            ValidationContext { external: &() },
            cx,
        );

        assert_eq!(report.issues().len(), 1);
        assert_eq!(
            report.issues()[0].message,
            ValidationMessage::key("validation-length-min").with_param("min", 3u64)
        );
    });
}

#[derive(Clone, Copy)]
enum DirectMessage {
    Structured,
    ThirdParty,
    MalformedEnvelope,
}

struct DirectMessageInput {
    message: DirectMessage,
}

impl garde::Validate for DirectMessageInput {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        let error = match self.message {
            DirectMessage::Structured => garde_error(
                ValidationMessage::key("validation-custom")
                    .with_param("string", "value:\0with separators")
                    .with_param("integer", -7i64)
                    .with_param("unsigned", 9u64)
                    .with_param("float", 1.5f64)
                    .with_param("bool", true),
            ),
            DirectMessage::ThirdParty => garde::Error::new("third-party validation text"),
            DirectMessage::MalformedEnvelope => {
                garde::Error::new("\0gpui-form:garde-message:v1:not-hex")
            }
        };
        report.append(parent().join("value"), error);
    }
}

impl GardePathMapper for DirectMessageInput {
    fn map_garde_path(&self, path: &str) -> Result<FieldPath, GardePathError> {
        match path {
            "value" => Ok(FieldPath::field("value")),
            _ => Err(GardePathError::UnknownField {
                path: path.to_owned(),
            }),
        }
    }
}

fn direct_message_report(
    message: DirectMessage,
    cx: &gpui::App,
) -> gpui_form::typed::ValidationAdapterReport {
    GardeAdapter::<DirectMessageInput>::default().validate(
        &DirectMessageInput { message },
        ValidationTrigger::Submit,
        ValidationScope::Form,
        ValidationContext { external: &() },
        cx,
    )
}

#[gpui::test]
fn garde_custom_error_preserves_key_and_params(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let report = direct_message_report(DirectMessage::Structured, cx);
        assert_eq!(report.issues().len(), 1);
        assert_eq!(
            report.issues()[0].message,
            ValidationMessage::key("validation-custom")
                .with_param("string", "value:\0with separators")
                .with_param("integer", -7i64)
                .with_param("unsigned", 9u64)
                .with_param("float", 1.5f64)
                .with_param("bool", true)
        );
    });
}

#[gpui::test]
fn garde_unknown_text_remains_literal(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let report = direct_message_report(DirectMessage::ThirdParty, cx);
        assert_eq!(report.issues().len(), 1);
        assert_eq!(
            report.issues()[0].message,
            ValidationMessage::literal("third-party validation text")
        );
    });
}

#[gpui::test]
fn garde_malformed_internal_message_is_blocking(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let report = direct_message_report(DirectMessage::MalformedEnvelope, cx);
        assert_eq!(report.issues().len(), 1);
        let issue = &report.issues()[0];
        assert_eq!(issue.source, ValidationSource::Internal);
        assert_eq!(issue.code, "garde_message_envelope");
        assert!(issue.path.is_none());
        assert!(matches!(
            &issue.message,
            ValidationMessage::Key { key, params }
                if key == "gpui-form-error-internal"
                    && params.contains_key("path")
                    && params.contains_key("reason")
        ));
    });
}

static LOCALE_VALIDATION_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(validation(adapter = "garde"))]
struct LocaleRenderInput {
    #[form(validate(on_submit))]
    value: String,
}

impl garde::Validate for LocaleRenderInput {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        LOCALE_VALIDATION_CALLS.fetch_add(1, Ordering::SeqCst);
        report.append(
            parent().join("value"),
            garde_error(
                ValidationMessage::key("validation-locale").with_param("value", self.value.clone()),
            ),
        );
    }
}

fn render_validation_message(message: &ValidationMessage, locale: &str) -> String {
    match message {
        ValidationMessage::Key { key, params } if key == "validation-locale" => {
            let ErrorParamValue::String(value) = &params["value"] else {
                panic!("validation-locale value parameter must be a string");
            };
            match locale {
                "en-US" => format!("Invalid value: {value}"),
                "zh-CN" => format!("无效值：{value}"),
                _ => key.to_string(),
            }
        }
        ValidationMessage::Key { key, .. } => key.to_string(),
        ValidationMessage::Literal(message) => message.to_string(),
    }
}

#[gpui::test]
fn validation_messages_render_against_locale_without_revalidation(cx: &mut TestAppContext) {
    LOCALE_VALIDATION_CALLS.store(0, Ordering::SeqCst);
    let form = cx.update(|cx| {
        cx.new(|cx| {
            LocaleRenderInputFormStore::from_value(
                LocaleRenderInput {
                    value: "current".into(),
                },
                cx,
            )
        })
    });
    let value = LocaleRenderInputFormStore::value_field(&form);

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
        });
        value
            .start_async_validation(
                "pending",
                ValidationTrigger::Change,
                |_| std::future::pending::<Result<(), gpui_form::typed::AsyncValidationIssue>>(),
                cx,
            )
            .unwrap();

        let revision = form.read(cx).revision();
        let report = form.read(cx).validation_report();
        let calls = LOCALE_VALIDATION_CALLS.load(Ordering::SeqCst);
        assert!(form.read(cx).is_validating());

        assert_eq!(
            render_validation_message(&report.issues()[0].message, "en-US"),
            "Invalid value: current"
        );
        assert_eq!(
            render_validation_message(&report.issues()[0].message, "zh-CN"),
            "无效值：current"
        );

        assert_eq!(form.read(cx).revision(), revision);
        assert_eq!(form.read(cx).validation_report(), report);
        assert_eq!(LOCALE_VALIDATION_CALLS.load(Ordering::SeqCst), calls);
        assert!(form.read(cx).is_validating());
    });
}
