use super::*;
use crate::features::command_palette::CommandPalette;

#[derive(Default)]
pub(super) struct Completion {
    leading_slash: bool,
    dismissed: bool,
}
impl Completion {
    fn changed(&mut self, text: &str) -> bool {
        let leading = text.starts_with('/');
        if !leading {
            self.dismissed = false;
        }
        let open = leading && !self.leading_slash && !self.dismissed;
        self.leading_slash = leading;
        open
    }
}
impl HomeView {
    pub(super) fn open_slash_if_needed(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.slash.changed(text)
            && self.input.read(cx).focus_handle(cx).is_focused(window)
            && !(window.has_active_dialog(cx) || self.has_image_preview(cx))
            && self.command_input_allowed(cx)
        {
            self.open_commands(true, window, cx);
        }
    }
    pub(crate) fn command_input_allowed(&self, cx: &App) -> bool {
        let state = self.state.read(cx);
        let Some(key) = state.selected.as_ref() else {
            return false;
        };
        let Some(s) = state.current() else {
            return false;
        };
        let preview = self.views.get(key).and_then(|v| v.preview.as_deref());
        state.can_submit(key, cx) && preview.is_none_or(|id| s.history().on_current_path(id))
    }
    pub(crate) fn command_label(&self, kind: actions::Kind, cx: &App) -> String {
        if kind == actions::Kind::Stop
            && self.state.read(cx).temporary
            && self.state.read(cx).current().is_none_or(|s| !s.busy())
        {
            return t(cx, "temporary-hide");
        }
        palette::label(kind, self.show_sidebar, self.show_history, cx)
    }
    pub(crate) fn commands_closed(&mut self, from_composer: bool, cx: &mut Context<Self>) {
        self.command_panel = None;
        if from_composer {
            self.slash.leading_slash = self.input.read(cx).value().starts_with('/');
            self.slash.dismissed = self.slash.leading_slash;
        }
        cx.notify();
    }
    pub(crate) fn close_commands(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = self.command_panel.take() {
            let from_composer = panel.read(cx).opened_from_composer();
            window.close_dialog(cx);
            self.commands_closed(from_composer, cx);
        }
    }
    pub(crate) fn open_commands(
        &mut self,
        from_composer: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self.command_panel.clone() {
            panel.update(cx, |panel, cx| panel.focus_input(window, cx));
            return;
        }
        self.close_session_search(window, cx);
        if window.has_active_dialog(cx) || self.has_image_preview(cx) {
            return;
        }
        self.command_panel = Some(CommandPalette::open(
            Some((cx.weak_entity(), self.state.clone())),
            from_composer,
            if from_composer {
                self.input.read(cx).value().to_string()
            } else {
                String::new()
            },
            window,
            cx,
        ));
    }
}
#[cfg(test)]
mod tests {
    use super::Completion;
    #[test]
    fn escape_dismissal_survives_edits_until_leading_slash_is_removed() {
        let mut c = Completion::default();
        assert!(c.changed("/"));
        c.dismissed = true;
        assert!(!c.changed("/review"));
        assert!(!c.changed("/rev"));
        assert!(!c.changed(""));
        assert!(c.changed("/"));
        assert!(!c.changed("/ 参数"));
    }
}
