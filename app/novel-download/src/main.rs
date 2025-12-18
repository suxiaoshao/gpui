use errors::{NovelError, NovelResult};
use gpui::*;
use gpui_component::Root;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer,
    fmt::{self},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};
use views::WorkspaceView;

mod crawler;
mod errors;
mod views;

static APP_NAME: &str = "novel-download";

actions!(novel_download, [Quit]);

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);
}

fn get_logs_dir() -> NovelResult<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = dirs_next::home_dir()
        .ok_or(NovelError::LogFileNotFound)
        .map(|dir| dir.join("Library/Logs").join(APP_NAME));

    #[cfg(not(target_os = "macos"))]
    let path = dirs_next::data_local_dir()
        .ok_or(NovelError::LogFileNotFound)
        .map(|dir| dir.join(APP_NAME).join("logs"));

    if let Ok(path) = &path
        && !path.exists()
    {
        create_dir_all(path).map_err(|_| NovelError::LogFileNotFound)?;
    }
    path
}

fn main() -> NovelResult<()> {
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
                        .map_err(|_| NovelError::LogFileNotFound)?,
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

    app.run(move |cx| {
        init(cx);
        if let Err(err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Novel Download".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| WorkspaceView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, "{}", err)
        };
        event!(Level::INFO, "window opened");
    });
    Ok(())
}

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit by action");
    cx.quit();
}
