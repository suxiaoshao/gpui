use super::*;
use crate::features::command_palette::CommandPalette;
use gpui_kit::component::WindowExt;
impl StartupView {
    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_quitting() {
            return;
        }
        if window.has_active_dialog(cx) {
            if let Some(panel) = &self.palette
                && panel.read(cx).is_open
            {
                panel.update(cx, |panel, cx| panel.focus_input(window, cx));
            }
            return;
        }
        self.palette = Some(CommandPalette::open(None, false, String::new(), window, cx));
    }
}
