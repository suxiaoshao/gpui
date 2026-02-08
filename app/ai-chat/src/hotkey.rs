use crate::{config::AiChatConfig, errors::AiChatResult, views::temporary::TemporaryView};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::*;
use gpui_component::Root;
use std::{any::TypeId, str::FromStr};
use tracing::{Level, event};
use window_ext::WindowExt;
#[cfg(target_os = "macos")]
pub use window_ext::{NSRunningApplication, Retained, record_frontmost_app, restore_frontmost_app};

pub struct TemporaryData {
    manager: GlobalHotKeyManager,
    _task: Task<()>,
    #[cfg(target_os = "macos")]
    front_app: Option<Retained<NSRunningApplication>>,
}

impl TemporaryData {
    pub fn hide(&mut self, window: &mut Window) {
        if let Err(err) = window.hide() {
            event!(Level::ERROR, "Failed to hide temporary window: {:?}", err);
        };
        #[cfg(target_os = "macos")]
        {
            restore_frontmost_app(&self.front_app);
            self.front_app = None;
        }
    }
    fn show(&mut self, window: &mut Window) {
        #[cfg(target_os = "macos")]
        {
            let prev_app = record_frontmost_app();
            self.front_app = prev_app;
        }
        if let Err(err) = window.show() {
            event!(Level::ERROR, "Failed to show temporary window: {:?}", err);
        };
    }
    fn on_short(&mut self, cx: &mut App) {
        let temporary_window = cx.windows().iter().find_map(|window| {
            window.downcast::<Root>().filter(|root| {
                root.read(cx)
                    .ok()
                    .map(|root| root.view().entity_type() == TypeId::of::<TemporaryView>())
                    .unwrap_or(false)
            })
        });
        match temporary_window {
            Some(temporary_window) => {
                if let Err(err) = temporary_window.update(cx, |_this, window, _cx| {
                    if window.is_showing().unwrap_or(false) {
                        self.hide(window);
                    } else {
                        self.show(window);
                    }
                }) {
                    event!(Level::ERROR, "Failed to update temporary window: {:?}", err);
                };
            }
            None => self.create_temporary_window(cx),
        }
    }
    fn create_temporary_window(&mut self, cx: &mut App) {
        #[cfg(target_os = "macos")]
        {
            let front_app = record_frontmost_app();
            self.front_app = front_app;
        }
        if let Err(err) = cx.open_window(
            WindowOptions {
                kind: WindowKind::Floating,
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(-100.), px(-100.))),
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    cx.primary_display().map(|display| display.id()),
                    size(px(800.), px(600.)),
                    cx,
                ))),
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                if let Err(err) = window.set_floating() {
                    event!(Level::ERROR, error = ?err, "Failed to set floating");
                }
                let view = cx.new(|cx| TemporaryView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, error = ?err, "Failed to open temporary window");
        };
    }
    pub fn update_hotkey(
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
        cx: &mut App,
    ) -> AiChatResult<()> {
        let temporary_data = cx.global_mut::<TemporaryData>();
        if let Some(old_hotkey) = old_hotkey {
            let old_hotkey = HotKey::from_str(old_hotkey)?;
            temporary_data.manager.unregister(old_hotkey)?;
        }
        if let Some(new_hotkey) = new_hotkey {
            let new_hotkey = HotKey::from_str(new_hotkey)?;
            temporary_data.manager.register(new_hotkey)?;
        }
        Ok(())
    }
}

impl Global for TemporaryData {}

pub fn init(cx: &mut App) {
    let span = tracing::info_span!("hotkey::init");
    let _enter = span.enter();
    event!(Level::INFO, "hotkey init");
    match inner_init(cx) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, error = ?err, "Failed to initialize hotkeys");
        }
    };
}

fn inner_init(cx: &mut App) -> AiChatResult<()> {
    let manager = GlobalHotKeyManager::new()?;
    let config = AiChatConfig::get()?;
    if let Some(temporary_hotkey) = config.temporary_hotkey {
        let hotkey = HotKey::from_str(&temporary_hotkey)?;
        manager.register(hotkey)?;
        event!(Level::INFO, "hotkey registered {}", hotkey);
    }
    let (tx, rx) = smol::channel::unbounded();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if let GlobalHotKeyEvent {
            state: HotKeyState::Pressed,
            ..
        } = e
        {
            match tx.send_blocking(e) {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "send hotkey event failed: {}", err);
                }
            };
        }
    }));
    let task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            event!(Level::INFO, "hotkey event received: {:?}", event);
            if let Err(err) = cx.update_global::<TemporaryData, _>(|this, cx| {
                this.on_short(cx);
            }) {
                event!(Level::ERROR, "open temporary window failed: {}", err);
            };
        }
    });
    cx.set_global(TemporaryData {
        manager,
        _task: task,
        #[cfg(target_os = "macos")]
        front_app: None,
    });
    Ok(())
}
