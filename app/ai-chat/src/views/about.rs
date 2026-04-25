use crate::{app::APP_NAME, app::menus, i18n::I18n};
use fluent_bundle::FluentArgs;
use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, TitlebarOptions, Window,
    WindowBounds, WindowKind, WindowOptions, img, px,
};
#[cfg(target_os = "macos")]
use gpui::{Point, point};
use gpui_component::{ActiveTheme, Sizable, button::Button, h_flex, label::Label, v_flex};

const ABOUT_APP_ICON: &str = "build-assets/icon/app-icon.ico";

pub(crate) fn open_about_window(cx: &mut App) {
    if let Some(existing) = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<AboutWindow>())
    {
        let _ = existing.update(cx, |view, window, cx| {
            window.activate_window();
            view.focus_handle.focus(window, cx);
        });
        return;
    }

    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(about_window_size(), cx)),
            titlebar: Some(about_titlebar_options(cx.global::<I18n>())),
            is_resizable: false,
            is_minimizable: false,
            kind: WindowKind::Normal,
            app_id: Some(APP_NAME.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(AboutWindow::new);
            let focus_handle = view.read(cx).focus_handle.clone();
            window.activate_window();
            focus_handle.focus(window, cx);
            view
        },
    );
}

fn about_window_size() -> gpui::Size<gpui::Pixels> {
    gpui::Size {
        width: px(320.),
        height: px(360.),
    }
}

#[cfg(target_os = "macos")]
fn about_titlebar_options(_: &I18n) -> TitlebarOptions {
    TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(about_traffic_light_position()),
    }
}

#[cfg(not(target_os = "macos"))]
fn about_titlebar_options(i18n: &I18n) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(about_window_title(i18n).into()),
        ..Default::default()
    }
}

#[cfg(not(target_os = "macos"))]
fn about_window_title(i18n: &I18n) -> String {
    let mut args = FluentArgs::new();
    args.set("app_name", i18n.t("app-title"));
    i18n.t_with_args("app-about-window-title", &args)
}

#[cfg(target_os = "macos")]
fn about_traffic_light_position() -> Point<gpui::Pixels> {
    point(px(12.), px(12.))
}

pub(crate) struct AboutWindow {
    focus_handle: FocusHandle,
    version: SharedString,
    description: SharedString,
    repository_url: SharedString,
}

impl AboutWindow {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: cx.global::<I18n>().t("tray-about-comments").into(),
            repository_url: env!("CARGO_PKG_REPOSITORY").into(),
        }
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();

        v_flex()
            .id("about-window")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .size_full()
            .items_center()
            .justify_start()
            .gap_5()
            .pt_12()
            .pb_5()
            .px_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                v_flex()
                    .items_center()
                    .gap_4()
                    .child(img(ABOUT_APP_ICON).size(px(72.)).flex_shrink_0())
                    .child(
                        Label::new(i18n.t("app-title"))
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(
                        Label::new(self.description.clone())
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new({
                            let mut args = FluentArgs::new();
                            args.set("version", self.version.as_ref());
                            i18n.t_with_args("app-about-version", &args)
                        })
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD),
                    ),
            )
            .child(
                h_flex().pt_2().child(
                    Button::new("about-github")
                        .label(i18n.t("app-about-github"))
                        .small()
                        .on_click({
                            let repository_url = self.repository_url.clone();
                            move |_, _, cx: &mut App| cx.open_url(&repository_url)
                        }),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_window_uses_compact_size() {
        let size = about_window_size();

        assert_eq!(size.width, px(320.));
        assert_eq!(size.height, px(360.));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_about_titlebar_is_transparent_with_visible_traffic_lights() {
        let titlebar = about_titlebar_options(&I18n::english_for_test());

        assert!(titlebar.title.is_none());
        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(about_traffic_light_position())
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_about_titlebar_keeps_system_title() {
        let titlebar = about_titlebar_options(&I18n::english_for_test());

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some("About AI Chat")
        );
        assert!(!titlebar.appears_transparent);
        assert!(titlebar.traffic_light_position.is_none());
    }
}
