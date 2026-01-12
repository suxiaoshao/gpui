use crate::{config::AiChatConfig, errors::AiChatResult, views::temporary::TemporaryView};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::*;
use gpui_component::Root;
use std::str::FromStr;
use tracing::{Level, event};

struct TemporaryData {
    _manager: GlobalHotKeyManager,
    _task: Task<()>,
    temporary_window: Option<WindowHandle<Root>>,
}

impl TemporaryData {
    fn on_short(&mut self, cx: &mut App) {
        match self.temporary_window {
            Some(temporary_window) => {
                let windows = cx.windows().to_vec();
                for window in windows {
                    if window.window_id() == temporary_window.window_id() {
                        match temporary_window.update(cx, |this, window, cx| {
                            if window.is_window_active() {
                                window.remove_window();
                                self.temporary_window = None;
                            } else {
                                window.activate_window();
                            }
                        }) {
                            Ok(_) => {}
                            Err(err) => {
                                event!(
                                    Level::ERROR,
                                    "Failed to update temporary window: {:?}",
                                    err
                                );
                            }
                        };
                        return;
                    }
                }
                self.create_temporary_window(cx);
            }
            None => self.create_temporary_window(cx),
        }
    }
    fn create_temporary_window(&mut self, cx: &mut App) {
        let temporary_window = match cx.open_window(
            WindowOptions {
                kind: WindowKind::PopUp,
                titlebar: Some(TitlebarOptions {
                    appears_transparent: false,
                    ..Default::default()
                }),
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
    let config = AiChatConfig::get()?;
    if let Some(temporary_hotkey) = config.temporary_hotkey {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = HotKey::from_str(&temporary_hotkey)?;
        manager.register(hotkey)?;
        event!(Level::INFO, "hotkey registered {}", hotkey);
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
            _manager: manager,
            _task: task,
            temporary_window: None,
        });
    }
    Ok(())
}
