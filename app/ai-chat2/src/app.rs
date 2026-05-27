use crate::{database, errors::AiChat2Error, foundation, state};
use gpui::*;
use gpui_component::{ActiveTheme, Root, TitleBar};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub(crate) static APP_NAME: &str = "top.sushao.ai-chat2";
const APP_TITLE: &str = "AI Chat 2";
const KEY_CONTEXT: &str = "AiChat2Root";

actions!(ai_chat2, [Quit]);

pub(crate) fn run() -> crate::errors::AiChat2Result<()> {
    init_tracing();

    let app = gpui_platform::application().with_assets(foundation::Assets::default());
    app.run(|cx: &mut App| {
        if let Err(err) = init(cx) {
            event!(Level::ERROR, error = ?err, "failed to initialize ai-chat2");
            eprintln!("failed to initialize {APP_TITLE}: {err}");
            cx.quit();
            return;
        }

        if let Err(err) = open_main_window(cx) {
            event!(Level::ERROR, error = ?err, "failed to open ai-chat2 main window");
            eprintln!("failed to open {APP_TITLE} window: {err}");
            cx.quit();
        }
    });

    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(fmt::layer().with_filter(LevelFilter::INFO))
        .try_init();
}

fn init(cx: &mut App) -> crate::errors::AiChat2Result<()> {
    gpui_component::init(cx);
    state::config::init(cx)?;
    database::init_store(cx)?;
    state::config::init_app_settings(cx)?;
    state::theme::init(cx);
    foundation::init_i18n(cx);
    if let Err(err) = state::hotkey::init(cx) {
        event!(Level::ERROR, error = ?err, "failed to initialize ai-chat2 hotkeys");
    }
    let hotkey_diagnostics = state::GlobalHotkeyState::diagnostics_snapshot(cx);
    event!(
        Level::INFO,
        temporary_hotkey = ?hotkey_diagnostics.temporary_hotkey,
        registered_shortcuts = hotkey_diagnostics.registered_shortcuts.len(),
        registration_errors = hotkey_diagnostics.registration_errors.len(),
        "ai-chat2 hotkey diagnostics"
    );

    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);
    Ok(())
}

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit ai-chat2 by action");
    cx.quit();
}

fn open_main_window(cx: &mut App) -> Result<WindowHandle<Root>, AiChat2Error> {
    let title = cx.global::<foundation::I18n>().t("app-title");
    cx.open_window(
        WindowOptions {
            titlebar: Some(main_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| AppRootView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .map_err(|err| AiChat2Error::Window(err.to_string()))
}

fn main_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

struct AppRootView {
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl AppRootView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        Self {
            focus_handle,
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
                cx.observe_global_in::<state::AiChat2AppSettings>(window, |_state, window, cx| {
                    state::theme::apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
            ],
        }
    }
}

impl Render for AppRootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = cx.global::<foundation::I18n>().t("app-title");
        window.set_window_title(&title);

        div()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_TITLE, main_titlebar_options};
    use gpui_component::TitleBar;

    #[test]
    fn main_window_uses_component_titlebar_options() {
        let titlebar = main_titlebar_options(APP_TITLE);
        let expected = TitleBar::title_bar_options();

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some(APP_TITLE)
        );
        assert_eq!(titlebar.appears_transparent, expected.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            expected.traffic_light_position
        );
    }
}
