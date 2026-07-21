use std::{borrow::Cow, fmt::Display};

use fluent_bundle::FluentArgs;
use gpui::App;

use crate::foundation::I18n;

pub(crate) fn validation_message(
    message: &gpui_form::typed::ValidationMessage,
    cx: &App,
) -> gpui::SharedString {
    match message {
        gpui_form::typed::ValidationMessage::Localized(message) => message.to_string().into(),
        gpui_form::typed::ValidationMessage::Key { key, params } => {
            let mut args = FluentArgs::new();
            for (name, value) in params {
                let value = match value {
                    gpui_form::typed::ErrorParamValue::String(value) => value.to_string(),
                    gpui_form::typed::ErrorParamValue::Integer(value) => value.to_string(),
                    gpui_form::typed::ErrorParamValue::Unsigned(value) => value.to_string(),
                    gpui_form::typed::ErrorParamValue::Float(value) => value.to_string(),
                    gpui_form::typed::ErrorParamValue::Bool(value) => value.to_string(),
                };
                args.set(name.as_ref(), value);
            }
            cx.global::<I18n>().t_with_args(key.as_ref(), &args).into()
        }
    }
}

#[derive(Clone)]
pub(crate) struct JacoValidationContext<D> {
    pub(crate) dependencies: D,
    i18n: I18n,
}

impl<D: Clone> JacoValidationContext<D> {
    pub(crate) fn new(dependencies: D, cx: &App) -> Self {
        Self {
            dependencies,
            i18n: cx.global::<I18n>().clone(),
        }
    }

    pub(crate) fn relocalized(&self, cx: &App) -> Self {
        Self::new(self.dependencies.clone(), cx)
    }

    pub(crate) fn error(&self, key: &'static str, args: &FluentArgs<'_>) -> garde::Error {
        garde::Error::new(self.i18n.t_with_args(key, args))
    }

    pub(crate) fn text(&self, key: &'static str) -> String {
        self.i18n.t(key)
    }
}

pub(crate) struct JacoGardeI18nProvider;

pub(crate) struct JacoGardeI18n {
    i18n: I18n,
}

impl<D> gpui_form::typed::GardeI18nProvider<JacoValidationContext<D>> for JacoGardeI18nProvider
where
    D: Clone + 'static,
{
    type Handler<'a>
        = JacoGardeI18n
    where
        D: 'a;

    fn handler<'a>(context: &'a JacoValidationContext<D>, _cx: &'a App) -> Self::Handler<'a> {
        JacoGardeI18n {
            i18n: context.i18n.clone(),
        }
    }
}

impl JacoGardeI18n {
    fn message(&self, key: &'static str) -> Cow<'static, str> {
        Cow::Owned(self.i18n.t(key))
    }

    fn message_with(
        &self,
        key: &'static str,
        name: &'static str,
        value: &dyn Display,
    ) -> Cow<'static, str> {
        let mut args = FluentArgs::new();
        args.set(name, value.to_string());
        Cow::Owned(self.i18n.t_with_args(key, &args))
    }
}

impl garde::i18n::I18n for JacoGardeI18n {
    fn length_lower_than(&self, min: usize) -> Cow<'static, str> {
        self.message_with("validation-garde-length-min", "min", &min)
    }

    fn length_greater_than(&self, max: usize) -> Cow<'static, str> {
        self.message_with("validation-garde-length-max", "max", &max)
    }

    fn range_lower_than(&self, min: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-range-min", "min", min)
    }

    fn range_greater_than(&self, max: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-range-max", "max", max)
    }

    fn credit_card_invalid(&self, reason: garde::i18n::InvalidCreditCard) -> Cow<'static, str> {
        self.message_with("validation-garde-credit-card", "reason", &reason)
    }

    fn pattern_no_match(&self, pattern: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-pattern", "pattern", pattern)
    }

    fn contains_missing(&self, pattern: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-contains", "pattern", pattern)
    }

    fn url_invalid(&self, reason: garde::i18n::InvalidUrl) -> Cow<'static, str> {
        self.message_with("validation-garde-url", "reason", &reason)
    }

    fn prefix_missing(&self, pattern: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-prefix", "prefix", pattern)
    }

    fn suffix_missing(&self, pattern: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-suffix", "suffix", pattern)
    }

    fn phone_number_invalid(&self, reason: garde::i18n::InvalidPhoneNumber) -> Cow<'static, str> {
        self.message_with("validation-garde-phone-number", "reason", &reason)
    }

    fn ip_invalid(&self, kind: garde::i18n::IpKind) -> Cow<'static, str> {
        self.message_with("validation-garde-ip", "kind", &kind)
    }

    fn matches_field_mismatch(&self, field: &dyn Display) -> Cow<'static, str> {
        self.message_with("validation-garde-matches", "field", field)
    }

    fn email_invalid(&self, reason: garde::i18n::InvalidEmail) -> Cow<'static, str> {
        self.message_with("validation-garde-email", "reason", &reason)
    }

    fn ascii_invalid(&self) -> Cow<'static, str> {
        self.message("validation-garde-ascii")
    }

    fn alphanumeric_invalid(&self) -> Cow<'static, str> {
        self.message("validation-garde-alphanumeric")
    }

    fn required_not_set(&self) -> Cow<'static, str> {
        self.message("validation-garde-required")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garde_builtin_rule_messages_exist_in_both_locales() {
        let parameterized = [
            ("validation-garde-length-min", "min"),
            ("validation-garde-length-max", "max"),
            ("validation-garde-range-min", "min"),
            ("validation-garde-range-max", "max"),
            ("validation-garde-credit-card", "reason"),
            ("validation-garde-pattern", "pattern"),
            ("validation-garde-contains", "pattern"),
            ("validation-garde-url", "reason"),
            ("validation-garde-prefix", "prefix"),
            ("validation-garde-suffix", "suffix"),
            ("validation-garde-phone-number", "reason"),
            ("validation-garde-ip", "kind"),
            ("validation-garde-matches", "field"),
            ("validation-garde-email", "reason"),
        ];
        let plain = [
            "validation-garde-ascii",
            "validation-garde-alphanumeric",
            "validation-garde-required",
        ];

        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for (key, parameter) in parameterized {
                let mut args = FluentArgs::new();
                args.set(parameter, "sample");
                assert_ne!(i18n.t_with_args(key, &args), key, "{locale}: {key}");
            }
            for key in plain {
                assert_ne!(i18n.t(key), key, "{locale}: {key}");
            }
        }
    }
}
