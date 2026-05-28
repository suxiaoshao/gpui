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
use gpui_component::{ActiveTheme, Root, StyledExt, TitleBar, h_flex, label::Label, v_flex};
use window_ext::WindowExt;

const PLACEHOLDER_CONTEXT: &str = "AiChat2PlaceholderWindow";
const ABOUT_WINDOW_SIZE: Size<Pixels> = size(px(360.), px(380.));
const SETTINGS_WINDOW_SIZE: Size<Pixels> = size(px(760.), px(520.));
const TEMPORARY_WINDOW_SIZE: Size<Pixels> = size(px(680.), px(420.));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaceholderKind {
    About,
    Settings,
    Temporary,
}

pub(crate) fn open_about_window(cx: &mut App) {
    open_placeholder_window(PlaceholderKind::About, cx);
}

pub(crate) fn open_settings_window(cx: &mut App) {
    open_placeholder_window(PlaceholderKind::Settings, cx);
}

pub(crate) fn open_temporary_window(cx: &mut App) {
    open_placeholder_window(PlaceholderKind::Temporary, cx);
}

fn open_placeholder_window(kind: PlaceholderKind, cx: &mut App) {
    if let Some(window) = find_placeholder_window(kind, cx) {
        if let Err(err) = window.update(cx, |root, window, cx| {
            reveal_placeholder_window(root, window, cx);
        }) {
            tracing::event!(
                tracing::Level::ERROR,
                error = ?err,
                kind = ?kind,
                "activate ai-chat2 placeholder window failed"
            );
        }
        return;
    }

    let title = placeholder_window_title(kind, cx.global::<I18n>());
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(kind.window_size(), cx)),
            titlebar: Some(placeholder_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Opaque,
            is_resizable: kind.is_resizable(),
            kind: WindowKind::Normal,
            app_id: Some(APP_NAME.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| PlaceholderWindow::new(kind, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    );

    match result {
        Ok(window) => {
            let _ = window.update(cx, |root, window, cx| {
                reveal_placeholder_window(root, window, cx);
            });
        }
        Err(err) => {
            tracing::event!(
                tracing::Level::ERROR,
                error = ?err,
                kind = ?kind,
                "open ai-chat2 placeholder window failed"
            );
        }
    }
}

fn find_placeholder_window(kind: PlaceholderKind, cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let is_kind = {
            let root_view = root.read(cx).ok()?.view().clone();
            let view = root_view.downcast::<PlaceholderWindow>().ok()?;
            view.read(cx).kind == kind
        };
        is_kind.then_some(root)
    })
}

fn reveal_placeholder_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Err(err) = window.show() {
        tracing::event!(
            tracing::Level::ERROR,
            error = ?err,
            "show ai-chat2 placeholder window failed"
        );
    }
    window.activate_window();

    let _ = root
        .view()
        .clone()
        .downcast::<PlaceholderWindow>()
        .map(|view| {
            view.update(cx, |view, cx| {
                view.focus_handle.focus(window, cx);
            });
        });
}

fn placeholder_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

fn placeholder_window_title(kind: PlaceholderKind, i18n: &I18n) -> String {
    match kind {
        PlaceholderKind::About => {
            let mut args = FluentArgs::new();
            args.set("app_name", i18n.t("app-title"));
            i18n.t_with_args("app-about-window-title", &args)
        }
        PlaceholderKind::Settings => i18n.t("placeholder-settings-title"),
        PlaceholderKind::Temporary => i18n.t("placeholder-temporary-title"),
    }
}

impl PlaceholderKind {
    const fn window_size(self) -> Size<Pixels> {
        match self {
            Self::About => ABOUT_WINDOW_SIZE,
            Self::Settings => SETTINGS_WINDOW_SIZE,
            Self::Temporary => TEMPORARY_WINDOW_SIZE,
        }
    }

    const fn is_resizable(self) -> bool {
        match self {
            Self::About => false,
            Self::Settings | Self::Temporary => true,
        }
    }

    fn heading(self, i18n: &I18n) -> String {
        match self {
            Self::About => i18n.t("placeholder-about-title"),
            Self::Settings => i18n.t("placeholder-settings-title"),
            Self::Temporary => i18n.t("placeholder-temporary-title"),
        }
    }

    fn body(self, i18n: &I18n) -> String {
        match self {
            Self::About => i18n.t("placeholder-about-body"),
            Self::Settings => i18n.t("placeholder-settings-body"),
            Self::Temporary => i18n.t("placeholder-temporary-body"),
        }
    }
}

pub(crate) struct PlaceholderWindow {
    kind: PlaceholderKind,
    focus_handle: FocusHandle,
    app_menu_bar: Entity<TitleBarAppMenuBar>,
    _subscriptions: Vec<Subscription>,
}

impl PlaceholderWindow {
    fn new(kind: PlaceholderKind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let app_menu_bar = TitleBarAppMenuBar::new(cx);

        Self {
            kind,
            focus_handle,
            app_menu_bar,
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
                cx.observe_global_in::<state::AiChat2AppSettings>(window, |this, window, cx| {
                    foundation::init_i18n(cx);
                    menus::sync_app_menus(cx);
                    state::theme::apply_current_theme(window, cx);
                    this.reload_app_menu_bar(cx);
                    cx.refresh_windows();
                }),
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

impl Focusable for PlaceholderWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PlaceholderWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let title = placeholder_window_title(self.kind, i18n);
        window.set_window_title(&title);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(PLACEHOLDER_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
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
                    .gap_4()
                    .px_8()
                    .child(
                        v_flex()
                            .items_center()
                            .gap_3()
                            .when(self.kind == PlaceholderKind::About, |this| {
                                this.child(img(APP_ICON_ASSET_PATH).size(px(72.)).flex_shrink_0())
                            })
                            .child(
                                Label::new(self.kind.heading(i18n))
                                    .text_size(px(20.))
                                    .font_semibold(),
                            )
                            .child(
                                Label::new(self.kind.body(i18n))
                                    .text_size(px(13.))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center(),
                            )
                            .when(self.kind == PlaceholderKind::About, |this| {
                                let mut args = FluentArgs::new();
                                args.set("version", env!("CARGO_PKG_VERSION"));
                                this.child(
                                    Label::new(i18n.t_with_args("app-about-version", &args))
                                        .text_size(px(13.))
                                        .font_medium(),
                                )
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
    use super::{PlaceholderKind, placeholder_window_title};
    use crate::foundation::I18n;
    use gpui::px;

    #[test]
    fn about_placeholder_uses_compact_non_resizable_window() {
        let size = PlaceholderKind::About.window_size();

        assert_eq!(size.width, px(360.));
        assert_eq!(size.height, px(380.));
        assert!(!PlaceholderKind::About.is_resizable());
    }

    #[test]
    fn placeholder_titles_are_localized() {
        let english = I18n::english_for_test();
        let chinese = I18n::for_locale_tag("zh-CN");

        assert_eq!(
            placeholder_window_title(PlaceholderKind::About, &english),
            "About AI Chat 2"
        );
        assert_eq!(
            placeholder_window_title(PlaceholderKind::Settings, &chinese),
            "设置"
        );
    }
}
