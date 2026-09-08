pub(crate) mod menus;
use crate::{
    features::startup::StartupView,
    foundation::{assets::Assets, i18n, paths},
    state::layout,
};
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::*;
use window_ext::WindowExt;
struct MainWindow {
    window: WindowHandle<Root>,
    view: Entity<StartupView>,
}
impl Global for MainWindow {}
pub(crate) fn run() {
    let log = paths::log_dir().and_then(|dir| {
        std::fs::create_dir_all(&dir)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("gupi.log"))
    });
    let log_warning = log.is_err();
    match log {
        Ok(file) => {
            let _ = tracing_subscriber::fmt().with_writer(file).try_init();
        }
        Err(error) => {
            eprintln!("Gupi log initialization: {error}");
            let _ = tracing_subscriber::fmt().try_init();
        }
    }
    let app = gpui_kit::application().with_assets(Assets::default());
    app.on_reopen(|cx| cx.defer(|cx| show(None, cx)));
    app.run(move |cx| {
        gpui_kit::init(cx);
        gpui_tokio::init(cx);
        crate::state::pi::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        i18n::apply(Default::default(), cx);
        menus::refresh(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", menus::Quit, None),
            KeyBinding::new("cmd-,", menus::ShowSettings, None),
        ]);
        cx.on_action(|_: &menus::ShowSettings, cx| cx.defer(|cx| show(Some(true), cx)));
        cx.on_action(|_: &menus::ShowMainWindow, cx| cx.defer(|cx| show(Some(false), cx)));
        cx.on_action(|_: &menus::Quit, cx| cx.defer(quit));
        let layout = paths::config_dir()
            .map_err(|e| e.to_string())
            .map(|dir| layout::load(&dir.join("state.toml")))
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "layout directory unavailable; using default layout");
                layout::LayoutState::default()
            });
        let bounds = layout
            .main_window
            .map(|p| p.restored(cx))
            .unwrap_or_else(|| {
                WindowBounds::Windowed(Bounds::centered(None, layout::default_window_size(), cx))
            });
        let options = WindowOptions {
            window_bounds: Some(bounds),
            window_min_size: Some(layout::minimum_window_size()),
            titlebar: Some(TitlebarOptions {
                title: Some("Gupi".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..TitleBar::window_options()
        };
        match cx.open_window(options, |window, cx| {
            window.on_window_should_close(cx, |window, cx| {
                if cfg!(any(target_os = "macos", target_os = "windows")) {
                    match window.native_window_handle() {
                        Ok(handle) => cx.defer(move |_| {
                            if let Err(error) = handle.hide() {
                                tracing::error!(%error, "hide window failed");
                            } else {
                                tracing::info!("main window hidden");
                            }
                        }),
                        Err(error) => tracing::error!(%error, "get native window failed"),
                    }
                } else {
                    cx.defer(quit);
                }
                false
            });
            let view = cx.new(|cx| StartupView::new(log_warning, window, cx));
            let root = cx.new(|cx| Root::new(view.clone(), window, cx));
            cx.set_global(MainWindow {
                window: window
                    .window_handle()
                    .downcast::<Root>()
                    .expect("root window"),
                view,
            });
            root
        }) {
            Ok(_) => cx.activate(true),
            Err(error) => {
                tracing::error!(%error, "open window failed");
                cx.quit();
            }
        }
    });
}
fn quit(cx: &mut App) {
    let main = cx
        .try_global::<MainWindow>()
        .map(|m| (m.window, m.view.clone()));
    if let Some((window, view)) = main {
        if let Err(error) = window.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.quit(window, cx))
        }) {
            tracing::error!(%error, "start managed quit failed");
        }
    } else {
        cx.quit();
    }
}
fn show(settings: Option<bool>, cx: &mut App) {
    let main = cx
        .try_global::<MainWindow>()
        .map(|m| (m.window, m.view.clone()));
    if let Some((window, view)) = main {
        let mut native = None;
        if let Err(error) = window.update(cx, |_, window, cx| {
            if !window.is_window_active() {
                native = window.native_window_handle().ok();
            }
            view.update(cx, |view, cx| {
                if !view.draining
                    && let Some(settings) = settings
                {
                    view.show_settings = settings;
                }
                cx.notify();
            });
        }) {
            tracing::error!(%error, "update main window failed");
        }
        if let Some(native) = native {
            // Native focus notifications can synchronously re-enter GPUI.
            cx.defer(move |_| {
                if let Err(error) = native.show() {
                    tracing::error!(%error, "show window failed");
                } else {
                    tracing::info!("main window shown");
                }
            });
        }
    }
}
