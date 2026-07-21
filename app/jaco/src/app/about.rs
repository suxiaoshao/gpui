use crate::{
    app::{
        APP_NAME, menus,
        title_bar_menu::{TitleBarAppMenuBar, title_bar_leading},
    },
    foundation::{self, I18n, assets::APP_ICON_ASSET_PATH},
    state,
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Root, Sizable, StyledExt, TitleBar, button::Button, h_flex, label::Label, v_flex,
};
use window_ext::{NativeWindowHandle, WindowExt};

const ABOUT_CONTEXT: &str = "JacoAboutWindow";
const ABOUT_WINDOW_SIZE: Size<Pixels> = size(px(360.), px(380.));

pub(crate) fn open_about_window(cx: &mut App) {
    if let Some(window) = find_about_window(cx) {
        let mut reveal_window = None;
        if let Err(err) = window.update(cx, |root, window, cx| {
            if !window.is_window_active() {
                match window.native_window_handle() {
                    Ok(handle) => reveal_window = Some(handle),
                    Err(err) => {
                        tracing::event!(
                            tracing::Level::ERROR,
                            error = ?err,
                            "get jaco about window handle failed"
                        );
                    }
                }
            }
            focus_about_window(root, window, cx);
        }) {
            tracing::event!(
                tracing::Level::ERROR,
                error = ?err,
                "activate jaco about window failed"
            );
        }
        if let Some(native_window) = reveal_window {
            schedule_about_window_reveal(native_window, cx);
        }
        return;
    }

    let title = about_window_title(cx.global::<I18n>());
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(about_window_size(), cx)),
            titlebar: Some(about_titlebar_options(title)),
            app_owns_titlebar_drag: true,
            window_background: WindowBackgroundAppearance::Opaque,
            is_resizable: false,
            is_minimizable: false,
            kind: WindowKind::Normal,
            app_id: Some(APP_NAME.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| AboutWindow::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    );

    match result {
        Ok(window) => {
            let _ = window.update(cx, |root, window, cx| {
                focus_about_window(root, window, cx);
            });
        }
        Err(err) => {
            tracing::event!(
                tracing::Level::ERROR,
                error = ?err,
                "open jaco about window failed"
            );
        }
    }
}

fn find_about_window(cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let root_view = root.read(cx).ok()?.view().clone();
        root_view.downcast::<AboutWindow>().ok().map(|_| root)
    })
}

fn focus_about_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let _ = root.view().clone().downcast::<AboutWindow>().map(|view| {
        view.update(cx, |view, cx| {
            view.focus_handle.focus(window, cx);
        });
    });
}

fn schedule_about_window_reveal(native_window: NativeWindowHandle, cx: &mut App) {
    cx.defer(move |_| {
        if let Err(err) = native_window.show() {
            tracing::event!(
                tracing::Level::ERROR,
                error = ?err,
                "show jaco about window failed"
            );
        }
    });
}

fn about_window_size() -> Size<Pixels> {
    ABOUT_WINDOW_SIZE
}

fn about_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

fn about_window_title(i18n: &I18n) -> String {
    let mut args = FluentArgs::new();
    args.set("app_name", i18n.t("app-title"));
    i18n.t_with_args("app-about-window-title", &args)
}

#[derive(Clone, Debug)]
struct AboutMetadata {
    version: SharedString,
    license: SharedString,
    repository_url: SharedString,
}

fn about_metadata() -> AboutMetadata {
    AboutMetadata {
        version: env!("CARGO_PKG_VERSION").into(),
        license: env!("CARGO_PKG_LICENSE").into(),
        repository_url: env!("CARGO_PKG_REPOSITORY").into(),
    }
}

pub(crate) struct AboutWindow {
    focus_handle: FocusHandle,
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    metadata: AboutMetadata,
    _subscriptions: Vec<Subscription>,
}

impl AboutWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let app_menu_bar = TitleBarAppMenuBar::new(cx);
        let config_store = state::config::store(cx);

        Self {
            focus_handle,
            app_menu_bar,
            metadata: about_metadata(),
            _subscriptions: vec![
                cx.observe_window_appearance(window, |_state, window, cx| {
                    state::theme::apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<state::theme::SystemAccentThemeState>(
                    window,
                    |_state, window, cx| {
                        state::theme::apply_current_theme(window, cx);
                        cx.refresh_windows();
                    },
                ),
                config_store.observe_select_in(
                    cx,
                    window,
                    |config| {
                        (
                            config.app_settings.language,
                            config.app_settings.theme.clone(),
                        )
                    },
                    |this, _settings, window, cx| {
                        foundation::init_i18n(cx);
                        menus::sync_app_menus(cx);
                        state::theme::apply_current_theme(window, cx);
                        this.reload_app_menu_bar(cx);
                        cx.refresh_windows();
                    },
                ),
            ],
        }
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        self.app_menu_bar
            .update(cx, |app_menu_bar, cx| app_menu_bar.reload(cx));
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let title = about_window_title(i18n);
        window.set_window_title(&title);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(ABOUT_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().tokens.background.background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .child(
                div()
                    .child(
                        TitleBar::new()
                            .child(title_bar_content(self.app_menu_bar.clone(), title.clone())),
                    )
                    .flex_initial(),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .gap_5()
                    .px_8()
                    .pb_8()
                    .child(
                        v_flex()
                            .items_center()
                            .gap_3()
                            .child(img(APP_ICON_ASSET_PATH).size(px(72.)).flex_shrink_0())
                            .child(
                                Label::new(i18n.t("app-title"))
                                    .text_size(px(20.))
                                    .font_semibold(),
                            )
                            .child(
                                Label::new(i18n.t("app-about-description"))
                                    .text_size(px(13.))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center(),
                            )
                            .child(
                                Label::new({
                                    let mut args = FluentArgs::new();
                                    args.set("version", self.metadata.version.as_ref());
                                    i18n.t_with_args("app-about-version", &args)
                                })
                                .text_size(px(13.))
                                .font_medium(),
                            )
                            .child(
                                Label::new({
                                    let mut args = FluentArgs::new();
                                    args.set("license", self.metadata.license.as_ref());
                                    i18n.t_with_args("app-about-license", &args)
                                })
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        Button::new("about-github")
                            .label(i18n.t("app-about-github"))
                            .small()
                            .on_click({
                                let repository_url = self.metadata.repository_url.clone();
                                move |_, _, cx: &mut App| cx.open_url(&repository_url)
                            }),
                    ),
            )
    }
}

fn title_bar_content(
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    title: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .when(menus::should_render_component_menu_bar(), |this| {
            this.child(title_bar_leading(app_menu_bar))
        })
        .child(title_bar_title(title))
}

fn title_bar_title(title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .justify_center()
        .overflow_hidden()
        .pr_2()
        .child(Label::new(title).text_sm().font_medium().truncate())
}

#[cfg(test)]
mod tests {
    use super::{about_metadata, about_titlebar_options, about_window_size, about_window_title};
    use crate::foundation::I18n;
    use gpui::px;

    #[test]
    fn about_window_uses_compact_non_resizable_window() {
        let size = about_window_size();
        let titlebar = about_titlebar_options("About Jaco");

        assert_eq!(size.width, px(360.));
        assert_eq!(size.height, px(380.));
        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some("About Jaco")
        );
    }

    #[test]
    fn about_title_is_localized() {
        let english = I18n::english_for_test();
        let chinese = I18n::for_locale_tag("zh-CN");

        assert_eq!(about_window_title(&english), "About Jaco");
        assert_eq!(about_window_title(&chinese), "关于 Jaco");
    }

    #[test]
    fn about_metadata_uses_package_constants() {
        let metadata = about_metadata();

        assert_eq!(metadata.version.as_ref(), env!("CARGO_PKG_VERSION"));
        assert_eq!(metadata.license.as_ref(), env!("CARGO_PKG_LICENSE"));
        assert_eq!(
            metadata.repository_url.as_ref(),
            env!("CARGO_PKG_REPOSITORY")
        );
    }
}
