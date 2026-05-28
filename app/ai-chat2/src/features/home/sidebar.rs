use crate::{
    app::menus,
    foundation::{self, assets::IconName},
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Side, h_flex,
    label::Label,
    sidebar::{Sidebar, SidebarFooter, SidebarMenu},
};

pub(crate) struct HomeSidebar;

impl HomeSidebar {
    pub(crate) fn new(_: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for HomeSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_label = sidebar_settings_label(cx.global::<foundation::I18n>());

        Sidebar::<SidebarMenu>::new("ai-chat2-main-sidebar")
            .side(Side::Left)
            .w_full()
            .border_r_0()
            .collapsible(false)
            .collapsed(false)
            .footer(
                SidebarFooter::new().child(
                    h_flex()
                        .id("sidebar-settings")
                        .w_full()
                        .min_w_0()
                        .h_7()
                        .px_2()
                        .items_center()
                        .gap_2()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .hover(|this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                        .on_click(cx.listener(|_this, _event, window, cx| {
                            window.dispatch_action(menus::OpenSettings.boxed_clone(), cx);
                        }))
                        .child(Icon::new(IconName::Settings).size_4())
                        .child(Label::new(settings_label).text_sm().truncate()),
                ),
            )
    }
}

fn sidebar_settings_label(i18n: &foundation::I18n) -> String {
    i18n.t("app-menu-settings")
}

#[cfg(test)]
mod tests {
    use super::sidebar_settings_label;
    use crate::foundation::I18n;

    #[test]
    fn sidebar_settings_label_uses_existing_i18n_key() {
        assert_eq!(
            sidebar_settings_label(&I18n::english_for_test()),
            "Settings"
        );
        assert_eq!(
            sidebar_settings_label(&I18n::for_locale_tag("zh-CN")),
            "设置"
        );
    }
}
