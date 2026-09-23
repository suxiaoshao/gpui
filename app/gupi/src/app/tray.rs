#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::foundation::i18n::t;
use gpui_kit::*;
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct Tray {
    icon: tray_icon::TrayIcon,
    menu: tray_icon::menu::Menu,
    targets: std::collections::HashMap<tray_icon::menu::MenuId, super::notifications::Target>,
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
        .with_menu(Box::new(menu.clone()))
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
                } else if let Some(target) = cx
                    .try_global::<Tray>()
                    .and_then(|t| t.targets.get(&id))
                    .cloned()
                {
                    target.open(cx);
                }
            });
        }
    });
    cx.set_global(Tray {
        icon,
        menu,
        targets: Default::default(),
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

pub(crate) struct Entry {
    pub title: String,
    pub activity: crate::state::conversation::Activity,
    pub unread: bool,
    pub target: super::notifications::Target,
}
pub fn update(count: usize, entries: Vec<Entry>, cx: &mut App) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use crate::state::conversation::Activity;
        use tray_icon::menu::{MenuItem, PredefinedMenuItem};
        let unread = t(cx, "notification-unread");
        let waiting = t(cx, "notification-waiting");
        let running = t(cx, "notification-running");
        if !cx.has_global::<Tray>() {
            return;
        }
        let tray = cx.global_mut::<Tray>();
        #[cfg(target_os = "macos")]
        tray.icon.set_title(Some(if count == 0 {
            String::new()
        } else {
            count.to_string()
        }));
        let _ = tray
            .icon
            .set_tooltip(Some(format!("Gupi — {unread}: {count}")));
        while tray.menu.items().len() > 4 {
            tray.menu.remove_at(4);
        }
        tray.targets.clear();
        if entries.is_empty() {
            return;
        }
        let _ = tray.menu.append(&PredefinedMenuItem::separator());
        let _ = tray
            .menu
            .append(&MenuItem::new(format!("{unread}: {count}"), false, None));
        // One entry per source. Waiting is the actionable state even when unread too.
        for entry in entries {
            let label = if entry.activity == Activity::Waiting {
                &waiting
            } else if entry.unread {
                &unread
            } else {
                &running
            };
            let item = MenuItem::new(format!("{label} — {}", entry.title), true, None);
            if tray.menu.append(&item).is_ok() {
                tray.targets.insert(item.id().clone(), entry.target);
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (count, entries, cx);
}
