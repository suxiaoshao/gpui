pub(crate) use app_i18n::I18n;
pub(crate) use gpui_lucide::IconName;
#[cfg(test)]
pub(crate) fn for_locale_tag(tag: &str) -> I18n {
    I18n::for_locale_tag(
        tag,
        include_str!("../../../locales/en-US/main.ftl"),
        include_str!("../../../locales/zh-CN/main.ftl"),
    )
}
