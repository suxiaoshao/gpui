#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use crate::errors::AiChatError;
use crate::errors::AiChatResult;
use crate::views::home::HomeView;
use gpui::*;
use gpui_component::Root;
use gpui_component::TitleBar;
use gpui_component::input;
use i18n::I18n;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod assets;
mod components;
mod config;
mod database;
mod errors;
mod extensions;
mod gpui_ext;
mod hotkey;
mod i18n;
mod llm;
mod store;
mod views;

static APP_NAME: &str = "top.sushao.ai-chat";

actions!(ai_chat, [Quit]);

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
            Timer::after(delay).await;
            event!(
                Level::INFO,
                seconds = delay.as_secs(),
                "profiling auto quit triggered"
            );
            flush();
            if let Err(err) = cx.update(|cx| cx.quit()) {
                event!(
                    Level::ERROR,
                    error = ?err,
                    "failed to quit after profiling delay"
                );
            }
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

fn quit(_: &Quit, cx: &mut App) {
    profiling::flush();
    event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new(
            "shift-enter",
            input::Enter { secondary: true },
            Some("Input"),
        ),
    ]);
    cx.activate(true);
    cx.on_action(quit);

    i18n::init_i18n(cx);
    database::init_store(cx);
    components::init(cx);
    views::init(cx);
    config::init(cx);
    store::init_global(cx);
    hotkey::init(cx);
    extensions::init(cx);
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

fn main() -> AiChatResult<()> {
    profiling::init();

    // tracing
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

    let span = tracing::info_span!("ai-chat");
    let _enter = span.enter();
    let app = Application::new().with_assets(assets::Assets::default());
    event!(Level::INFO, "app created");

    app.run(|cx: &mut App| {
        init(cx);
        profiling::schedule_auto_quit(cx);
        let title = cx.global::<I18n>().t("app-title");
        if let Err(err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..TitleBar::title_bar_options()
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| HomeView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, "open main window: {}", err)
        };
        event!(Level::INFO, "window opened");
    });
    Ok(())
}
