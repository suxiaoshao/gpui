use std::collections::HashMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui_kit::Global;
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Locale {
    EnUs,
    ZhCn,
}

pub struct I18n {
    locale: Locale,
    bundles: HashMap<Locale, FluentBundle<FluentResource>>,
}

impl Global for I18n {}

impl I18n {
    /// Builds the application's translator from application-owned resources.
    pub fn new(locale: Locale, en_us: &str, zh_cn: &str) -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, build_bundle("en-US", en_us));
        bundles.insert(Locale::ZhCn, build_bundle("zh-CN", zh_cn));
        Self { locale, bundles }
    }

    pub fn detected(en_us: &str, zh_cn: &str) -> Self {
        Self::new(detect_locale(), en_us, zh_cn)
    }

    pub fn for_locale_tag(tag: &str, en_us: &str, zh_cn: &str) -> Self {
        let locale = normalize_locale(tag)
            .filter(|id| id.language.as_str() == "zh")
            .map_or(Locale::EnUs, |_| Locale::ZhCn);
        Self::new(locale, en_us, zh_cn)
    }

    pub fn t(&self, key: &str) -> String {
        self.translate(key, None)
    }

    pub fn t_with_args(&self, key: &str, args: &FluentArgs<'_>) -> String {
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
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(source.to_string()).expect("valid fluent resource");
    bundle
        .add_resource(resource)
        .expect("resource can be added");
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_normalization_and_fallback_preserve_translation() {
        let en = "greeting = Hello { $name }";
        let zh = "greeting = 你好 { $name }";
        let mut args = FluentArgs::new();
        args.set("name", "Alice");
        assert_eq!(
            I18n::for_locale_tag("zh_CN.UTF-8", en, zh).t_with_args("greeting", &args),
            "你好 Alice"
        );
        assert_eq!(
            I18n::for_locale_tag("fr-FR", en, zh).t_with_args("greeting", &args),
            "Hello Alice"
        );
        assert_eq!(I18n::new(Locale::EnUs, en, zh).t("missing"), "missing");
    }
}
