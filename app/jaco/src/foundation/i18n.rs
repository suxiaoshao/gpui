use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui::{App, AppContext, Entity, Global, Subscription};
use gpui_store::Select;
use jaco_core::AppLanguage;
use std::{collections::HashMap, rc::Rc};
use unic_langid::LanguageIdentifier;

use crate::state::config;

#[derive(Clone, Copy, Default)]
struct SelectLanguage;

impl Select<config::ConfigOperation> for SelectLanguage {
    type Output = AppLanguage;

    fn select(&self, operation: &config::ConfigOperation) -> Self::Output {
        operation
            .data()
            .map(|config| config.app_settings_payload().language)
            .unwrap_or_default()
    }
}

const EN_US: &str = include_str!("../../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../../locales/zh-CN/main.ftl");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Locale {
    EnUs,
    ZhCn,
}

#[derive(Clone)]
pub(crate) struct I18n {
    locale: Locale,
    bundles: Rc<HashMap<Locale, FluentBundle<FluentResource>>>,
}

impl Global for I18n {}

struct LocalizationRuntime {
    language: AppLanguage,
    _subscription: Subscription,
}

struct LocalizationRuntimeGlobal {
    _runtime: Entity<LocalizationRuntime>,
}

impl Global for LocalizationRuntimeGlobal {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(I18n::from_settings(cx));
}

pub(crate) fn init_runtime(cx: &mut App) {
    let language = config::store(cx).read(cx, |operation| {
        operation
            .data()
            .map(|config| config.app_settings_payload().language)
            .unwrap_or_default()
    });
    let runtime = cx.new(|cx| {
        let subscription = config::store(cx).observe_select(
            cx,
            SelectLanguage,
            |runtime: &mut LocalizationRuntime, language, cx| runtime.apply(*language, cx),
        );
        LocalizationRuntime {
            language,
            _subscription: subscription,
        }
    });
    cx.set_global(LocalizationRuntimeGlobal { _runtime: runtime });
}

pub(crate) fn init_bootstrap(cx: &mut App) {
    cx.set_global(I18n::new(detect_locale()));
}

impl LocalizationRuntime {
    fn apply(&mut self, language: AppLanguage, cx: &mut App) {
        if self.language == language {
            return;
        }
        self.language = language;
        cx.set_global(I18n::new(locale_for_language(language)));
        crate::app::menus::sync_app_menus(cx);
        crate::app::reload_app_menu_bars(cx);
        cx.refresh_windows();
    }
}

impl I18n {
    fn new(locale: Locale) -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, build_bundle("en-US", EN_US));
        bundles.insert(Locale::ZhCn, build_bundle("zh-CN", ZH_CN));

        Self {
            locale,
            bundles: Rc::new(bundles),
        }
    }

    fn from_settings(cx: &App) -> Self {
        let language = if cx.has_global::<config::JacoConfigStore>() {
            config::store(cx).read(cx, |operation| {
                operation
                    .data()
                    .map(|config| config.app_settings_payload().language)
                    .unwrap_or_default()
            })
        } else {
            AppLanguage::default()
        };
        Self::new(locale_for_language(language))
    }

    #[cfg(test)]
    pub(crate) fn for_language(language: AppLanguage) -> Self {
        Self::new(locale_for_language(language))
    }

    #[cfg(test)]
    pub(crate) fn for_locale_tag(locale: &str) -> Self {
        let locale = match normalize_locale(locale).filter(|id| id.language.as_str() == "zh") {
            Some(_) => Locale::ZhCn,
            None => Locale::EnUs,
        };
        Self::new(locale)
    }

    #[cfg(test)]
    pub(crate) fn english_for_test() -> Self {
        Self::new(Locale::EnUs)
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

fn locale_for_language(language: AppLanguage) -> Locale {
    match language {
        AppLanguage::English => Locale::EnUs,
        AppLanguage::Chinese => Locale::ZhCn,
        AppLanguage::System => detect_locale(),
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
mod tests {
    use super::I18n;
    use fluent_bundle::FluentArgs;
    use jaco_core::AppLanguage;

    #[test]
    fn explicit_language_selects_expected_locale() {
        assert_eq!(
            I18n::for_language(AppLanguage::Chinese).t("language-system"),
            "跟随系统"
        );
        assert_eq!(
            I18n::for_language(AppLanguage::English).t("language-system"),
            "System"
        );
    }

    #[test]
    fn missing_key_falls_back_to_key() {
        assert_eq!(
            I18n::for_language(AppLanguage::English).t("not-a-real-key"),
            "not-a-real-key"
        );
    }

    #[test]
    fn formatted_messages_are_localized() {
        let mut args = FluentArgs::new();
        args.set("path", "/tmp/jaco");
        args.set("error", "expected table");
        args.set("message", "could not initialize");
        args.set("duration", "1s");
        args.set("name", "read_file");
        args.set("id", "invocation-1");

        assert_ne!(
            I18n::for_locale_tag("en-US").t_with_args("status-data-dir", &args),
            "status-data-dir"
        );
        assert_ne!(
            I18n::for_locale_tag("zh-CN").t_with_args("status-data-dir", &args),
            "status-data-dir"
        );
        assert_ne!(
            I18n::for_locale_tag("en-US").t_with_args("config-load-error-message", &args),
            "config-load-error-message"
        );
        assert_ne!(
            I18n::for_locale_tag("zh-CN").t_with_args("config-load-error-message", &args),
            "config-load-error-message"
        );
        assert_ne!(
            I18n::for_locale_tag("en-US").t("config-load-error-title"),
            "config-load-error-title"
        );
        assert_ne!(
            I18n::for_locale_tag("zh-CN").t("config-load-error-title"),
            "config-load-error-title"
        );
        assert_ne!(
            I18n::for_locale_tag("en-US").t_with_args("conversation-agent-failed", &args),
            "conversation-agent-failed"
        );
        assert_ne!(
            I18n::for_locale_tag("zh-CN").t_with_args("conversation-agent-failed", &args),
            "conversation-agent-failed"
        );
        assert_ne!(
            I18n::for_locale_tag("en-US").t_with_args("conversation-agent-canceled", &args),
            "conversation-agent-canceled"
        );
        assert_ne!(
            I18n::for_locale_tag("zh-CN").t_with_args("conversation-agent-canceled", &args),
            "conversation-agent-canceled"
        );
        for locale in ["en-US", "zh-CN"] {
            assert_ne!(
                I18n::for_locale_tag(locale)
                    .t_with_args("critical-session-error-description", &args),
                "critical-session-error-description"
            );
        }
        for key in [
            "conversation-status-canceled",
            "conversation-status-max-steps",
            "conversation-status-completed-without-output",
            "critical-session-error-title",
            "critical-session-retry",
        ] {
            assert_ne!(I18n::for_locale_tag("en-US").t(key), key);
            assert_ne!(I18n::for_locale_tag("zh-CN").t(key), key);
        }

        for key in [
            "conversation-tool-invocation-title",
            "conversation-tool-source-local",
            "conversation-tool-source-mcp",
            "conversation-tool-source-provider-hosted",
            "conversation-tool-status-requested",
            "conversation-tool-status-awaiting-approval",
            "conversation-tool-status-running",
            "conversation-tool-status-succeeded",
            "conversation-tool-status-failed",
            "conversation-tool-status-denied",
            "conversation-tool-status-canceled",
            "conversation-tool-duration",
            "conversation-tool-duration-updated",
            "conversation-tool-unavailable",
            "conversation-tool-field-model-name",
            "conversation-tool-field-original-name",
            "conversation-tool-field-namespace",
            "conversation-tool-field-source",
            "conversation-tool-field-server",
            "conversation-tool-field-invocation-id",
            "conversation-tool-field-call-id",
            "conversation-tool-field-arguments",
            "conversation-tool-field-access",
            "conversation-tool-field-approval",
            "conversation-tool-access-kind-read",
            "conversation-tool-access-kind-write",
            "conversation-tool-access-kind-execute",
            "conversation-tool-access-kind-network",
            "conversation-tool-access-target",
            "conversation-tool-access-normalized-path",
            "conversation-tool-access-within-project",
            "conversation-tool-access-reason-key",
            "conversation-tool-value-yes",
            "conversation-tool-value-no",
            "conversation-tool-field-created-at",
            "conversation-tool-field-started-at",
            "conversation-tool-field-completed-at",
            "conversation-tool-field-updated-at",
            "conversation-tool-field-text-output",
            "conversation-tool-field-structured-output",
            "conversation-tool-field-error",
            "conversation-tool-error-code",
            "conversation-tool-error-message",
            "conversation-tool-error-retryable",
            "conversation-tool-error-provider",
            "conversation-tool-approval-pending",
            "conversation-tool-approval-approved",
            "conversation-tool-approval-denied",
            "conversation-tool-approval-expired",
            "conversation-tool-approval-canceled",
            "conversation-tool-approval-request-reason",
            "conversation-tool-approval-requested-at",
            "conversation-tool-approval-decision",
            "conversation-tool-approval-decided-by",
            "conversation-tool-approval-decision-reason",
            "conversation-tool-approval-decided-at",
            "conversation-tool-approval-expires-at",
            "conversation-tool-preview-truncated",
            "conversation-tool-raw-hidden",
            "conversation-tool-expand",
            "conversation-tool-collapse",
            "conversation-tool-copy-preview",
            "conversation-tool-unresolved",
        ] {
            for locale in ["en-US", "zh-CN"] {
                assert_ne!(
                    I18n::for_locale_tag(locale).t_with_args(key, &args),
                    key,
                    "missing tool invocation i18n key {key} for {locale}"
                );
            }
        }
    }

    #[test]
    fn conversation_attachment_messages_exist_in_each_locale() {
        let mut args = FluentArgs::new();
        args.set("name", "report.pdf");
        let keys = [
            "conversation-attachment-fallback-name",
            "conversation-attachment-type-file",
            "conversation-attachment-type-attachment",
            "conversation-attachment-source-managed",
            "conversation-attachment-source-local",
            "conversation-attachment-source-generated",
            "conversation-attachment-source-external",
            "conversation-attachment-source-provider",
            "conversation-attachment-status-checking",
            "conversation-attachment-status-unavailable",
            "conversation-attachment-unavailable-missing-record",
            "conversation-attachment-unavailable-invalid-record",
            "conversation-attachment-unavailable-source",
            "conversation-attachment-unavailable-missing-file",
            "conversation-attachment-unavailable-access",
            "conversation-attachment-open",
            "conversation-attachment-reveal-macos",
            "conversation-attachment-reveal-windows",
            "conversation-attachment-reveal-linux",
            "conversation-attachment-save-copy",
            "conversation-attachment-action-failed-title",
            "conversation-attachment-action-failed-message",
            "conversation-attachment-save-failed-title",
            "conversation-attachment-save-failed-message",
            "conversation-attachment-save-success-title",
            "conversation-attachment-save-success-message",
        ];

        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for key in keys {
                assert_ne!(
                    i18n.t_with_args(key, &args),
                    key,
                    "missing conversation attachment i18n key {key} for {locale}"
                );
            }
            assert!(
                i18n.t_with_args("conversation-attachment-save-success-message", &args)
                    .contains("report.pdf"),
                "conversation attachment success message must interpolate $name for {locale}"
            );
        }
    }
}
