use crate::{
    app::{open_temporary_window, quit_app, show_or_create_main_window, toggle_temporary_window},
    i18n::I18n,
    views::about::open_about_window,
};
use anyhow::{Context as _, anyhow};
use fluent_bundle::FluentArgs;
use gpui::{App, AsyncApp, Global, Task};
use tracing::{Level, event};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent, TrayIconId,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const TRAY_ICON_ID: &str = "ai-chat-main-tray";
const MENU_OPEN_MAIN: &str = "tray-open-main";
const MENU_OPEN_TEMPORARY: &str = "tray-open-temporary";
const MENU_ABOUT: &str = "tray-about";
const MENU_QUIT: &str = "tray-quit";

#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../assets/png/tray-template.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] =
    include_bytes!("../../build-assets/icon/app-icon.iconset/icon_32x32.png");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayMenuAction {
    OpenMain,
    OpenTemporary,
    About,
    Quit,
}

enum TrayEvent {
    Menu(MenuEvent),
    #[cfg(not(target_os = "linux"))]
    Icon(TrayIconEvent),
}

#[derive(Clone)]
struct TrayStrings {
    open_main: String,
    open_temporary: String,
    version: String,
    about: String,
    quit: String,
    tooltip: String,
}

impl TrayStrings {
    fn new(i18n: &I18n) -> Self {
        let mut args = FluentArgs::new();
        args.set("version", env!("CARGO_PKG_VERSION"));

        Self {
            open_main: i18n.t("tray-open-main"),
            open_temporary: i18n.t("tray-open-temporary"),
            version: i18n.t_with_args("tray-version", &args),
            about: i18n.t("tray-about"),
            quit: i18n.t("tray-quit"),
            tooltip: i18n.t("tray-tooltip"),
        }
    }
}

pub(crate) struct TrayState {
    _event_task: Task<()>,
    #[cfg(target_os = "linux")]
    tray_update: smol::channel::Sender<TrayStrings>,
    #[cfg(not(target_os = "linux"))]
    tray_icon: TrayIcon,
}

impl Global for TrayState {}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<TrayState>() {
        return;
    }

    let i18n = cx.global::<I18n>();
    let strings = TrayStrings::new(i18n);
    let (tx, rx) = smol::channel::unbounded();

    MenuEvent::set_event_handler(Some({
        let tx = tx.clone();
        move |event| {
            let _ = tx.send_blocking(TrayEvent::Menu(event));
        }
    }));

    #[cfg(not(target_os = "linux"))]
    TrayIconEvent::set_event_handler(Some({
        let tx = tx.clone();
        move |event| {
            let _ = tx.send_blocking(TrayEvent::Icon(event));
        }
    }));

    let event_task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            match event {
                TrayEvent::Menu(event) => handle_menu_event(event, cx),
                #[cfg(not(target_os = "linux"))]
                TrayEvent::Icon(event) => handle_tray_event(event, cx),
            }
        }
    });

    #[cfg(target_os = "linux")]
    {
        let (tray_update, tray_updates) = smol::channel::unbounded();
        spawn_linux_tray(strings, tray_updates);
        cx.set_global(TrayState {
            _event_task: event_task,
            tray_update,
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        match build_tray_icon(&strings) {
            Ok(tray_icon) => {
                cx.set_global(TrayState {
                    _event_task: event_task,
                    tray_icon,
                });
            }
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Failed to initialize tray");
            }
        }
    }
}

pub(crate) fn refresh(cx: &mut App) {
    if !cx.has_global::<TrayState>() {
        return;
    }

    let strings = TrayStrings::new(cx.global::<I18n>());

    #[cfg(target_os = "linux")]
    {
        let tray_update = cx.global::<TrayState>().tray_update.clone();
        if let Err(err) = tray_update.try_send(strings) {
            event!(Level::ERROR, error = ?err, "Failed to refresh tray");
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        update_tray_icon(&cx.global::<TrayState>().tray_icon, &strings);
    }
}

fn handle_menu_event(event: MenuEvent, cx: &mut AsyncApp) {
    let Some(action) = tray_menu_action(event.id.as_ref()) else {
        return;
    };

    cx.update(|cx| match action {
        TrayMenuAction::OpenMain => show_or_create_main_window(cx),
        TrayMenuAction::OpenTemporary => open_temporary_window(cx),
        TrayMenuAction::About => open_about_window(cx),
        TrayMenuAction::Quit => quit_app(cx),
    });
}

#[cfg(not(target_os = "linux"))]
fn handle_tray_event(event: TrayIconEvent, cx: &mut AsyncApp) {
    let TrayIconEvent::Click {
        button,
        button_state,
        ..
    } = event
    else {
        return;
    };

    if event.id() != &TrayIconId::from(TRAY_ICON_ID) {
        return;
    }

    if button == MouseButton::Left && button_state == MouseButtonState::Up {
        cx.update(toggle_temporary_window);
    }
}

#[cfg(not(target_os = "linux"))]
fn build_tray_icon(strings: &TrayStrings) -> anyhow::Result<TrayIcon> {
    let menu = build_menu(strings)?;
    let mut builder = TrayIconBuilder::new()
        .with_id(TRAY_ICON_ID)
        .with_menu(Box::new(menu))
        .with_tooltip(strings.tooltip.clone())
        .with_menu_on_left_click(false)
        .with_icon(load_tray_icon()?);

    #[cfg(target_os = "macos")]
    {
        builder = builder.with_icon_as_template(true);
    }

    builder
        .build()
        .map_err(|err| anyhow!("tray build failed: {err}"))
}

#[cfg(target_os = "linux")]
fn spawn_linux_tray(strings: TrayStrings, tray_updates: smol::channel::Receiver<TrayStrings>) {
    let _ = std::thread::Builder::new()
        .name("ai-chat-tray".into())
        .spawn(move || {
            use gtk::prelude::*;

            if let Err(err) = gtk::init() {
                event!(Level::ERROR, error = ?err, "Failed to initialize GTK for tray");
                return;
            }

            let menu = match build_menu(&strings) {
                Ok(menu) => menu,
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "Failed to build tray menu");
                    return;
                }
            };

            let tray_icon = TrayIconBuilder::new()
                .with_id(TRAY_ICON_ID)
                .with_menu(Box::new(menu))
                .with_tooltip(strings.tooltip.clone())
                .with_temp_dir_path(std::env::temp_dir())
                .with_icon(match load_tray_icon() {
                    Ok(icon) => icon,
                    Err(err) => {
                        event!(Level::ERROR, error = ?err, "Failed to load tray icon");
                        return;
                    }
                })
                .build();

            match tray_icon {
                Ok(tray_icon) => {
                    gtk::glib::MainContext::default().spawn_local(async move {
                        while let Ok(strings) = tray_updates.recv().await {
                            update_tray_icon(&tray_icon, &strings);
                        }
                    });
                    gtk::main();
                }
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "Failed to build tray");
                }
            }
        });
}

fn update_tray_icon(tray_icon: &TrayIcon, strings: &TrayStrings) {
    match build_menu(strings) {
        Ok(menu) => tray_icon.set_menu(Some(Box::new(menu))),
        Err(err) => event!(Level::ERROR, error = ?err, "Failed to refresh tray menu"),
    }

    if let Err(err) = tray_icon.set_tooltip(Some(strings.tooltip.clone())) {
        event!(Level::ERROR, error = ?err, "Failed to refresh tray tooltip");
    }
}

fn build_menu(strings: &TrayStrings) -> anyhow::Result<Menu> {
    let open_main = MenuItem::with_id(MENU_OPEN_MAIN, &strings.open_main, true, None);
    let open_temporary =
        MenuItem::with_id(MENU_OPEN_TEMPORARY, &strings.open_temporary, true, None);
    let version = MenuItem::new(&strings.version, false, None);
    let about = MenuItem::with_id(MENU_ABOUT, &strings.about, true, None);
    let quit = MenuItem::with_id(MENU_QUIT, &strings.quit, true, None);

    Menu::with_items(&[
        &open_main,
        &open_temporary,
        &PredefinedMenuItem::separator(),
        &version,
        &about,
        &quit,
    ])
    .map_err(|err| anyhow!("menu build failed: {err}"))
}

fn load_tray_icon() -> anyhow::Result<Icon> {
    let image = image::load_from_memory(TRAY_ICON_BYTES)
        .context("decode tray icon png failed")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).context("create tray icon failed")
}

fn tray_menu_action(menu_id: &str) -> Option<TrayMenuAction> {
    match menu_id {
        MENU_OPEN_MAIN => Some(TrayMenuAction::OpenMain),
        MENU_OPEN_TEMPORARY => Some(TrayMenuAction::OpenTemporary),
        MENU_ABOUT => Some(TrayMenuAction::About),
        MENU_QUIT => Some(TrayMenuAction::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn tray_menu_actions_map_known_ids() {
        assert_eq!(
            tray_menu_action(MENU_OPEN_MAIN),
            Some(TrayMenuAction::OpenMain)
        );
        assert_eq!(
            tray_menu_action(MENU_OPEN_TEMPORARY),
            Some(TrayMenuAction::OpenTemporary)
        );
        assert_eq!(tray_menu_action(MENU_ABOUT), Some(TrayMenuAction::About));
        assert_eq!(tray_menu_action(MENU_QUIT), Some(TrayMenuAction::Quit));
        assert_eq!(tray_menu_action("unknown"), None);
    }

    #[::core::prelude::v1::test]
    fn tray_icon_bytes_decode_to_rgba32() {
        let image = image::load_from_memory(TRAY_ICON_BYTES).expect("tray icon decodes");

        assert_eq!(image.width(), 32);
        assert_eq!(image.height(), 32);
        assert_eq!(image.color(), image::ColorType::Rgba8);
    }

    #[::core::prelude::v1::test]
    fn tray_strings_follow_i18n_locale() {
        let strings = TrayStrings::new(&I18n::for_locale_tag("zh-CN"));

        assert_eq!(strings.open_main, "打开 AI 对话");
        assert_eq!(strings.open_temporary, "打开临时对话");
        assert_eq!(strings.about, "关于 AI 对话");
        assert_eq!(strings.quit, "退出 AI 对话");
        assert_eq!(strings.tooltip, "AI 对话");
    }
}
