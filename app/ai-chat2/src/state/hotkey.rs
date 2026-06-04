use std::{collections::BTreeMap, str::FromStr, time::SystemTime};

use ai_chat_core::ShortcutId;
use ai_chat_db::ShortcutRecord;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, BorrowAppContext, Global, Task};
use tracing::{Level, event};

use crate::{
    database,
    errors::{AiChat2Error, AiChat2Result},
    state::AiChat2AppSettings,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum RegisteredHotkeyAction {
    TemporaryConversation,
    Shortcut { shortcut_id: ShortcutId },
}

impl RegisteredHotkeyAction {
    fn label(&self) -> String {
        match self {
            Self::TemporaryConversation => "temporary".to_string(),
            Self::Shortcut { shortcut_id } => format!("shortcut:{shortcut_id}"),
        }
    }
}

trait HotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChat2Result<()>;
    fn unregister(&mut self, hotkey: HotKey) -> AiChat2Result<()>;
}

struct SystemHotkeyBackend {
    manager: GlobalHotKeyManager,
}

impl HotkeyBackend for SystemHotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChat2Result<()> {
        Ok(self.manager.register(hotkey)?)
    }

    fn unregister(&mut self, hotkey: HotKey) -> AiChat2Result<()> {
        Ok(self.manager.unregister(hotkey)?)
    }
}

struct DisabledHotkeyBackend {
    error: String,
}

impl HotkeyBackend for DisabledHotkeyBackend {
    fn register(&mut self, _hotkey: HotKey) -> AiChat2Result<()> {
        Err(AiChat2Error::HotkeyUnavailable(self.error.clone()))
    }

    fn unregister(&mut self, _hotkey: HotKey) -> AiChat2Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeHotkeyBackend;

#[cfg(test)]
impl HotkeyBackend for FakeHotkeyBackend {
    fn register(&mut self, _hotkey: HotKey) -> AiChat2Result<()> {
        Ok(())
    }

    fn unregister(&mut self, _hotkey: HotKey) -> AiChat2Result<()> {
        Ok(())
    }
}

pub(crate) struct GlobalHotkeyState {
    backend: Box<dyn HotkeyBackend>,
    temporary_hotkey: Option<String>,
    registered_shortcuts: BTreeMap<ShortcutId, String>,
    hotkey_actions: BTreeMap<u32, RegisteredHotkeyAction>,
    registration_errors: BTreeMap<String, String>,
    last_pressed: Option<HotkeyPressDiagnostics>,
    _task: Task<()>,
}

impl Global for GlobalHotkeyState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HotkeyPressDiagnostics {
    pub(crate) hotkey_id: u32,
    pub(crate) action: String,
    pub(crate) pressed_at: SystemTime,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShortcutRuntimeDiagnostics {
    pub(crate) temporary_hotkey: Option<String>,
    pub(crate) registered_shortcuts: BTreeMap<ShortcutId, String>,
    pub(crate) registration_errors: BTreeMap<String, String>,
    pub(crate) last_pressed: Option<HotkeyPressDiagnostics>,
}

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let (tx, rx) = smol::channel::unbounded();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state() == HotKeyState::Pressed
            && let Err(err) = tx.send_blocking(event)
        {
            event!(Level::ERROR, error = ?err, "send hotkey event failed");
        }
    }));

    let task = cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv().await {
            let hotkey_id = event.id();
            cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                hotkeys.handle_pressed_hotkey(hotkey_id);
            });
        }
    });

    let mut manager_error = None;
    let backend: Box<dyn HotkeyBackend> = match GlobalHotKeyManager::new() {
        Ok(manager) => Box::new(SystemHotkeyBackend { manager }),
        Err(err) => {
            event!(Level::ERROR, error = ?err, "global hotkey manager unavailable");
            manager_error = Some(err.to_string());
            Box::new(DisabledHotkeyBackend {
                error: err.to_string(),
            })
        }
    };
    let mut hotkeys = GlobalHotkeyState::new(backend, task);
    if let Some(err) = manager_error {
        hotkeys
            .registration_errors
            .insert("manager".to_string(), err);
    }
    hotkeys.load_initial_shortcuts(cx)?;
    event!(
        Level::INFO,
        temporary_hotkey = ?hotkeys.temporary_hotkey,
        registered_shortcuts = hotkeys.registered_shortcuts.len(),
        registration_errors = hotkeys.registration_errors.len(),
        "initialized ai-chat2 global hotkeys"
    );
    cx.set_global(hotkeys);
    Ok(())
}

#[cfg(test)]
pub(crate) fn set_test_hotkey_state(cx: &mut App) {
    cx.set_global(GlobalHotkeyState::new(
        Box::<FakeHotkeyBackend>::default(),
        Task::ready(()),
    ));
}

impl GlobalHotkeyState {
    fn new(backend: Box<dyn HotkeyBackend>, task: Task<()>) -> Self {
        Self {
            backend,
            temporary_hotkey: None,
            registered_shortcuts: BTreeMap::new(),
            hotkey_actions: BTreeMap::new(),
            registration_errors: BTreeMap::new(),
            last_pressed: None,
            _task: task,
        }
    }

    fn parse_hotkey(hotkey: &str) -> AiChat2Result<HotKey> {
        Ok(HotKey::from_str(hotkey)?)
    }

    fn register_action(
        &mut self,
        hotkey: &str,
        action: RegisteredHotkeyAction,
    ) -> AiChat2Result<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()) == Some(&action) {
            return Ok(());
        }

        self.backend.register(hotkey)?;
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "registered ai-chat2 hotkey"
        );
        self.hotkey_actions.insert(hotkey.id(), action);
        Ok(())
    }

    fn unregister_action(
        &mut self,
        hotkey: &str,
        expected_action: RegisteredHotkeyAction,
    ) -> AiChat2Result<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()) != Some(&expected_action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                expected_action = ?expected_action,
                "skip ai-chat2 hotkey unregister because registered action does not match"
            );
            return Ok(());
        }

        self.backend.unregister(hotkey)?;
        self.hotkey_actions.remove(&hotkey.id());
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "unregistered ai-chat2 hotkey"
        );
        Ok(())
    }

    fn load_initial_shortcuts(&mut self, cx: &mut App) -> AiChat2Result<()> {
        if let Some(hotkey) = cx
            .global::<AiChat2AppSettings>()
            .temporary_hotkey()
            .map(str::to_string)
            && let Err(err) = self.register_temporary_hotkey(hotkey)
        {
            event!(Level::ERROR, error = ?err, "failed to load temporary hotkey");
        }

        let shortcuts = database::repository(cx).list_shortcuts()?;
        for shortcut in shortcuts {
            self.register_shortcut(shortcut);
        }

        Ok(())
    }

    fn register_temporary_hotkey(&mut self, hotkey: String) -> AiChat2Result<()> {
        let result = self.register_action(&hotkey, RegisteredHotkeyAction::TemporaryConversation);
        self.temporary_hotkey = Some(hotkey);

        match result {
            Ok(()) => {
                self.registration_errors.remove("temporary");
                Ok(())
            }
            Err(err) => {
                self.registration_errors
                    .insert("temporary".to_string(), err.to_string());
                event!(Level::ERROR, error = ?err, "failed to register temporary hotkey");
                Err(err)
            }
        }
    }

    pub(crate) fn update_temporary_hotkey(
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
        cx: &mut App,
    ) -> AiChat2Result<()> {
        if !cx.has_global::<GlobalHotkeyState>() {
            return Ok(());
        }

        let mut result = Ok(());
        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
            result = hotkeys.update_temporary_hotkey_runtime(old_hotkey, new_hotkey);
        });
        result
    }

    fn update_temporary_hotkey_runtime(
        &mut self,
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
    ) -> AiChat2Result<()> {
        event!(
            Level::INFO,
            old_hotkey = ?old_hotkey,
            new_hotkey = ?new_hotkey,
            "updating ai-chat2 temporary hotkey"
        );

        if old_hotkey == new_hotkey {
            self.temporary_hotkey = new_hotkey.map(str::to_string);
            return Ok(());
        }

        if let Some(old_hotkey) = old_hotkey {
            self.unregister_action(old_hotkey, RegisteredHotkeyAction::TemporaryConversation)?;
        }

        if let Some(new_hotkey) = new_hotkey
            && let Err(err) = self.register_temporary_hotkey(new_hotkey.to_string())
        {
            if let Some(old_hotkey) = old_hotkey {
                let _ = self.register_temporary_hotkey(old_hotkey.to_string());
            }
            return Err(err);
        }

        if new_hotkey.is_none() {
            self.temporary_hotkey = None;
            self.registration_errors.remove("temporary");
        }

        Ok(())
    }

    fn register_shortcut(&mut self, shortcut: ShortcutRecord) {
        if !shortcut.enabled {
            return;
        }

        let action = RegisteredHotkeyAction::Shortcut {
            shortcut_id: shortcut.id.clone(),
        };
        let result = self.register_action(&shortcut.hotkey, action);
        if let Err(err) = result {
            self.registration_errors
                .insert(shortcut.id.clone(), err.to_string());
            event!(
                Level::ERROR,
                shortcut_id = %shortcut.id,
                hotkey = %shortcut.hotkey,
                error = ?err,
                "failed to register ai-chat2 shortcut hotkey"
            );
            return;
        }

        self.registration_errors.remove(&shortcut.id);
        self.registered_shortcuts
            .insert(shortcut.id, shortcut.hotkey);
    }

    fn handle_pressed_hotkey(&mut self, hotkey_id: u32) {
        let Some(action) = self.hotkey_actions.get(&hotkey_id).cloned() else {
            event!(
                Level::INFO,
                hotkey_id,
                "ignoring ai-chat2 hotkey press with no registered action"
            );
            return;
        };

        let action_label = action.label();
        self.last_pressed = Some(HotkeyPressDiagnostics {
            hotkey_id,
            action: action_label.clone(),
            pressed_at: SystemTime::now(),
        });
        event!(
            Level::INFO,
            hotkey_id,
            action = %action_label,
            "recorded ai-chat2 hotkey press"
        );
    }

    pub(crate) fn diagnostics(&self) -> ShortcutRuntimeDiagnostics {
        ShortcutRuntimeDiagnostics {
            temporary_hotkey: self.temporary_hotkey.clone(),
            registered_shortcuts: self.registered_shortcuts.clone(),
            registration_errors: self.registration_errors.clone(),
            last_pressed: self.last_pressed.clone(),
        }
    }

    pub(crate) fn diagnostics_snapshot(cx: &App) -> ShortcutRuntimeDiagnostics {
        cx.try_global::<GlobalHotkeyState>()
            .map(GlobalHotkeyState::diagnostics)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeHotkeyBackend, GlobalHotkeyState};
    use global_hotkey::hotkey::HotKey;
    use gpui::Task;
    use std::str::FromStr;

    #[test]
    fn temporary_hotkey_registration_records_diagnostics() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        hotkeys
            .register_temporary_hotkey("cmd+shift+j".to_string())
            .expect("register temporary hotkey");

        let hotkey = HotKey::from_str("cmd+shift+j").unwrap();
        hotkeys.handle_pressed_hotkey(hotkey.id());
        let diagnostics = hotkeys.diagnostics();

        assert_eq!(diagnostics.temporary_hotkey.as_deref(), Some("cmd+shift+j"));
        assert!(diagnostics.registration_errors.is_empty());
        assert_eq!(
            diagnostics
                .last_pressed
                .as_ref()
                .map(|press| press.action.as_str()),
            Some("temporary")
        );
    }

    #[test]
    fn invalid_temporary_hotkey_is_reported_without_panicking() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        let result = hotkeys.register_temporary_hotkey("cmd+shift+".to_string());

        assert!(result.is_err());
        assert!(
            hotkeys
                .diagnostics()
                .registration_errors
                .contains_key("temporary")
        );
    }

    #[test]
    fn temporary_hotkey_runtime_update_can_clear_registration() {
        let mut hotkeys =
            GlobalHotkeyState::new(Box::<FakeHotkeyBackend>::default(), Task::ready(()));
        hotkeys
            .register_temporary_hotkey("cmd+shift+j".to_string())
            .expect("register temporary hotkey");

        hotkeys
            .update_temporary_hotkey_runtime(Some("cmd+shift+j"), None)
            .expect("clear temporary hotkey");

        assert_eq!(hotkeys.diagnostics().temporary_hotkey, None);
        assert!(hotkeys.diagnostics().registration_errors.is_empty());
    }
}
