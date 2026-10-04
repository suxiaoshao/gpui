pub use app_i18n::I18n;
use gpui_kit::App;
const EN_US: &str = include_str!("../../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../../locales/zh-CN/main.ftl");

pub(crate) fn init_i18n(cx: &mut App) {
    cx.set_global(I18n::detected(EN_US, ZH_CN));
}
#[cfg(test)]
pub(crate) fn for_locale_tag(tag: &str) -> I18n {
    I18n::for_locale_tag(tag, EN_US, ZH_CN)
}
