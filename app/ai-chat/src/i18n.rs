use std::{
    collections::HashMap,
    sync::atomic::{AtomicU8, Ordering},
};

use crate::state::{AiChatConfig, Language};
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui::{App, Global};
use unic_langid::LanguageIdentifier;

const EN_US: &str = include_str!("../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../locales/zh-CN/main.ftl");
const LOCALE_UNSET: u8 = u8::MAX;
static CURRENT_STATIC_LOCALE: AtomicU8 = AtomicU8::new(LOCALE_UNSET);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::EnUs),
            1 => Some(Self::ZhCn),
            _ => None,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::EnUs => 0,
            Self::ZhCn => 1,
        }
    }
}

pub(crate) struct I18n {
    locale: Locale,
    bundles: HashMap<Locale, FluentBundle<FluentResource>>,
}

impl Global for I18n {}

pub(crate) fn init_i18n(cx: &mut App) {
    let i18n = I18n::from_config(cx);
    set_static_locale(i18n.locale);
    cx.set_global(i18n);
}

pub(crate) fn refresh_i18n(cx: &mut App) {
    let i18n = I18n::from_config(cx);
    set_static_locale(i18n.locale);
    cx.set_global(i18n);
}

pub(crate) fn t_static(key: &str) -> String {
    I18n::new(static_locale()).t(key)
}

impl I18n {
    fn new(locale: Locale) -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, build_bundle("en-US", EN_US));
        bundles.insert(Locale::ZhCn, build_bundle("zh-CN", ZH_CN));

        Self { locale, bundles }
    }

    fn from_config(cx: &App) -> Self {
        let language = cx
            .try_global::<AiChatConfig>()
            .map(AiChatConfig::language)
            .unwrap_or_default();
        Self::new(locale_for_language(language))
    }

    #[cfg(test)]
    pub(crate) fn english_for_test() -> Self {
        Self::new(Locale::EnUs)
    }

    #[cfg(test)]
    pub(crate) fn for_locale_tag(locale: &str) -> Self {
        let locale = match normalize_locale(locale).filter(|id| id.language.as_str() == "zh") {
            Some(_) => Locale::ZhCn,
            None => Locale::EnUs,
        };
        Self::new(locale)
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

fn locale_for_language(language: Language) -> Locale {
    match language {
        Language::English => Locale::EnUs,
        Language::Chinese => Locale::ZhCn,
        Language::System => detect_locale(),
    }
}

fn set_static_locale(locale: Locale) {
    CURRENT_STATIC_LOCALE.store(locale.as_u8(), Ordering::Relaxed);
}

fn static_locale() -> Locale {
    Locale::from_u8(CURRENT_STATIC_LOCALE.load(Ordering::Relaxed)).unwrap_or_else(detect_locale)
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
    use super::{I18n, locale_for_language};
    use crate::state::Language;

    #[test]
    fn explicit_language_selects_expected_locale() {
        assert_eq!(
            I18n::new(locale_for_language(Language::Chinese)).t("language-system"),
            "跟随系统"
        );
        assert_eq!(
            I18n::new(locale_for_language(Language::English)).t("language-system"),
            "System"
        );
    }
}
