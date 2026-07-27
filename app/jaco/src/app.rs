pub(crate) mod menus;
pub(crate) mod session;
pub(crate) mod tasks;
pub(crate) mod temporary_window;
pub(crate) mod title_bar_menu;

use crate::features::{
    about::AboutWindow,
    home::{HomeView, JacoRoot},
    settings::SettingsView,
};
use crate::{database, errors::JacoError, foundation, state};
use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_store::Store;
use std::{
    cell::RefCell,
    ffi::OsString,
    fs::create_dir_all,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer, filter::Targets, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};
use window_ext::{NativeWindowHandle, WindowExt};

pub(crate) static APP_NAME: &str = "top.sushao.jaco";
pub(crate) const LOG_DIR_ENV: &str = "JACO_LOG_DIR";
#[cfg(test)]
const APP_TITLE: &str = "Jaco";
const MAIN_WINDOW_FALLBACK_SIZE: Size<Pixels> = size(px(1536.), px(864.));
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppShutdownPhase {
    Running,
    Draining,
}

pub(crate) type AppShutdownStore = Store<AppShutdownPhase>;

pub(crate) fn run() -> crate::errors::JacoResult<()> {
    init_tracing()?;
    event!(Level::INFO, "startup begin");

    let app = gpui_platform::application().with_assets(foundation::Assets::default());
    app.on_reopen(show_or_create_main_window);
    let startup_error = Rc::new(RefCell::new(None));
    let startup_error_for_run = Rc::clone(&startup_error);
    app.run(move |cx: &mut App| {
        if let Err(err) = init(cx) {
            event!(Level::ERROR, error = ?err, "failed to initialize jaco");
            *startup_error_for_run.borrow_mut() = Some(err);
            cx.quit();
            return;
        }

        show_or_create_main_window(cx);
    });

    if let Some(err) = startup_error.borrow_mut().take() {
        return Err(err);
    }

    Ok(())
}

fn get_logs_dir() -> crate::errors::JacoResult<PathBuf> {
    let path = match override_dir_from_env(LOG_DIR_ENV) {
        Some(path) => path,
        None => {
            logs_dir_from_base(logs_base_dir().ok_or(crate::errors::JacoError::LogFileNotFound)?)
        }
    };

    if !path.exists() {
        create_dir_all(&path).map_err(|_| crate::errors::JacoError::LogFileNotFound)?;
    }

    Ok(path)
}

fn override_dir_from_env(name: &str) -> Option<PathBuf> {
    override_dir_from_value(std::env::var_os(name))
}

fn override_dir_from_value(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn logs_base_dir() -> Option<PathBuf> {
    dirs_next::home_dir()
}

#[cfg(not(target_os = "macos"))]
fn logs_base_dir() -> Option<PathBuf> {
    dirs_next::data_local_dir()
}

#[cfg(target_os = "macos")]
fn logs_dir_from_base(base_dir: PathBuf) -> PathBuf {
    base_dir.join("Library/Logs").join(APP_NAME)
}

#[cfg(not(target_os = "macos"))]
fn logs_dir_from_base(base_dir: PathBuf) -> PathBuf {
    base_dir.join(APP_NAME).join("logs")
}

fn init_tracing() -> crate::errors::JacoResult<()> {
    let logs_dir = get_logs_dir()?;
    let log_file = logs_dir.join("data.log");
    let targets = Targets::new()
        .with_default(LevelFilter::WARN)
        .with_target("jaco::", LevelFilter::INFO)
        .with_target("gpui_operation", LevelFilter::DEBUG)
        .with_target("gpui_macos::text_system", LevelFilter::ERROR);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .with_writer(
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(&log_file)
                        .map_err(|_| crate::errors::JacoError::LogFileNotFound)?,
                )
                .with_filter(targets.clone()),
        )
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .event_format(fmt::format().pretty())
                .with_filter(targets),
        )
        .init();

    Ok(())
}

fn init(cx: &mut App) -> crate::errors::JacoResult<()> {
    SHUTTING_DOWN.store(false, Ordering::Release);
    AppShutdownStore::install_global(cx, AppShutdownPhase::Running);
    tasks::init(cx);
    gpui_component::init(cx);
    foundation::init_bootstrap(cx);

    state::config::init(cx)?;
    foundation::init_i18n(cx);
    state::layout::init(cx)?;
    gpui_tokio::init(cx);
    state::theme::init(cx);
    state::mcp::init(cx)?;
    if let Err(err) = state::hotkey::init(cx) {
        event!(Level::ERROR, error = ?err, "failed to initialize jaco hotkeys");
    }

    database::init_store(cx);
    session::init(cx);
    title_bar_menu::init(cx);
    temporary_window::init(cx);
    crate::features::init(cx);

    menus::init(cx);
    foundation::init_i18n_runtime(cx);
    menus::sync_app_menus(cx);
    reload_app_menu_bars(cx);
    #[cfg(target_os = "macos")]
    menus::ensure_localized_window_menu_registered();

    cx.activate(true);
    Ok(())
}

fn init_ready_services(cx: &mut App) -> crate::errors::JacoResult<()> {
    state::providers::init(cx);
    state::projects::init(cx);
    state::prompts::init(cx);
    state::shortcuts::init(cx);
    state::hotkey::init_shortcuts(cx);
    Ok(())
}

pub(crate) fn critical_resources_ready(cx: &App) -> bool {
    state::config::store(cx).read(cx, |operation| {
        matches!(operation, state::config::ConfigOperation::Ready(_))
    }) && database::is_ready(cx)
}

pub(crate) fn quit_app(cx: &mut App) {
    if SHUTTING_DOWN.swap(true, Ordering::AcqRel) {
        show_or_create_main_window(cx);
        return;
    }

    let session = database::store(cx).read(cx, |resource| match resource {
        database::DatabaseResource::Bound { operation, .. } => {
            operation.data().map(|data| data.session.clone())
        }
        database::DatabaseResource::AwaitingConfig => None,
    });
    let runtime_task =
        ready_runtime(cx).map(|runtime| runtime.update(cx, |runtime, cx| runtime.shutdown_all(cx)));

    AppShutdownStore::global(cx).set(cx, AppShutdownPhase::Draining);
    state::hotkey::shutdown(cx);
    crate::features::screenshot::overlay::close(cx);
    temporary_window::close_temporary_window(cx);
    show_or_create_main_window(cx);
    event!(Level::INFO, "managed jaco shutdown started");

    let shutdown_task = cx.spawn(async move |cx| {
        if let Some(runtime_task) = runtime_task {
            runtime_task.await;
        }

        let active = session.and_then(|session| {
            cx.update(|cx| session.update(cx, |session, _| session.take_active()))
        });
        if let Some(active) = active.as_ref() {
            while active.active_jobs() != 0 {
                smol::Timer::after(Duration::from_millis(10)).await;
            }
        }
        drop(active);

        cx.update(|cx| {
            state::layout::save_now(cx);
            event!(Level::INFO, "managed jaco shutdown completed");
            cx.quit();
        });
    });
    tasks::retain_application(shutdown_task, cx);
}

pub(crate) fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::Acquire)
}

fn register_main_window_close_behavior(window: &mut Window, cx: &mut App) {
    window.on_window_should_close(cx, |window, _cx| {
        if should_hide_main_window_on_close() {
            if let Err(err) = window.hide() {
                event!(Level::ERROR, error = ?err, "hide jaco main window failed");
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
    let view = cx.new(|cx| JacoRoot::new(window, cx));
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

fn focus_main_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let _ = with_root_view::<JacoRoot, _>(root, cx, |view, cx| {
        view.update(cx, |view, cx| {
            view.focus_primary(window, cx);
        });
    });
}

fn schedule_main_window_reveal(native_window: NativeWindowHandle, cx: &mut App) {
    cx.defer(move |_| {
        if let Err(err) = native_window.show() {
            event!(Level::ERROR, error = ?err, "show jaco main window failed");
        }
    });
}

pub(crate) fn open_main_window(cx: &mut App) -> Result<WindowHandle<Root>, JacoError> {
    let title = cx.global::<foundation::I18n>().t("app-title");
    let placement = state::layout::restored_window_placement(
        state::layout::WindowPlacementKind::Main,
        MAIN_WINDOW_FALLBACK_SIZE,
        cx,
    );
    cx.open_window(
        WindowOptions {
            window_bounds: Some(placement.window_bounds),
            display_id: placement.display_id,
            titlebar: Some(main_titlebar_options(title)),
            app_owns_titlebar_drag: true,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        create_main_root,
    )
    .map_err(|err| JacoError::Window(err.to_string()))
}

fn main_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}

pub(crate) fn find_main_window(cx: &App) -> Option<WindowHandle<Root>> {
    find_window_by_view::<JacoRoot>(cx)
}

pub(crate) fn ready_runtime(
    cx: &App,
) -> Option<Entity<crate::features::conversation::runtime::ConversationRuntimeStore>> {
    session::ready_runtime(cx)
}

pub(crate) fn reload_app_menu_bars(cx: &mut App) {
    let roots = cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<Root>())
        .collect::<Vec<_>>();

    for root in roots {
        let _ = root.update(cx, |root, _window, cx| {
            let _ = with_root_view::<JacoRoot, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<HomeView, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<AboutWindow, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
            let _ = with_root_view::<SettingsView, _>(root, cx, |view, cx| {
                view.update(cx, |view, cx| view.reload_app_menu_bar(cx));
            });
        });
    }
}

pub(crate) fn show_or_create_main_window(cx: &mut App) {
    if let Some(window) = find_main_window(cx) {
        let mut reveal_window = None;
        if let Err(err) = window.update(cx, |root, window, cx| {
            if !window.is_window_active() {
                match window.native_window_handle() {
                    Ok(handle) => reveal_window = Some(handle),
                    Err(err) => {
                        event!(Level::ERROR, error = ?err, "get jaco main window handle failed");
                    }
                }
            }
            focus_main_window(root, window, cx);
        }) {
            event!(Level::ERROR, error = ?err, "update jaco main window failed");
        }
        if let Some(native_window) = reveal_window {
            schedule_main_window_reveal(native_window, cx);
        }
        return;
    }

    match open_main_window(cx) {
        Ok(window) => {
            if let Err(err) = window.update(cx, |root, window, cx| {
                focus_main_window(root, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "activate new jaco main window failed");
            }
        }
        Err(err) => {
            event!(Level::ERROR, error = ?err, "open jaco main window failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APP_NAME, APP_TITLE, logs_dir_from_base, main_titlebar_options, override_dir_from_value,
        should_hide_main_window_on_close,
    };
    use gpui_component::TitleBar;
    use std::{ffi::OsString, path::PathBuf};

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

    #[test]
    fn log_directory_matches_app_convention() {
        let dir = logs_dir_from_base(PathBuf::from("/tmp/base"));

        #[cfg(target_os = "macos")]
        assert_eq!(dir, PathBuf::from("/tmp/base/Library/Logs").join(APP_NAME));

        #[cfg(not(target_os = "macos"))]
        assert_eq!(dir, PathBuf::from("/tmp/base").join(APP_NAME).join("logs"));
    }

    #[test]
    fn log_directory_override_ignores_empty_env_value() {
        assert_eq!(override_dir_from_value(None), None);
        assert_eq!(override_dir_from_value(Some(OsString::new())), None);
        assert_eq!(
            override_dir_from_value(Some(OsString::from("/tmp/jaco-logs"))),
            Some(PathBuf::from("/tmp/jaco-logs"))
        );
    }
}
