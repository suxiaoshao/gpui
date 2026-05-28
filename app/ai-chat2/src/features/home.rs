use crate::{
    app::menus,
    foundation::{self, assets::IconName},
    state,
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Side, h_flex,
    label::Label,
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarFooter, SidebarMenu},
};

pub(crate) struct HomeView {
    layout_state: Entity<state::AiChat2LayoutState>,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let layout_state = cx.global::<state::LayoutStateStore>().entity();

        Self {
            layout_state: layout_state.clone(),
            _subscriptions: vec![cx.observe(&layout_state, |_state, _layout, cx| {
                cx.notify();
            })],
        }
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_width = self.layout_state.read(cx).sidebar_width();
        let layout_state = self.layout_state.clone();

        div().size_full().min_h_0().overflow_hidden().child(
            h_resizable("ai-chat2-home-layout")
                .on_resize(move |resizable_state, _window, cx| {
                    let width = resizable_state
                        .read(cx)
                        .sizes()
                        .first()
                        .copied()
                        .unwrap_or(state::layout::SIDEBAR_DEFAULT_WIDTH);
                    layout_state.update(cx, |layout, cx| {
                        layout.set_sidebar_width(width, cx);
                    });
                })
                .child(
                    resizable_panel()
                        .size(sidebar_width)
                        .size_range(
                            state::layout::SIDEBAR_MIN_WIDTH..state::layout::SIDEBAR_MAX_WIDTH,
                        )
                        .child(render_sidebar(cx)),
                )
                .child(resizable_panel().child(div().size_full().min_w_0())),
        )
    }
}

fn render_sidebar(cx: &mut Context<HomeView>) -> impl IntoElement {
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
