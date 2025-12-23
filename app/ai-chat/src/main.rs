use crate::errors::AiChatError;
use crate::errors::AiChatResult;
use crate::views::home::HomeView;
use gpui::*;
use gpui_component::Root;
use gpui_component::TitleBar;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod errors;
mod views;

static APP_NAME: &str = "top.sushao.ai-chat";

actions!(feiwen, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);

    // store::init_store(cx);
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

    let span = tracing::info_span!("init");
    let _enter = span.enter();
    let app = Application::new().with_assets(gpui_component_assets::Assets);
    event!(Level::INFO, "app created");

    app.run(|cx: &mut App| {
        init(cx);
        if let Err(err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| HomeView::new());
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, "{}", err)
        };
        event!(Level::INFO, "window opened");
    });
    Ok(())
}
