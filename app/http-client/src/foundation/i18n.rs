use std::collections::HashMap;

#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui::{App, Global, SharedString};
use gpui_form::{ErrorParamValue, ValidationMessage};
use unic_langid::LanguageIdentifier;

const EN_US: &str = include_str!("../../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../../locales/zh-CN/main.ftl");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Locale {
    EnUs,
    ZhCn,
}

pub(crate) struct I18n {
    locale: Locale,
    bundles: HashMap<Locale, FluentBundle<FluentResource>>,
}

impl Global for I18n {}

pub(crate) fn init_i18n(cx: &mut App) {
    cx.set_global(I18n::new(detect_locale()));
}

impl I18n {
    fn new(locale: Locale) -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, build_bundle("en-US", EN_US));
        bundles.insert(Locale::ZhCn, build_bundle("zh-CN", ZH_CN));

        Self { locale, bundles }
    }

    pub(crate) fn t(&self, key: &str) -> String {
        self.translate(key, None)
    }

    pub(crate) fn t_with_args(&self, key: &str, args: &FluentArgs<'_>) -> String {
        self.translate(key, Some(args))
    }

    fn translate(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        let Some(bundle) = self.bundle() else {
            return key.to_string();
        };
        let Some(message) = bundle.get_message(key) else {
            return key.to_string();
        };
        let Some(pattern) = message.value() else {
            return key.to_string();
        };

        let mut errors = vec![];
        let text = bundle.format_pattern(pattern, args, &mut errors);
        if errors.is_empty() {
            text.to_string()
        } else {
            key.to_string()
        }
    }

    fn bundle(&self) -> Option<&FluentBundle<FluentResource>> {
        self.bundles
            .get(&self.locale)
            .or_else(|| self.bundles.get(&Locale::EnUs))
    }
}

/// Resolves a form validation message at the application rendering boundary.
///
/// Form validation stores stable keys and parameters rather than translated strings, so changing
/// the application locale does not require the form to be validated again.
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

fn detect_locale() -> Locale {
    let locale = sys_locale::get_locale()
        .or_else(|| read_env_locale("LC_ALL"))
        .or_else(|| read_env_locale("LANG"))
        .or_else(|| read_env_locale("LANGUAGE"));

    match locale
        .as_deref()
        .and_then(normalize_locale)
        .filter(|id| id.language.as_str() == "zh")
    {
        Some(_) => Locale::ZhCn,
        None => Locale::EnUs,
    }
}

fn read_env_locale(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_locale(value: &str) -> Option<LanguageIdentifier> {
    let normalized = value
        .split(['.', '@'])
        .next()
        .unwrap_or(value)
        .replace('_', "-");

    normalized.parse::<LanguageIdentifier>().ok()
}

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
mod tests {
    use super::*;

    const REQUIRED_REQUEST_KEYS: &[&str] = &[
        "gpui-form-error-required",
        "button-send",
        "button-cancel",
        "button-clear-response",
        "button-save-response",
        "button-add",
        "button-delete",
        "button-select-file",
        "button-change-file",
        "button-clear-file",
        "button-move-up",
        "button-move-down",
        "tab-params",
        "tab-authorization",
        "tab-headers",
        "tab-body",
        "tab-settings",
        "tab-response-body",
        "tab-response-headers",
        "response-title",
        "response-empty",
        "response-sending",
        "response-receiving-known",
        "response-receiving-unknown",
        "response-status",
        "response-final-url",
        "response-protocol",
        "response-head-time",
        "response-total-time",
        "response-received-size",
        "response-stored-size",
        "response-header-name",
        "response-header-value",
        "response-headers-empty",
        "response-view-auto",
        "response-view-text",
        "response-view-json",
        "response-view-xml",
        "response-view-hex",
        "response-view-base64",
        "response-view-image",
        "response-view-audio",
        "response-view-video",
        "response-view-pdf",
        "response-media-loading",
        "response-media-play",
        "response-media-pause",
        "response-media-mute",
        "response-media-unmute",
        "response-media-position",
        "response-media-runtime-unavailable",
        "response-media-plugin-missing",
        "response-media-unsupported",
        "response-media-decode-failed",
        "response-media-control-failed",
        "response-media-resolution-unsupported",
        "response-pdf-loading",
        "response-pdf-previous",
        "response-pdf-next",
        "response-pdf-page",
        "response-pdf-invalid",
        "response-pdf-encrypted",
        "response-pdf-too-large",
        "response-pdf-render-failed",
        "response-preview-truncated",
        "response-decoding-unsupported",
        "response-viewer-mode-unavailable",
        "response-viewer-invalid-json",
        "response-viewer-invalid-image",
        "response-image-too-large",
        "response-save-complete",
        "response-save-failed",
        "request-problem-transport",
        "request-problem-timeout",
        "request-problem-redirect",
        "request-problem-request-body",
        "request-problem-response-read",
        "request-problem-response-decode",
        "request-problem-storage",
        "request-problem-too-large-encoded",
        "request-problem-too-large-stored",
        "request-problem-internal",
        "field-method",
        "field-url",
        "field-name",
        "field-key",
        "field-value",
        "field-content-type",
        "field-file",
        "field-username",
        "field-password",
        "field-token",
        "field-location",
        "field-timeout-ms",
        "params-invalid-url-disabled",
        "body-none",
        "body-form-data",
        "body-urlencoded",
        "body-text",
        "body-binary",
        "text-format-plain",
        "text-format-json",
        "text-format-javascript",
        "text-format-html",
        "text-format-xml",
        "text-format-css",
        "multipart-text",
        "multipart-file",
        "multipart-file-not-selected",
        "auth-none",
        "auth-basic",
        "auth-bearer",
        "auth-api-key",
        "auth-location-header",
        "auth-location-query",
        "auth-generated-override",
        "auth-query-override",
        "body-content-type-override",
        "settings-follow-redirects",
        "settings-follow-original-method",
        "settings-timeout-help",
        "request-url-invalid",
        "request-url-scheme-invalid",
        "request-header-name-invalid",
        "request-header-value-invalid",
        "request-media-type-invalid",
        "request-multipart-name-required",
        "request-multipart-name-invalid",
        "request-file-required",
        "request-file-unavailable",
        "request-file-name-invalid",
        "request-basic-username-colon",
        "request-auth-value-invalid",
        "request-api-key-name-required",
        "request-api-key-name-invalid",
    ];

    #[test]
    fn locales_have_the_same_message_and_variable_contract() {
        assert_eq!(fluent_contract(EN_US), fluent_contract(ZH_CN));
    }

    #[test]
    fn media_and_pdf_messages_have_the_required_variable_contract() {
        let expected = [
            ("response-media-position", &["current", "total"] as &[&str]),
            ("response-media-plugin-missing", &["plugin"] as &[&str]),
            (
                "response-media-resolution-unsupported",
                &["height", "width"] as &[&str],
            ),
            ("response-pdf-page", &["current", "total"] as &[&str]),
        ];

        for (locale, source) in [("en-US", EN_US), ("zh-CN", ZH_CN)] {
            let contract = fluent_contract(source);
            for (key, variables) in expected {
                let actual = contract
                    .get(key)
                    .unwrap_or_else(|| panic!("{locale} is missing {key}"));
                let expected = variables.iter().map(|value| (*value).to_string()).collect();
                assert_eq!(
                    actual, &expected,
                    "{locale} has an invalid variable contract for {key}"
                );
            }
        }
    }

    #[test]
    fn request_messages_parse_and_cover_the_plan_contract() {
        for (locale, source) in [("en-US", EN_US), ("zh-CN", ZH_CN)] {
            let contract = fluent_contract(source);
            let bundle = build_bundle(locale, source);

            for key in contract.keys() {
                assert!(
                    bundle
                        .get_message(key)
                        .and_then(|message| message.value())
                        .is_some(),
                    "{locale} is missing a value for {key}",
                );
            }

            for key in REQUIRED_REQUEST_KEYS {
                assert!(
                    contract.contains_key(*key),
                    "{locale} is missing required Request key {key}",
                );
            }
        }
    }

    #[test]
    fn validation_messages_translate_keys_parameters_and_literals() {
        let zh_cn = I18n::new(Locale::ZhCn);
        assert_eq!(
            validation_message_with_i18n(
                &ValidationMessage::key("gpui-form-error-required"),
                &zh_cn,
            ),
            "此字段为必填项。"
        );
        assert_eq!(
            validation_message_with_i18n(&ValidationMessage::literal("literal error"), &zh_cn),
            "literal error"
        );

        let resource = FluentResource::try_new("parameter-test = Value: { $value }".to_string())
            .expect("valid test Fluent resource");
        let mut parameter_bundle = FluentBundle::new(vec![
            "en-US"
                .parse::<LanguageIdentifier>()
                .expect("valid language id"),
        ]);
        parameter_bundle.set_use_isolating(false);
        parameter_bundle
            .add_resource(resource)
            .expect("test resource can be added");
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, parameter_bundle);
        let parameter_i18n = I18n {
            locale: Locale::EnUs,
            bundles,
        };
        assert_eq!(
            validation_message_with_i18n(
                &ValidationMessage::key("parameter-test").with_param("value", 42_u64),
                &parameter_i18n,
            ),
            "Value: 42"
        );
    }
}
