#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::foundation::i18n::t;
use gpui_kit::*;
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct Tray {
    _icon: tray_icon::TrayIcon,
    _events: Task<()>,
    items: [tray_icon::menu::MenuItem; 4],
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Global for Tray {}
pub fn init(_cx: &mut App) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Err(error) = build(_cx) {
        tracing::error!(%error, "create system tray failed");
    }
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build(cx: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use tray_icon::{
        Icon, TrayIconBuilder,
        menu::{Menu, MenuEvent, MenuItem},
    };
    let menu = Menu::new();
    let temporary = MenuItem::new(t(cx, "temporary-title"), true, None);
    let main = MenuItem::new(t(cx, "menu-show-main"), true, None);
    let settings = MenuItem::new(t(cx, "menu-settings"), true, None);
    let quit = MenuItem::new(t(cx, "menu-quit"), true, None);
    menu.append_items(&[&temporary, &main, &settings, &quit])?;
    let image = image::load_from_memory(include_bytes!("../../build-assets/icon/app-icon.png"))?
        .thumbnail(32, 32)
        .into_rgba8();
    let icon = Icon::from_rgba(image.as_raw().clone(), image.width(), image.height())?;
    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("Gupi")
        .build()?;
    let items = [
        temporary.clone(),
        main.clone(),
        settings.clone(),
        quit.clone(),
    ];
    let (tx, rx) = smol::channel::unbounded();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = tx.try_send(e.id);
    }));
    let events = cx.spawn(async move |cx| {
        while let Ok(id) = rx.recv().await {
            cx.update(|cx| {
                if id == *temporary.id() {
                    super::temporary::toggle(cx);
                } else if id == *main.id() {
                    super::show(Some(false), cx);
                } else if id == *settings.id() {
                    super::show(Some(true), cx);
                } else if id == *quit.id() {
                    super::quit(cx);
                }
            });
        }
    });
    cx.set_global(Tray {
        _icon: icon,
        _events: events,
        items,
    });
    Ok(())
}

pub fn refresh(_cx: &App) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(tray) = _cx.try_global::<Tray>() {
        for (item, key) in tray.items.iter().zip([
            "temporary-title",
            "menu-show-main",
            "menu-settings",
            "menu-quit",
        ]) {
            item.set_text(t(_cx, key));
        }
    }
}
