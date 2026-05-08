use crate::errors::FeiwenError;
use errors::FeiwenResult;
use features::WorkspaceView;
use foundation::I18n;
use gpui::*;
use gpui_component::{Root, TitleBar};
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod components;
mod errors;
mod features;
mod fetch;
mod foundation;
mod store;

static APP_NAME: &str = "top.sushao.feiwen";

actions!(feiwen, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    event!(Level::INFO, "initializing feiwen app");
    gpui_component::init(cx);
    app_theme::init_system_accent_theme(cx);
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);

    foundation::i18n::init_i18n(cx);
    store::init_store(cx);
    event!(Level::INFO, "feiwen app initialized");
}

fn get_logs_dir() -> FeiwenResult<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = dirs_next::home_dir()
        .ok_or(FeiwenError::LogFileNotFound)
        .map(|dir| dir.join("Library/Logs").join(APP_NAME));

    #[cfg(not(target_os = "macos"))]
    let path = dirs_next::data_local_dir()
        .ok_or(FeiwenError::LogFileNotFound)
        .map(|dir| dir.join(APP_NAME).join("logs"));

    if let Ok(path) = &path
        && !path.exists()
    {
        event!(Level::INFO, path = %path.display(), "creating log directory");
        create_dir_all(path).map_err(|_| FeiwenError::LogFileNotFound)?;
    }
    path
}

fn main() -> FeiwenResult<()> {
    let logs_dir = get_logs_dir()?;
    let log_file = logs_dir.join("data.log");

    // tracing
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .with_writer(
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(&log_file)
                        .map_err(|_| FeiwenError::LogFileNotFound)?,
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

    event!(
        Level::INFO,
        logs_dir = %logs_dir.display(),
        log_file = %log_file.display(),
        "tracing initialized"
    );

    let span = tracing::info_span!("init");
    let _enter = span.enter();
    let app = gpui_platform::application().with_assets(foundation::Assets::default());
    event!(Level::INFO, "app created");

    app.run(|cx: &mut App| {
        init(cx);
        let title = cx.global::<I18n>().t("app-title");
        event!(Level::INFO, title = %title, "opening main window");
        match cx.open_window(
            WindowOptions {
                titlebar: Some(main_titlebar_options(title)),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| WorkspaceView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(_) => event!(Level::INFO, "main window opened"),
            Err(err) => event!(Level::ERROR, error = %err, "failed to open main window"),
        }
    });
    Ok(())
}

fn main_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

#[cfg(test)]
mod tests {
    use super::main_titlebar_options;
    use gpui_component::TitleBar;

    #[test]
    fn main_window_uses_component_titlebar_options() {
        let titlebar = main_titlebar_options("Feiwen");
        let expected = TitleBar::title_bar_options();

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some("Feiwen")
        );
        assert_eq!(titlebar.appears_transparent, expected.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            expected.traffic_light_position
        );
    }
}
