mod backend;
mod registry;
mod shortcut_flow;
mod temporary_window;

use crate::{
    database::{Db, GlobalShortcutBinding, NewGlobalShortcutBinding, UpdateGlobalShortcutBinding},
    errors::AiChatResult,
    i18n::I18n,
    platform::{
        capture::CaptureError,
        display::{TEMPORARY_WINDOW_SIZE, recentered_bounds_for_display, target_display_id},
    },
    state::{AiChatConfig, ConversationDraft, ModelStore},
    views::{screenshot::overlay as screenshot_overlay, temporary::TemporaryView},
};
use get_selected_text::get_selected_text;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::*;
use gpui_component::{
    Root, WindowExt as NotificationWindowExt,
    notification::{Notification, NotificationType},
};
#[cfg(target_os = "macos")]
pub use platform_ext::app::{NSRunningApplication, Retained};
#[cfg(target_os = "macos")]
use platform_ext::app::{record_frontmost_app, restore_frontmost_app};
use platform_ext::{OcrError, ocr::ImageFrame};
use std::{collections::BTreeMap, str::FromStr, time::Duration};
use tracing::{Level, event};
use window_ext::WindowExt;

use self::backend::SystemHotkeyBackend;
#[cfg(target_os = "macos")]
pub(crate) use self::temporary_window::record_front_app_for_temporary_window;
pub(crate) use self::temporary_window::{
    init_temporary_window_state, open_temporary_window, toggle_temporary_window,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisteredHotkeyAction {
    TemporaryWindow,
    ShortcutBinding { binding_id: i32 },
}

trait HotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChatResult<()>;
    fn unregister(&mut self, hotkey: HotKey) -> AiChatResult<()>;
}

pub struct GlobalHotkeyState {
    backend: Box<dyn HotkeyBackend>,
    temporary_hotkey: Option<String>,
    shortcut_bindings: BTreeMap<i32, GlobalShortcutBinding>,
    hotkey_actions: BTreeMap<u32, RegisteredHotkeyAction>,
    _task: Task<()>,
    #[cfg(target_os = "macos")]
    front_app: Option<Retained<NSRunningApplication>>,
}

impl Global for GlobalHotkeyState {}

pub fn init(cx: &mut App) {
    let span = tracing::info_span!("hotkey::init");
    let _enter = span.enter();
    event!(Level::INFO, "hotkey init");
    init_temporary_window_state(cx);
    match inner_init(cx) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, error = ?err, "Failed to initialize hotkeys");
        }
    };
}

fn inner_init(cx: &mut App) -> AiChatResult<()> {
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
            cx.update_global::<GlobalHotkeyState, _>(|hotkeys, cx| {
                hotkeys.handle_pressed_hotkey(event.id(), cx);
            });
        }
    });

    let mut hotkeys = GlobalHotkeyState {
        backend: Box::new(SystemHotkeyBackend {
            manager: GlobalHotKeyManager::new()?,
        }),
        temporary_hotkey: None,
        shortcut_bindings: BTreeMap::new(),
        hotkey_actions: BTreeMap::new(),
        _task: task,
        #[cfg(target_os = "macos")]
        front_app: None,
    };
    hotkeys.load_initial_shortcuts(cx)?;
    cx.set_global(hotkeys);
    Ok(())
}
