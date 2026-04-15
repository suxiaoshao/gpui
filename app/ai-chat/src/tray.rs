use crate::{
    app::{quit_app, show_or_create_main_window},
    hotkey,
    i18n::I18n,
};
use anyhow::{Context as _, anyhow};
use fluent_bundle::FluentArgs;
use gpui::{App, AsyncApp, Global, Task};
use tracing::{Level, event};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent, TrayIconId,
    menu::{AboutMetadata, Icon as MenuIcon, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const TRAY_ICON_ID: &str = "ai-chat-main-tray";
const MENU_OPEN_MAIN: &str = "tray-open-main";
const MENU_OPEN_TEMPORARY: &str = "tray-open-temporary";
const MENU_QUIT: &str = "tray-quit";

const TRAY_TEMPLATE_ICON_BYTES: &[u8] = include_bytes!("../assets/png/tray-template.png");
const ABOUT_ICON_BYTES: &[u8] =
    include_bytes!("../build-assets/icon/app-icon.iconset/icon_128x128.png");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayMenuAction {
    OpenMain,
    OpenTemporary,
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

#[derive(Clone)]
struct AboutInfo {
    name: String,
    comments: String,
    website_label: String,
}

impl AboutInfo {
    fn new(i18n: &I18n) -> Self {
        Self {
            name: i18n.t("app-title"),
            comments: i18n.t("tray-about-comments"),
            website_label: i18n.t("tray-about-website-label"),
        }
    }

    fn metadata(&self) -> anyhow::Result<AboutMetadata> {
        Ok(AboutMetadata {
            name: Some(self.name.clone()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            short_version: Some(format!(
                "{}.{}",
                env!("CARGO_PKG_VERSION_MAJOR"),
                env!("CARGO_PKG_VERSION_MINOR")
            )),
            authors: parse_authors(env!("CARGO_PKG_AUTHORS")),
            comments: Some(self.comments.clone()),
            license: non_empty_env("CARGO_PKG_LICENSE"),
            website: non_empty_env("CARGO_PKG_HOMEPAGE"),
            website_label: Some(self.website_label.clone()),
            icon: Some(load_menu_icon(ABOUT_ICON_BYTES)?),
            ..Default::default()
        })
    }
}

pub(crate) struct TrayState {
    _event_task: Task<()>,
    #[cfg(not(target_os = "linux"))]
    _tray_icon: TrayIcon,
}

impl Global for TrayState {}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<TrayState>() {
        return;
    }

    let i18n = cx.global::<I18n>();
    let strings = TrayStrings::new(i18n);
    let about = AboutInfo::new(i18n);
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
        spawn_linux_tray(strings, about);
        cx.set_global(TrayState {
            _event_task: event_task,
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        match build_tray_icon(&strings, &about) {
            Ok(tray_icon) => {
                cx.set_global(TrayState {
                    _event_task: event_task,
                    _tray_icon: tray_icon,
                });
            }
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Failed to initialize tray");
            }
        }
    }
}

fn handle_menu_event(event: MenuEvent, cx: &mut AsyncApp) {
    let Some(action) = tray_menu_action(event.id.as_ref()) else {
        return;
    };

    let result = cx.update(|cx| match action {
        TrayMenuAction::OpenMain => show_or_create_main_window(cx),
        TrayMenuAction::OpenTemporary => hotkey::open_temporary_window(cx),
        TrayMenuAction::Quit => quit_app(cx),
    });

    if let Err(err) = result {
        event!(Level::ERROR, error = ?err, "Failed to handle tray menu action");
    }
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

    if button == MouseButton::Left
        && button_state == MouseButtonState::Up
        && let Err(err) = cx.update(hotkey::toggle_temporary_window)
    {
        event!(
            Level::ERROR,
            error = ?err,
            "Failed to toggle temporary window from tray"
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn build_tray_icon(strings: &TrayStrings, about: &AboutInfo) -> anyhow::Result<TrayIcon> {
    let menu = build_menu(strings, about)?;
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
fn spawn_linux_tray(strings: TrayStrings, about: AboutInfo) {
    let _ = std::thread::Builder::new()
        .name("ai-chat-tray".into())
        .spawn(move || {
            use gtk::prelude::*;

            if let Err(err) = gtk::init() {
                event!(Level::ERROR, error = ?err, "Failed to initialize GTK for tray");
                return;
            }

            let menu = match build_menu(&strings, &about) {
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
                Ok(_tray_icon) => gtk::main(),
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "Failed to build tray");
                }
            }
        });
}

fn build_menu(strings: &TrayStrings, about: &AboutInfo) -> anyhow::Result<Menu> {
    let open_main = MenuItem::with_id(MENU_OPEN_MAIN, &strings.open_main, true, None);
    let open_temporary =
        MenuItem::with_id(MENU_OPEN_TEMPORARY, &strings.open_temporary, true, None);
    let version = MenuItem::new(&strings.version, false, None);
    let about = PredefinedMenuItem::about(Some(strings.about.as_str()), Some(about.metadata()?));
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
    let image = image::load_from_memory(TRAY_TEMPLATE_ICON_BYTES)
        .context("decode tray icon png failed")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).context("create tray icon failed")
}

fn load_menu_icon(bytes: &[u8]) -> anyhow::Result<MenuIcon> {
    let image = image::load_from_memory(bytes)
        .context("decode about icon png failed")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    MenuIcon::from_rgba(image.into_raw(), width, height).context("create about icon failed")
}

fn tray_menu_action(menu_id: &str) -> Option<TrayMenuAction> {
    match menu_id {
        MENU_OPEN_MAIN => Some(TrayMenuAction::OpenMain),
        MENU_OPEN_TEMPORARY => Some(TrayMenuAction::OpenTemporary),
        MENU_QUIT => Some(TrayMenuAction::Quit),
        _ => None,
    }
}

fn parse_authors(value: &str) -> Option<Vec<String>> {
    let authors = value
        .split(':')
        .map(str::trim)
        .filter(|author| !author.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!authors.is_empty()).then_some(authors)
}

fn non_empty_env(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_string())
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
        assert_eq!(tray_menu_action(MENU_QUIT), Some(TrayMenuAction::Quit));
        assert_eq!(tray_menu_action("unknown"), None);
    }

    #[::core::prelude::v1::test]
    fn parse_authors_skips_empty_segments() {
        assert_eq!(
            parse_authors("alice:bob::"),
            Some(vec!["alice".into(), "bob".into()])
        );
        assert_eq!(parse_authors("::"), None);
    }
}
