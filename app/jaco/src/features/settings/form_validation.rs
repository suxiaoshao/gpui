use fluent_bundle::FluentArgs;
use gpui::{App, SharedString};
use gpui_form::typed::{
    ErrorParamValue, GardeMessageProvider, GardeRule, ValidationMessage, garde_error,
};

use crate::foundation::I18n;

pub(crate) fn validation_message(message: &ValidationMessage, cx: &App) -> SharedString {
    validation_message_with_i18n(message, cx.global::<I18n>())
}

fn validation_message_with_i18n(message: &ValidationMessage, i18n: &I18n) -> SharedString {
    match message {
        ValidationMessage::Literal(message) => message.to_string().into(),
        ValidationMessage::Key { key, params } => {
            let mut args = FluentArgs::new();
            for (name, value) in params {
                let value = match value {
                    ErrorParamValue::String(value)
                        if name.as_ref() == "field"
                            && value.as_ref().starts_with("provider-field-") =>
                    {
                        i18n.t(value.as_ref())
                    }
                    ErrorParamValue::String(value) => value.to_string(),
                    ErrorParamValue::Integer(value) => value.to_string(),
                    ErrorParamValue::Unsigned(value) => value.to_string(),
                    ErrorParamValue::Float(value) => value.to_string(),
                    ErrorParamValue::Bool(value) => value.to_string(),
                };
                args.set(name.as_ref(), value);
            }
            i18n.t_with_args(key.as_ref(), &args).into()
        }
    }
}

#[derive(Clone)]
pub(crate) struct JacoValidationContext<D> {
    pub(crate) dependencies: D,
}

impl<D> JacoValidationContext<D> {
    pub(crate) fn new(dependencies: D) -> Self {
        Self { dependencies }
    }
}

pub(crate) struct JacoGardeMessageProvider;

impl GardeMessageProvider for JacoGardeMessageProvider {
    fn message(rule: GardeRule) -> ValidationMessage {
        match rule {
            GardeRule::LengthLowerThan { min } => {
                ValidationMessage::key("validation-garde-length-min").with_param("min", min as u64)
            }
            GardeRule::LengthGreaterThan { max } => {
                ValidationMessage::key("validation-garde-length-max").with_param("max", max as u64)
            }
            GardeRule::RangeLowerThan { min } => {
                ValidationMessage::key("validation-garde-range-min").with_param("min", min)
            }
            GardeRule::RangeGreaterThan { max } => {
                ValidationMessage::key("validation-garde-range-max").with_param("max", max)
            }
            GardeRule::CreditCardInvalid { reason } => {
                ValidationMessage::key("validation-garde-credit-card")
                    .with_param("reason", reason.to_string())
            }
            GardeRule::PatternNoMatch { pattern } => {
                ValidationMessage::key("validation-garde-pattern").with_param("pattern", pattern)
            }
            GardeRule::ContainsMissing { pattern } => {
                ValidationMessage::key("validation-garde-contains").with_param("pattern", pattern)
            }
            GardeRule::UrlInvalid { reason } => ValidationMessage::key("validation-garde-url")
                .with_param("reason", reason.to_string()),
            GardeRule::PrefixMissing { pattern } => {
                ValidationMessage::key("validation-garde-prefix").with_param("prefix", pattern)
            }
            GardeRule::SuffixMissing { pattern } => {
                ValidationMessage::key("validation-garde-suffix").with_param("suffix", pattern)
            }
            GardeRule::PhoneNumberInvalid { reason } => {
                ValidationMessage::key("validation-garde-phone-number")
                    .with_param("reason", reason.to_string())
            }
            GardeRule::IpInvalid { kind } => {
                ValidationMessage::key("validation-garde-ip").with_param("kind", kind.to_string())
            }
            GardeRule::MatchesFieldMismatch { field } => {
                ValidationMessage::key("validation-garde-matches").with_param("field", field)
            }
            GardeRule::EmailInvalid { reason } => ValidationMessage::key("validation-garde-email")
                .with_param("reason", reason.to_string()),
            GardeRule::AsciiInvalid => ValidationMessage::key("validation-garde-ascii"),
            GardeRule::AlphanumericInvalid => {
                ValidationMessage::key("validation-garde-alphanumeric")
            }
            GardeRule::RequiredNotSet => ValidationMessage::key("validation-garde-required"),
        }
    }
}

pub(crate) fn garde_message(
    key: &'static str,
    params: impl IntoIterator<Item = (&'static str, ErrorParamValue)>,
) -> garde::Error {
    let message = params
        .into_iter()
        .fold(ValidationMessage::key(key), |message, (name, value)| {
            message.with_param(name, value)
        });
    garde_error(message)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[test]
    fn garde_rules_map_to_existing_fluent_keys() {
        let cases = [
            (
                GardeRule::LengthLowerThan { min: 2 },
                ValidationMessage::key("validation-garde-length-min").with_param("min", 2_u64),
            ),
            (
                GardeRule::LengthGreaterThan { max: 8 },
                ValidationMessage::key("validation-garde-length-max").with_param("max", 8_u64),
            ),
            (
                GardeRule::RangeLowerThan {
                    min: Cow::Borrowed("1"),
                },
                ValidationMessage::key("validation-garde-range-min").with_param("min", "1"),
            ),
            (
                GardeRule::RangeGreaterThan {
                    max: Cow::Borrowed("9"),
                },
                ValidationMessage::key("validation-garde-range-max").with_param("max", "9"),
            ),
            (
                GardeRule::CreditCardInvalid {
                    reason: garde::i18n::InvalidCreditCard::InvalidFormat,
                },
                ValidationMessage::key("validation-garde-credit-card")
                    .with_param("reason", "invalid format".to_string()),
            ),
            (
                GardeRule::PatternNoMatch {
                    pattern: Cow::Borrowed("[a-z]+"),
                },
                ValidationMessage::key("validation-garde-pattern").with_param("pattern", "[a-z]+"),
            ),
            (
                GardeRule::ContainsMissing {
                    pattern: Cow::Borrowed("abc"),
                },
                ValidationMessage::key("validation-garde-contains").with_param("pattern", "abc"),
            ),
            (
                GardeRule::UrlInvalid {
                    reason: garde::i18n::InvalidUrl::EmptyHost,
                },
                ValidationMessage::key("validation-garde-url")
                    .with_param("reason", "empty host".to_string()),
            ),
            (
                GardeRule::PrefixMissing {
                    pattern: Cow::Borrowed("pre"),
                },
                ValidationMessage::key("validation-garde-prefix").with_param("prefix", "pre"),
            ),
            (
                GardeRule::SuffixMissing {
                    pattern: Cow::Borrowed("post"),
                },
                ValidationMessage::key("validation-garde-suffix").with_param("suffix", "post"),
            ),
            (
                GardeRule::PhoneNumberInvalid {
                    reason: garde::i18n::InvalidPhoneNumber::Invalid,
                },
                ValidationMessage::key("validation-garde-phone-number")
                    .with_param("reason", "not a valid phone number".to_string()),
            ),
            (
                GardeRule::IpInvalid {
                    kind: garde::i18n::IpKind::V4,
                },
                ValidationMessage::key("validation-garde-ip")
                    .with_param("kind", "IPv4".to_string()),
            ),
            (
                GardeRule::MatchesFieldMismatch {
                    field: Cow::Borrowed("password"),
                },
                ValidationMessage::key("validation-garde-matches").with_param("field", "password"),
            ),
            (
                GardeRule::EmailInvalid {
                    reason: garde::i18n::InvalidEmail::MissingAt,
                },
                ValidationMessage::key("validation-garde-email")
                    .with_param("reason", "value is missing `@`".to_string()),
            ),
            (
                GardeRule::AsciiInvalid,
                ValidationMessage::key("validation-garde-ascii"),
            ),
            (
                GardeRule::AlphanumericInvalid,
                ValidationMessage::key("validation-garde-alphanumeric"),
            ),
            (
                GardeRule::RequiredNotSet,
                ValidationMessage::key("validation-garde-required"),
            ),
        ];

        let locales = [I18n::for_locale_tag("en-US"), I18n::for_locale_tag("zh-CN")];
        for (rule, expected) in cases {
            let message = JacoGardeMessageProvider::message(rule);
            assert_eq!(message, expected);
            for i18n in &locales {
                let rendered = validation_message_with_i18n(&message, i18n);
                let ValidationMessage::Key { key, .. } = &message else {
                    unreachable!()
                };
                assert_ne!(rendered.as_ref(), key.as_ref());
            }
        }

        let provider_message = ValidationMessage::key("provider-validation-required")
            .with_param("field", "provider-field-name");
        assert_eq!(
            validation_message_with_i18n(&provider_message, &locales[0]).as_ref(),
            "Name is required",
        );
        assert_eq!(
            validation_message_with_i18n(&provider_message, &locales[1]).as_ref(),
            "名称 为必填项",
        );
    }
}
