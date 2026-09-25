use crate::state::config::AppLanguage;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui_kit::{App, Global};

struct LocaleSpec {
    id: &'static str,
    component_locale: &'static str,
    autonym: &'static str,
    source: &'static str,
}

const ENGLISH: LocaleSpec = LocaleSpec {
    id: "en-US",
    component_locale: "en",
    autonym: "English",
    source: include_str!("../../locales/en-US/main.ftl"),
};

const SIMPLIFIED_CHINESE: LocaleSpec = LocaleSpec {
    id: "zh-CN",
    component_locale: "zh-CN",
    autonym: "简体中文",
    source: include_str!("../../locales/zh-CN/main.ftl"),
};

const TRADITIONAL_CHINESE: LocaleSpec = LocaleSpec {
    id: "zh-TW",
    component_locale: "zh-TW",
    autonym: "繁體中文",
    source: include_str!("../../locales/zh-TW/main.ftl"),
};

const JAPANESE: LocaleSpec = LocaleSpec {
    id: "ja",
    component_locale: "ja",
    autonym: "日本語",
    source: include_str!("../../locales/ja/main.ftl"),
};

const KOREAN: LocaleSpec = LocaleSpec {
    id: "ko",
    component_locale: "ko",
    autonym: "한국어",
    source: include_str!("../../locales/ko/main.ftl"),
};

const GERMAN: LocaleSpec = LocaleSpec {
    id: "de",
    component_locale: "de",
    autonym: "Deutsch",
    source: include_str!("../../locales/de/main.ftl"),
};

const FRENCH: LocaleSpec = LocaleSpec {
    id: "fr",
    component_locale: "fr",
    autonym: "Français",
    source: include_str!("../../locales/fr/main.ftl"),
};

const SPANISH: LocaleSpec = LocaleSpec {
    id: "es",
    component_locale: "es",
    autonym: "Español",
    source: include_str!("../../locales/es/main.ftl"),
};

const BRAZILIAN_PORTUGUESE: LocaleSpec = LocaleSpec {
    id: "pt-BR",
    component_locale: "pt-BR",
    autonym: "Português (Brasil)",
    source: include_str!("../../locales/pt-BR/main.ftl"),
};

#[cfg(test)]
const LOCALES: [&LocaleSpec; 9] = [
    &ENGLISH,
    &SIMPLIFIED_CHINESE,
    &TRADITIONAL_CHINESE,
    &JAPANESE,
    &KOREAN,
    &GERMAN,
    &FRENCH,
    &SPANISH,
    &BRAZILIAN_PORTUGUESE,
];

pub(crate) struct I18n {
    bundle: FluentBundle<FluentResource>,
    english_fallback: FluentBundle<FluentResource>,
    locale: &'static str,
}

pub(crate) struct SystemLocale(pub Option<String>);

impl Global for I18n {}
impl Global for SystemLocale {}

pub(crate) fn apply(language: AppLanguage, cx: &mut App) {
    let locale = locale_for_language(language, cx);
    if cx
        .try_global::<I18n>()
        .is_some_and(|current| current.locale == locale.id)
    {
        return;
    }

    cx.set_global(I18n {
        bundle: build_bundle(locale),
        english_fallback: build_bundle(&ENGLISH),
        locale: locale.id,
    });
    gpui_kit::component::set_locale(locale.component_locale);
}

fn build_bundle(locale: &LocaleSpec) -> FluentBundle<FluentResource> {
    let mut bundle = FluentBundle::new(vec![locale.id.parse().expect("valid locale identifier")]);
    bundle
        .add_resource(
            FluentResource::try_new(locale.source.to_owned()).expect("valid Fluent resource"),
        )
        .expect("unique Fluent message identifiers");
    bundle
}

pub(crate) fn t(cx: &App, key: &str) -> String {
    t_with_args(cx, key, &FluentArgs::new())
}

pub(crate) fn t_with_args(cx: &App, key: &str, args: &FluentArgs<'_>) -> String {
    let i18n = cx.global::<I18n>();
    format_message(&i18n.bundle, key, args)
        .or_else(|| format_message(&i18n.english_fallback, key, args))
        .unwrap_or_else(|| key.into())
}

fn format_message(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: &FluentArgs<'_>,
) -> Option<String> {
    let message = bundle.get_message(key)?;
    let pattern = message.value()?;
    Some(
        bundle
            .format_pattern(pattern, Some(args), &mut Vec::new())
            .into_owned(),
    )
}

fn locale_for_language(language: AppLanguage, cx: &App) -> &'static LocaleSpec {
    if matches!(language, AppLanguage::System) {
        system_locale(cx)
    } else {
        explicit_locale(language)
    }
}

fn explicit_locale(language: AppLanguage) -> &'static LocaleSpec {
    match language {
        AppLanguage::System => &ENGLISH,
        AppLanguage::English => &ENGLISH,
        AppLanguage::Chinese => &SIMPLIFIED_CHINESE,
        AppLanguage::TraditionalChinese => &TRADITIONAL_CHINESE,
        AppLanguage::Japanese => &JAPANESE,
        AppLanguage::Korean => &KOREAN,
        AppLanguage::German => &GERMAN,
        AppLanguage::French => &FRENCH,
        AppLanguage::Spanish => &SPANISH,
        AppLanguage::BrazilianPortuguese => &BRAZILIAN_PORTUGUESE,
    }
}

/// Return an explicit locale override for native platform UI. System keeps the OS default.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn native_locale(language: AppLanguage) -> Option<&'static str> {
    match language {
        AppLanguage::System => None,
        language => Some(explicit_locale(language).id),
    }
}

fn locale_for_system_tag(tag: &str) -> &'static LocaleSpec {
    let tag = tag.split(['.', '@']).next().unwrap_or(tag);
    let parts = tag.split(['-', '_']).collect::<Vec<_>>();
    let Some(language) = parts.first().map(|part| part.to_ascii_lowercase()) else {
        return &ENGLISH;
    };

    match language.as_str() {
        "zh" => {
            let script = parts
                .iter()
                .skip(1)
                .find(|part| part.len() == 4)
                .map(|part| part.to_ascii_lowercase());
            if matches!(script.as_deref(), Some("hant")) {
                return &TRADITIONAL_CHINESE;
            }
            if matches!(script.as_deref(), Some("hans")) {
                return &SIMPLIFIED_CHINESE;
            }
            if parts
                .iter()
                .skip(1)
                .any(|part| matches!(part.to_ascii_uppercase().as_str(), "TW" | "HK" | "MO"))
            {
                &TRADITIONAL_CHINESE
            } else {
                &SIMPLIFIED_CHINESE
            }
        }
        "ja" => &JAPANESE,
        "ko" => &KOREAN,
        "de" => &GERMAN,
        "fr" => &FRENCH,
        "es" => &SPANISH,
        "pt" => &BRAZILIAN_PORTUGUESE,
        _ => &ENGLISH,
    }
}

fn system_locale(cx: &App) -> &'static LocaleSpec {
    if let Some(snapshot) = cx.try_global::<SystemLocale>() {
        return snapshot
            .0
            .as_deref()
            .map(locale_for_system_tag)
            .unwrap_or(&ENGLISH);
    }
    sys_locale::get_locale()
        .as_deref()
        .map(locale_for_system_tag)
        .unwrap_or(&ENGLISH)
}

pub(crate) fn system_language_autonym(cx: &App) -> &'static str {
    system_locale(cx).autonym
}

pub(crate) fn locale(cx: &App) -> &'static str {
    cx.global::<I18n>().locale
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn registry_contains_all_nine_unique_locales_and_valid_fluent_resources() {
        let mut ids = BTreeSet::new();
        for locale in LOCALES {
            assert!(ids.insert(locale.id), "duplicate locale {}", locale.id);
            let _ = build_bundle(locale);
        }
        assert_eq!(ids.len(), 9);
        assert_eq!(
            ids,
            BTreeSet::from([
                "de", "en-US", "es", "fr", "ja", "ko", "pt-BR", "zh-CN", "zh-TW"
            ])
        );
        assert_eq!(
            LOCALES
                .iter()
                .map(|locale| locale.component_locale)
                .collect::<Vec<_>>(),
            [
                "en", "zh-CN", "zh-TW", "ja", "ko", "de", "fr", "es", "pt-BR"
            ]
        );
    }

    #[test]
    fn every_translation_has_the_same_message_keys_and_arguments_as_english() {
        let english = message_signatures(ENGLISH.source);
        for locale in LOCALES {
            assert_eq!(
                message_signatures(locale.source),
                english,
                "key or argument mismatch for {}",
                locale.id
            );
        }
    }

    #[test]
    fn missing_localized_messages_fall_back_to_english() {
        let incomplete = LocaleSpec {
            id: "fr",
            component_locale: "fr",
            autonym: "Français",
            source: "app-title = Titre traduit\n",
        };
        let localized = build_bundle(&incomplete);
        let english = build_bundle(&ENGLISH);
        assert_eq!(
            format_message(&localized, "app-title", &FluentArgs::new()).as_deref(),
            Some("Titre traduit")
        );
        assert_eq!(
            format_message(&localized, "settings-pi-command", &FluentArgs::new())
                .or_else(|| format_message(&english, "settings-pi-command", &FluentArgs::new()))
                .as_deref(),
            Some("Pi executable")
        );
    }

    #[gpui_kit::test]
    fn system_selection_uses_the_saved_pre_override_locale(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(SystemLocale(Some("zh-Hant-TW".into())));
            apply(AppLanguage::English, cx);
            assert_eq!(locale(cx), "en-US");
            apply(AppLanguage::System, cx);
            assert_eq!(locale(cx), "zh-TW");
            assert_eq!(system_language_autonym(cx), "繁體中文");
            let mut args = FluentArgs::new();
            args.set("language", system_language_autonym(cx).to_owned());
            assert_eq!(
                t_with_args(cx, "setup-system-language", &args),
                "系統語言：\u{2068}繁體中文\u{2069}"
            );
        });
    }

    #[test]
    fn explicit_native_locale_tags_match_the_app_locale_registry() {
        assert_eq!(native_locale(AppLanguage::System), None);
        assert_eq!(native_locale(AppLanguage::English), Some("en-US"));
        assert_eq!(native_locale(AppLanguage::Chinese), Some("zh-CN"));
        assert_eq!(
            native_locale(AppLanguage::TraditionalChinese),
            Some("zh-TW")
        );
        assert_eq!(native_locale(AppLanguage::Japanese), Some("ja"));
        assert_eq!(native_locale(AppLanguage::Korean), Some("ko"));
        assert_eq!(native_locale(AppLanguage::German), Some("de"));
        assert_eq!(native_locale(AppLanguage::French), Some("fr"));
        assert_eq!(native_locale(AppLanguage::Spanish), Some("es"));
        assert_eq!(
            native_locale(AppLanguage::BrazilianPortuguese),
            Some("pt-BR")
        );
    }

    #[test]
    fn system_locale_resolution_honors_script_region_and_language() {
        let cases = [
            ("zh-Hant", "zh-TW", "繁體中文"),
            ("zh-Hant-CN", "zh-TW", "繁體中文"),
            ("zh-Hans-TW", "zh-CN", "简体中文"),
            ("zh_TW.UTF-8", "zh-TW", "繁體中文"),
            ("zh-HK", "zh-TW", "繁體中文"),
            ("zh-MO", "zh-TW", "繁體中文"),
            ("zh-SG", "zh-CN", "简体中文"),
            ("ja_JP", "ja", "日本語"),
            ("ko-KR", "ko", "한국어"),
            ("de-DE", "de", "Deutsch"),
            ("fr-CA", "fr", "Français"),
            ("es-MX", "es", "Español"),
            ("pt-PT", "pt-BR", "Português (Brasil)"),
            ("unknown", "en-US", "English"),
        ];
        for (tag, expected, autonym) in cases {
            let locale = locale_for_system_tag(tag);
            assert_eq!(locale.id, expected, "{tag}");
            assert_eq!(locale.autonym, autonym, "{tag}");
        }
    }

    #[gpui_kit::test]
    fn applying_the_same_language_does_not_notify_again(cx: &mut gpui_kit::TestAppContext) {
        use std::{cell::Cell, rc::Rc};
        cx.update(|cx| {
            gpui_kit::init(cx);
            apply(AppLanguage::Chinese, cx);
        });
        let count = Rc::new(Cell::new(0));
        let observed = count.clone();
        let _subscription =
            cx.update(|cx| cx.observe_global::<I18n>(move |_| observed.set(observed.get() + 1)));
        cx.update(|cx| {
            apply(AppLanguage::Chinese, cx);
            apply(AppLanguage::Chinese, cx);
        });
        assert_eq!(count.get(), 0);
        cx.update(|cx| apply(AppLanguage::English, cx));
        assert_eq!(count.get(), 1);
    }

    fn message_signatures(source: &str) -> BTreeMap<String, BTreeSet<String>> {
        let mut messages = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                current = None;
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if let Some((id, value)) = trimmed.split_once('=') {
                    let id = id.trim();
                    if id
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                    {
                        let id = id.to_owned();
                        messages.insert(id.clone(), variables(value));
                        current = Some(id);
                        continue;
                    }
                }
                current = None;
            } else if let Some(id) = &current {
                messages
                    .get_mut(id)
                    .expect("current message exists")
                    .extend(variables(trimmed));
            }
        }
        messages
    }

    fn variables(value: &str) -> BTreeSet<String> {
        value
            .match_indices('$')
            .filter_map(|(index, _)| {
                let name = value[index + 1..]
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
                    .collect::<String>();
                (!name.is_empty()).then_some(name)
            })
            .collect()
    }
}
