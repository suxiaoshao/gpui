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
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };
    let menu = Menu::new();
    let temporary = MenuItem::new(t(cx, "temporary-title"), true, None);
    let main = MenuItem::new(t(cx, "menu-show-main"), true, None);
    let settings = MenuItem::new(t(cx, "menu-settings"), true, None);
    let quit = MenuItem::new(t(cx, "menu-quit"), true, None);
    menu.append_items(&[
        &temporary,
        &main,
        &PredefinedMenuItem::separator(),
        &settings,
        &quit,
    ])?;
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../assets/brand/tray-template.png").as_slice();
    #[cfg(target_os = "windows")]
    let bytes = include_bytes!("../../build-assets/icon/app-icon.png").as_slice();
    let image = image::load_from_memory(bytes)?;
    #[cfg(target_os = "windows")]
    let image = image.thumbnail(32, 32);
    let image = image.into_rgba8();
    let icon = Icon::from_rgba(image.as_raw().clone(), image.width(), image.height())?;
    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_icon(icon)
        .with_tooltip("Gupi");
    #[cfg(target_os = "macos")]
    let builder = builder.with_icon_as_template(true);
    let icon = builder.build()?;
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
            let action: Option<Box<dyn Action>> = match key {
                "menu-show-main" => Some(Box::new(super::menus::ShowMainWindow)),
                "menu-settings" => Some(Box::new(super::menus::ShowSettings)),
                "menu-quit" => Some(Box::new(super::menus::Quit)),
                _ => None,
            };
            let binding = if key == "temporary-title" {
                super::shortcuts::launcher_binding(_cx).and_then(|key| Keystroke::parse(key).ok())
            } else {
                action.and_then(|action| {
                    _cx.key_bindings()
                        .borrow()
                        .bindings()
                        .rfind(|binding| binding.action().partial_eq(action.as_ref()))
                        .and_then(|binding| binding.keystrokes().first())
                        .map(|key| key.as_keystroke().clone())
                })
            };
            // Display only: OS global shortcuts already own their registration.
            let label = t(_cx, key);
            item.set_text(match binding {
                Some(key) => format!("{label}    {}", gpui_kit::component::kbd::Kbd::format(&key)),
                None => label,
            });
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
        // Two window entries and the separator/settings/quit footer stay in place.
        while tray.menu.items().len() > 5 {
            tray.menu.remove_at(2);
        }
        tray.targets.clear();
        if entries.is_empty() {
            return;
        }
        let _ = tray.menu.insert(&PredefinedMenuItem::separator(), 2);
        let _ = tray
            .menu
            .insert(&MenuItem::new(format!("{unread}: {count}"), false, None), 3);
        // One entry per source. Waiting is the actionable state even when unread too.
        let mut entries = entries;
        entries.sort_by_key(|entry| match (entry.activity, entry.unread) {
            (Activity::Waiting, _) => 0,
            (_, true) => 1,
            _ => 2,
        });
        for entry in entries {
            let label = if entry.activity == Activity::Waiting {
                &waiting
            } else if entry.unread {
                &unread
            } else {
                &running
            };
            let item = MenuItem::new(format!("{label} — {}", entry.title), true, None);
            let position = tray.menu.items().len() - 3;
            if tray.menu.insert(&item, position).is_ok() {
                tray.targets.insert(item.id().clone(), entry.target);
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (count, entries, cx);
}
