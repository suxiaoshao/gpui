use std::collections::HashMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui::{App, Global};
use unic_langid::LanguageIdentifier;

const EN_US: &str = include_str!("../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../locales/zh-CN/main.ftl");

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

pub(crate) fn t_static(key: &str) -> String {
    I18n::new(detect_locale()).t(key)
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
    let resource = FluentResource::try_new(source.to_string()).expect("valid fluent resource");
    bundle.add_resource(resource).expect("resource can be added");
    bundle
}
