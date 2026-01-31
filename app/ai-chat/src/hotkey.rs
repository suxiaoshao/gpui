use crate::{config::AiChatConfig, errors::AiChatResult, views::temporary::TemporaryView};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::*;
use gpui_component::Root;
use std::str::FromStr;
use tracing::{Level, event};

pub struct TemporaryData {
    manager: GlobalHotKeyManager,
    _task: Task<()>,
    pub temporary_window: Option<WindowHandle<Root>>,
}

impl TemporaryData {
    fn on_short(&mut self, cx: &mut App) {
        match self.temporary_window {
            Some(temporary_window) => {
                let windows = cx.windows();
                if windows
                    .iter()
                    .any(|window| window.window_id() == temporary_window.window_id())
                {
                    match temporary_window.update(cx, |_this, window, _cx| {
                        if window.is_window_active() {
                            window.remove_window();
                            self.temporary_window = None;
                        } else {
                            window.activate_window();
                        }
                    }) {
                        Ok(_) => {}
                        Err(err) => {
                            event!(Level::ERROR, "Failed to update temporary window: {:?}", err);
                        }
                    };
                } else {
                    self.create_temporary_window(cx);
                }
            }
            None => self.create_temporary_window(cx),
        }
    }
    fn create_temporary_window(&mut self, cx: &mut App) {
        let temporary_window = match cx.open_window(
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
                let view = cx.new(|cx| TemporaryView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(data) => data,
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Failed to open temporary window");
                return;
            }
        };
        self.temporary_window = Some(temporary_window);
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
            smol::block_on(async {
                match tx.send(e).await {
                    Ok(_) => {}
                    Err(err) => {
                        event!(Level::ERROR, "send hotkey event failed: {}", err);
                    }
                };
            });
        }
    }));
    let task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            event!(Level::INFO, "hotkey event received: {:?}", event);
            match cx.update_global::<TemporaryData, _>(|this, cx| {
                this.on_short(cx);
            }) {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "open temporary window failed: {}", err);
                }
            };
        }
    });
    cx.set_global(TemporaryData {
        manager,
        _task: task,
        temporary_window: None,
    });
    Ok(())
}
