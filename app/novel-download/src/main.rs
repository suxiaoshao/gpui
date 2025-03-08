use std::{fs::create_dir_all, path::PathBuf};

use errors::{NovelError, NovelResult};
use gpui::*;
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
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

    if let Ok(path) = &path {
        if !path.exists() {
            create_dir_all(path).map_err(|_| NovelError::LogFileNotFound)?;
        }
    }
    path
}

fn main() -> NovelResult<()> {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(get_logs_dir()?.join("data.log"))
                        .map_err(|_| NovelError::LogFileNotFound)?,
                )
                .with_filter(LevelFilter::INFO),
        )
        .init();
    let app = Application::new();

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
            |window, cx| cx.new(|cx| WorkspaceView::new(window, cx)),
        ) {
            tracing::info_span!("init");
            event!(Level::ERROR, "{}", err)
        };
    });
    Ok(())
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
