use crate::state::config::AppLanguage;
use fluent_bundle::{FluentBundle, FluentResource};
use gpui_kit::{App, Global};
pub(crate) struct I18n {
    bundle: FluentBundle<FluentResource>,
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
    let mut bundle = FluentBundle::new(vec![locale.parse().expect("locale")]);
    bundle
        .add_resource(FluentResource::try_new(source.into()).expect("Fluent resource"))
        .expect("Fluent keys");
    cx.set_global(I18n { bundle });
    gpui_kit::component::set_locale(if chinese { "zh-CN" } else { "en" });
}
pub(crate) fn t(cx: &App, key: &str) -> String {
    let bundle = &cx.global::<I18n>().bundle;
    bundle
        .get_message(key)
        .and_then(|m| m.value())
        .map(|v| bundle.format_pattern(v, None, &mut vec![]).into_owned())
        .unwrap_or_else(|| key.into())
}

pub(crate) fn system_is_chinese() -> bool {
    sys_locale::get_locale().is_some_and(|s| s.starts_with("zh"))
}
