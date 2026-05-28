pub(crate) mod about;
pub(crate) mod menus;
pub(crate) mod placeholder_windows;
pub(crate) mod title_bar_menu;

use crate::features::home::HomeView;
use crate::{database, errors::AiChat2Error, foundation, state};
use gpui::*;
use gpui_component::{Root, TitleBar};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use window_ext::WindowExt;

pub(crate) static APP_NAME: &str = "top.sushao.ai-chat2";
const APP_TITLE: &str = "AI Chat 2";

pub(crate) fn run() -> crate::errors::AiChat2Result<()> {
    init_tracing();

    let app = gpui_platform::application().with_assets(foundation::Assets::default());
    app.on_reopen(show_or_create_main_window);
    app.run(|cx: &mut App| {
        if let Err(err) = init(cx) {
            event!(Level::ERROR, error = ?err, "failed to initialize ai-chat2");
            eprintln!("failed to initialize {APP_TITLE}: {err}");
            cx.quit();
            return;
        }

        show_or_create_main_window(cx);
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
    state::layout::init(cx)?;
    database::init_store(cx)?;
    state::config::init_app_settings(cx)?;
    state::theme::init(cx);
    foundation::init_i18n(cx);
    title_bar_menu::init(cx);
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

    menus::init(cx);
    menus::sync_app_menus(cx);
    reload_app_menu_bars(cx);
    #[cfg(target_os = "macos")]
    menus::ensure_localized_window_menu_registered();

    cx.activate(true);
    Ok(())
}

pub(crate) fn quit_app(cx: &mut App) {
    event!(Level::INFO, "quit ai-chat2 by action");
    cx.quit();
}

fn register_main_window_close_behavior(window: &mut Window, cx: &mut App) {
    window.on_window_should_close(cx, |window, _cx| {
        if should_hide_main_window_on_close() {
            if let Err(err) = window.hide() {
                event!(Level::ERROR, error = ?err, "hide ai-chat2 main window failed");
                return true;
            }
            return false;
        }

        true
    });
}

const fn should_hide_main_window_on_close() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

fn create_main_root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    register_main_window_close_behavior(window, cx);
    let view = cx.new(|cx| HomeView::new(window, cx));
    cx.new(|cx| Root::new(view, window, cx))
}

fn find_window_by_view<V: 'static>(cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let root_view = root.read(cx).ok()?.view().clone();
        root_view.downcast::<V>().ok().map(|_| root)
    })
}

fn with_root_view<V: 'static, R>(
    root: &mut Root,
    cx: &mut Context<Root>,
    callback: impl FnOnce(Entity<V>, &mut Context<Root>) -> R,
) -> Option<R> {
    let view = root.view().clone().downcast::<V>().ok()?;
    Some(callback(view, cx))
}

fn reveal_main_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Err(err) = window.show() {
        event!(Level::ERROR, error = ?err, "show ai-chat2 main window failed");
    }
    window.activate_window();

    let _ = with_root_view::<HomeView, _>(root, cx, |view, cx| {
        view.update(cx, |view, cx| view.focus(window, cx));
    });
}

pub(crate) fn open_main_window(cx: &mut App) -> Result<WindowHandle<Root>, AiChat2Error> {
    let title = cx.global::<foundation::I18n>().t("app-title");
    cx.open_window(
        WindowOptions {
            titlebar: Some(main_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        create_main_root,
    )
    .map_err(|err| AiChat2Error::Window(err.to_string()))
}

fn main_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

pub(crate) fn find_main_window(cx: &App) -> Option<WindowHandle<Root>> {
    find_window_by_view::<HomeView>(cx)
}

pub(crate) fn reload_app_menu_bars(cx: &mut App) {
    let roots = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<Root>())
        .collect::<Vec<_>>();

    for root in roots {
        let _ = root.update(cx, |root, _window, cx| {
            let _ = with_root_view::<HomeView, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<about::AboutWindow, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<placeholder_windows::PlaceholderWindow, _>(
                root,
                cx,
                |view, cx| {
                    view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
                },
            );
        });
    }
}

pub(crate) fn show_or_create_main_window(cx: &mut App) {
    if let Some(window) = find_main_window(cx) {
        if let Err(err) = window.update(cx, |root, window, cx| {
            reveal_main_window(root, window, cx);
        }) {
            event!(Level::ERROR, error = ?err, "update ai-chat2 main window failed");
        }
        return;
    }

    match open_main_window(cx) {
        Ok(window) => {
            if let Err(err) = window.update(cx, |root, window, cx| {
                reveal_main_window(root, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "activate new ai-chat2 main window failed");
            }
        }
        Err(err) => {
            event!(Level::ERROR, error = ?err, "open ai-chat2 main window failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_TITLE, main_titlebar_options, should_hide_main_window_on_close};
    use gpui_component::TitleBar;

    #[test]
    fn main_window_close_behavior_matches_platform_support() {
        assert_eq!(
            should_hide_main_window_on_close(),
            cfg!(any(target_os = "macos", target_os = "windows"))
        );
    }

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
