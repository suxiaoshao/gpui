pub use app_i18n::I18n;
#[cfg(test)]
use app_i18n::Locale;
use fluent_bundle::FluentArgs;
#[cfg(test)]
use fluent_bundle::{FluentBundle, FluentResource};
use gpui_form::{ErrorParamValue, ValidationMessage};
use gpui_kit::App;
use gpui_kit::SharedString;
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use unic_langid::LanguageIdentifier;
#[cfg(test)]
const EN_US: &str = include_str!("../../../locales/en-US/main.ftl");
#[cfg(test)]
const ZH_CN: &str = include_str!("../../../locales/zh-CN/main.ftl");

#[cfg(test)]
pub(crate) fn for_locale_tag(tag: &str) -> I18n {
    I18n::for_locale_tag(tag, EN_US, ZH_CN)
}
#[cfg(test)]
fn build_bundle(lang: &str, source: &str) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = lang.parse().expect("valid language id");
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(source.to_string()).expect("valid fluent resource");
    bundle
        .add_resource(resource)
        .expect("resource can be added");
    bundle
}

/// Resolves a form validation message when the field is rendered.
///
/// Form validation retains stable message keys and parameters; localization belongs to the
/// application boundary so a locale change does not require re-validating the form.
pub(crate) fn validation_message(message: &ValidationMessage, cx: &App) -> SharedString {
    validation_message_with_i18n(message, cx.global::<I18n>())
}

pub(crate) fn validation_message_with_i18n(
    message: &ValidationMessage,
    i18n: &I18n,
) -> SharedString {
    match message {
        ValidationMessage::Literal(message) => message.to_string().into(),
        ValidationMessage::Key { key, params } => {
            let mut args = FluentArgs::new();
            for (name, value) in params {
                let value = match value {
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

#[cfg(test)]
fn fluent_contract(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut contract = BTreeMap::new();
    let mut current_key = None;

    for line in source.lines() {
        if let Some((candidate, value)) = line.split_once('=') {
            let candidate = candidate.trim();
            if !candidate.is_empty()
                && candidate
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            {
                let key = candidate.to_string();
                contract.entry(key.clone()).or_default();
                current_key = Some(key);
                collect_variables(value, &mut contract, current_key.as_deref());
                continue;
            }
        }

        collect_variables(line, &mut contract, current_key.as_deref());
    }

    contract
}

#[cfg(test)]
fn collect_variables(
    text: &str,
    contract: &mut BTreeMap<String, BTreeSet<String>>,
    current_key: Option<&str>,
) {
    let Some(current_key) = current_key else {
        return;
    };

    let variables = contract
        .get_mut(current_key)
        .expect("current Fluent key must have an entry");
    let mut remaining = text;
    while let Some((_, suffix)) = remaining.split_once('$') {
        let name: String = suffix
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '-')
            .collect();
        if !name.is_empty() {
            variables.insert(name);
        }
        remaining = suffix;
    }
}

#[cfg(test)]
pub(crate) fn init_i18n(cx: &mut App) {
    cx.set_global(I18n::detected(EN_US, ZH_CN));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_have_the_same_message_and_variable_contract() {
        assert_eq!(fluent_contract(EN_US), fluent_contract(ZH_CN));
    }

    #[test]
    fn all_declared_messages_are_available_from_both_bundles() {
        for (locale, source) in [("en-US", EN_US), ("zh-CN", ZH_CN)] {
            let bundle = build_bundle(locale, source);
            for key in fluent_contract(source).keys() {
                assert!(
                    bundle
                        .get_message(key)
                        .and_then(|message| message.value())
                        .is_some(),
                    "{locale} is missing a value for {key}",
                );
            }
        }
    }

    #[test]
    fn validation_messages_translate_keys_and_preserve_literals() {
        let zh_cn = I18n::new(Locale::ZhCn, EN_US, ZH_CN);
        assert_eq!(
            validation_message_with_i18n(
                &ValidationMessage::key("gpui-form-error-required"),
                &zh_cn,
            ),
            "此字段为必填项"
        );

        let en_us = I18n::new(Locale::EnUs, EN_US, ZH_CN);
        let status = validation_message_with_i18n(
            &ValidationMessage::key("download-error-http-status").with_param("status", 503_u64),
            &en_us,
        );
        assert_eq!(status, "The server returned HTTP status 503.");
        assert_eq!(
            validation_message_with_i18n(&ValidationMessage::literal("literal error"), &en_us),
            "literal error"
        );
    }
}
