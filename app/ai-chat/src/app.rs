use crate::errors::{AiChatError, AiChatResult};
use crate::features::{home::HomeView, settings::SettingsView};
use crate::{components, database, features, foundation, state};
use foundation::I18n;
use gpui::*;
use gpui_component::input;
use gpui_component::{Root, TitleBar};
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use window_ext::WindowExt;

pub(crate) mod menus;
pub(crate) mod tray;

pub(crate) static APP_NAME: &str = "top.sushao.ai-chat";
const MAIN_WINDOW_FALLBACK_SIZE: Size<Pixels> = size(px(1536.), px(864.));

#[cfg(feature = "dhat-heap")]
mod profiling {
    use super::*;

    const PROFILE_EXIT_AFTER_SECS_ENV: &str = "AI_CHAT_PROFILE_EXIT_AFTER_SECS";
    const PROFILE_OUTPUT_FILE: &str = "dhat-heap.json";

    #[global_allocator]
    static ALLOC: dhat::Alloc = dhat::Alloc;

    thread_local! {
        static HEAP_PROFILER: std::cell::RefCell<Option<dhat::Profiler>> =
            const { std::cell::RefCell::new(None) };
    }

    fn profile_exit_after() -> Option<std::time::Duration> {
        let secs = std::env::var(PROFILE_EXIT_AFTER_SECS_ENV)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;
        (secs > 0).then_some(std::time::Duration::from_secs(secs))
    }

    pub(super) fn init() {
        HEAP_PROFILER.with(|profiler| {
            *profiler.borrow_mut() = Some(
                dhat::Profiler::builder()
                    .file_name(PROFILE_OUTPUT_FILE)
                    .build(),
            );
        });
    }

    pub(super) fn flush() {
        HEAP_PROFILER.with(|profiler| {
            drop(profiler.borrow_mut().take());
        });
    }

    pub(super) fn schedule_auto_quit(cx: &mut App) {
        let Some(delay) = profile_exit_after() else {
            return;
        };

        cx.spawn(async move |cx| {
            smol::Timer::after(delay).await;
            event!(
                Level::INFO,
                seconds = delay.as_secs(),
                "profiling auto quit triggered"
            );
            flush();
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

#[cfg(not(feature = "dhat-heap"))]
mod profiling {
    use super::*;

    pub(super) fn init() {}

    pub(super) fn flush() {}

    pub(super) fn schedule_auto_quit(_: &mut App) {}
}

pub(crate) fn quit_app(cx: &mut App) {
    state::workspace::save_now(cx);
    profiling::flush();
    event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([KeyBinding::new(
        "shift-enter",
        input::Enter { secondary: true },
        Some("Input"),
    )]);

    state::theme::init(cx);
    state::config::init(cx);
    foundation::init_i18n(cx);
    menus::init(cx);
    menus::sync_app_menus(cx);
    #[cfg(target_os = "macos")]
    menus::ensure_localized_window_menu_registered();
    cx.activate(true);

    database::init_store(cx);
    components::init(cx);
    features::init(cx);
    state::chat::init_global(cx);
    features::hotkey::init(cx);
}

fn register_main_window_close_behavior(window: &mut Window, cx: &mut App) {
    window.on_window_should_close(cx, |window, _cx| {
        if should_hide_main_window_on_close() {
            if let Err(err) = window.hide() {
                event!(Level::ERROR, error = ?err, "hide main window on close failed");
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

pub(crate) fn find_window_by_view<V: 'static>(cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let root_view = root.read(cx).ok()?.view().clone();
        root_view.downcast::<V>().ok().map(|_| root)
    })
}

pub(crate) fn with_root_view<V: 'static, R>(
    root: &mut Root,
    cx: &mut Context<Root>,
    callback: impl FnOnce(Entity<V>, &mut Context<Root>) -> R,
) -> Option<R> {
    let view = root.view().clone().downcast::<V>().ok()?;
    Some(callback(view, cx))
}

fn focus_main_window_chat_form(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let _ = with_root_view::<HomeView, _>(root, cx, |home, cx| {
        home.update(cx, |home, cx| home.focus_chat_form(window, cx));
    });
}

fn reveal_main_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Err(err) = window.show() {
        event!(Level::ERROR, error = ?err, "show main window failed");
    }
    window.activate_window();
    focus_main_window_chat_form(root, window, cx);
}

pub(crate) fn open_main_window(cx: &mut App) -> Result<WindowHandle<Root>, anyhow::Error> {
    let title = cx.global::<I18n>().t("app-title");
    let placement = state::workspace::restored_window_placement(
        state::workspace::WindowPlacementKind::Main,
        MAIN_WINDOW_FALLBACK_SIZE,
        cx,
    );
    cx.open_window(
        WindowOptions {
            window_bounds: Some(placement.window_bounds),
            display_id: placement.display_id,
            titlebar: Some(main_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        create_main_root,
    )
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
            let _ = with_root_view::<HomeView, _>(root, cx, |home, cx| {
                home.update(cx, |home, cx| home.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<SettingsView, _>(root, cx, |settings, cx| {
                settings.update(cx, |settings, cx| settings.reload_app_menu_bar(cx));
            });
        });
    }
}

pub(crate) fn show_or_create_main_window(cx: &mut App) {
    if let Some(window) = find_main_window(cx) {
        if let Err(err) = window.update(cx, |root, window, cx| {
            reveal_main_window(root, window, cx);
        }) {
            event!(Level::ERROR, error = ?err, "update main window failed");
        }
        return;
    }

    match open_main_window(cx) {
        Ok(window) => {
            if let Err(err) = window.update(cx, |root, window, cx| {
                reveal_main_window(root, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "activate new main window failed");
            }
        }
        Err(err) => {
            event!(Level::ERROR, error = ?err, "open main window failed");
        }
    }
}

pub(crate) fn open_temporary_window(cx: &mut App) {
    prepare_temporary_window_action(cx);
    cx.defer(features::hotkey::open_temporary_window);
}

pub(crate) fn toggle_temporary_window(cx: &mut App) {
    prepare_temporary_window_action(cx);
    cx.defer(features::hotkey::toggle_temporary_window);
}

fn prepare_temporary_window_action(cx: &mut App) {
    #[cfg(target_os = "macos")]
    features::hotkey::record_front_app_for_temporary_window(cx);
    cx.activate(true);
}

fn get_logs_dir() -> AiChatResult<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = dirs_next::home_dir()
        .ok_or(AiChatError::LogFileNotFound)
        .map(|dir| dir.join("Library/Logs").join(APP_NAME));

    #[cfg(not(target_os = "macos"))]
    let path = dirs_next::data_local_dir()
        .ok_or(AiChatError::LogFileNotFound)
        .map(|dir| dir.join(APP_NAME).join("logs"));

    if let Ok(path) = &path
        && !path.exists()
    {
        create_dir_all(path).map_err(|_| AiChatError::LogFileNotFound)?;
    }
    path
}

fn init_tracing() -> AiChatResult<()> {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .with_writer(
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(get_logs_dir()?.join("data.log"))
                        .map_err(|_| AiChatError::LogFileNotFound)?,
                )
                .with_filter(LevelFilter::INFO),
        )
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .event_format(fmt::format().pretty())
                .with_filter(LevelFilter::INFO),
        )
        .init();
    Ok(())
}

pub(crate) fn run() -> AiChatResult<()> {
    init_tracing()?;
    event!(Level::INFO, "startup begin");

    profiling::init();

    let span = tracing::info_span!("ai-chat");
    let _enter = span.enter();
    let app = gpui_platform::application().with_assets(foundation::Assets::default());
    app.on_reopen(show_or_create_main_window);
    event!(Level::INFO, "app created");

    app.run(|cx: &mut App| {
        init(cx);
        profiling::schedule_auto_quit(cx);
        show_or_create_main_window(cx);
        tray::init(cx);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{main_titlebar_options, should_hide_main_window_on_close};
    use gpui_component::TitleBar;

    #[::core::prelude::v1::test]
    fn main_window_close_behavior_matches_platform_support() {
        assert_eq!(
            should_hide_main_window_on_close(),
            cfg!(any(target_os = "macos", target_os = "windows"))
        );
    }

    #[test]
    fn main_window_uses_component_titlebar_options() {
        let titlebar = main_titlebar_options("AI Chat");
        let expected = TitleBar::title_bar_options();

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some("AI Chat")
        );
        assert_eq!(titlebar.appears_transparent, expected.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            expected.traffic_light_position
        );
    }
}
