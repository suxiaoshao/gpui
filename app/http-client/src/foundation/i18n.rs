pub use app_i18n::I18n;
pub(crate) fn init_i18n(cx: &mut gpui_kit::App) {
    cx.set_global(I18n::detected(
        include_str!("../../locales/en-US/main.ftl"),
        include_str!("../../locales/zh-CN/main.ftl"),
    ));
}
