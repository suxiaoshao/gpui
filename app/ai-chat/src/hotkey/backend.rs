use super::{AiChatResult, GlobalHotKeyManager, HotKey, HotkeyBackend};

pub(super) struct SystemHotkeyBackend {
    pub(super) manager: GlobalHotKeyManager,
}

impl HotkeyBackend for SystemHotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> AiChatResult<()> {
        self.manager.register(hotkey)?;
        Ok(())
    }

    fn unregister(&mut self, hotkey: HotKey) -> AiChatResult<()> {
        self.manager.unregister(hotkey)?;
        Ok(())
    }
}
