use crate::state::config::AppLanguage;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui_kit::{App, Global};
pub(crate) struct I18n {
    bundle: FluentBundle<FluentResource>,
    locale: &'static str,
}
impl Global for I18n {}
pub(crate) fn apply(language: AppLanguage, cx: &mut App) {
    let chinese = match language {
        AppLanguage::Chinese => true,
        AppLanguage::English => false,
        AppLanguage::System => system_is_chinese(),
    };
    let (locale, source) = if chinese {
        ("zh-CN", include_str!("../../locales/zh-CN/main.ftl"))
    } else {
        ("en-US", include_str!("../../locales/en-US/main.ftl"))
    };
    if cx
        .try_global::<I18n>()
        .is_some_and(|current| current.locale == locale)
    {
        return;
    }
    let mut bundle = FluentBundle::new(vec![locale.parse().expect("locale")]);
    bundle
        .add_resource(FluentResource::try_new(source.into()).expect("Fluent resource"))
        .expect("Fluent keys");
    cx.set_global(I18n { bundle, locale });
    gpui_kit::component::set_locale(if chinese { "zh-CN" } else { "en" });
}
pub(crate) fn t(cx: &App, key: &str) -> String {
    t_with_args(cx, key, &FluentArgs::new())
}
pub(crate) fn t_with_args(cx: &App, key: &str, args: &FluentArgs<'_>) -> String {
    let bundle = &cx.global::<I18n>().bundle;
    bundle
        .get_message(key)
        .and_then(|m| m.value())
        .map(|v| {
            bundle
                .format_pattern(v, Some(args), &mut vec![])
                .into_owned()
        })
        .unwrap_or_else(|| key.into())
}

pub(crate) fn system_is_chinese() -> bool {
    sys_locale::get_locale().is_some_and(|s| s.starts_with("zh"))
}

pub(crate) fn locale(cx: &App) -> &'static str {
    cx.global::<I18n>().locale
}

#[cfg(test)]
mod notification_tests {
    use super::*;
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
}
