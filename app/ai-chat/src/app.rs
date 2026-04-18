use crate::errors::{AiChatError, AiChatResult};
use crate::views::home::HomeView;
use crate::{app_menus, assets, components, database, hotkey, i18n, state, tray, views};
use gpui::*;
use gpui_component::input;
use gpui_component::{Root, TitleBar};
use i18n::I18n;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use window_ext::WindowExt;

pub(crate) static APP_NAME: &str = "top.sushao.ai-chat";

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
    i18n::init_i18n(cx);
    app_menus::init(cx);
    cx.set_menus(app_menus::app_menus(cx.global::<I18n>()));
    #[cfg(target_os = "macos")]
    app_menus::ensure_localized_window_menu_registered();
    cx.activate(true);

    database::init_store(cx);
    components::init(cx);
    views::init(cx);
    state::chat::init_global(cx);
    hotkey::init(cx);
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
    cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..TitleBar::title_bar_options()
            }),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        create_main_root,
    )
}

pub(crate) fn find_main_window(cx: &App) -> Option<WindowHandle<Root>> {
    find_window_by_view::<HomeView>(cx)
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
    cx.defer(hotkey::open_temporary_window);
}

pub(crate) fn toggle_temporary_window(cx: &mut App) {
    prepare_temporary_window_action(cx);
    cx.defer(hotkey::toggle_temporary_window);
}

fn prepare_temporary_window_action(cx: &mut App) {
    #[cfg(target_os = "macos")]
    hotkey::record_front_app_for_temporary_window(cx);
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
    let app = gpui_platform::application().with_assets(assets::Assets::default());
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
    use super::should_hide_main_window_on_close;

    #[::core::prelude::v1::test]
    fn main_window_close_behavior_matches_platform_support() {
        assert_eq!(
            should_hide_main_window_on_close(),
            cfg!(any(target_os = "macos", target_os = "windows"))
        );
    }
}
