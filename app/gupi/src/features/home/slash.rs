use super::*;
use crate::features::command_palette::CommandPalette;

#[derive(Default)]
pub(super) struct Completion {
    leading_slash: bool,
    dismissed: bool,
    file_query: Option<(std::ops::Range<usize>, String)>,
    file_dismissed: bool,
}
impl Completion {
    pub(super) fn dismiss_file_query(&mut self) {
        self.file_dismissed = true;
    }
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
    pub(crate) fn select_resource(
        &mut self,
        command: pi_rpc::protocol::SlashCommand,
        from_composer: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(token) = crate::foundation::composer_resources::token(&command, true) else {
            return;
        };
        let Some(session) = self.state.read(cx).current() else {
            return;
        };
        if !session.can_edit_draft() {
            return;
        }
        let text = self.input.read(cx).value();
        let mut range = crate::foundation::composer_resources::replacement(
            &text,
            from_composer,
            session
                .commands
                .data()
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        // Pi's delimiter belongs to this one insertion transaction. Reuse an
        // existing space instead of adding a separate edit/undo step.
        if text
            .get(range.end..)
            .is_some_and(|suffix| suffix.starts_with(' '))
        {
            range.end += 1;
        }
        let end = range.start + token.text().len();
        // Dismissal precedes the Change event from this user edit.
        self.slash.leading_slash = true;
        self.slash.dismissed = true;
        self.input.update(cx, |input, cx| {
            if input
                .replace_range_with_token(range, token, window, cx)
                .is_ok()
            {
                input.set_selected_range(end..end, cx);
                input.focus(window, cx);
            }
        });
    }
    pub(super) fn open_slash_if_needed(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.input.read(cx);
        let query = file_query(text, input.cursor()).filter(|range| {
            !input
                .tokens()
                .iter()
                .any(|token| token.range().start < range.end && token.range().end > range.start)
        });
        if query.is_none() {
            self.slash.file_dismissed = false;
        }
        if let Some(range) = query
            && !self.slash.file_dismissed
            && self.input.read(cx).focus_handle(cx).is_focused(window)
            && !(window.has_active_dialog(cx) || self.has_image_preview(cx))
        {
            self.open_files(Some(range), window, cx);
            return;
        }
        if self.slash.changed(text)
            && !self
                .input
                .read(cx)
                .content()
                .tokens()
                .iter()
                .any(|token| token.range().start == 0)
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
        self.slash.file_dismissed = true;
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
        self.slash.file_query = None;
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

    pub(super) fn open_files(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) || self.has_image_preview(cx) {
            return;
        }
        let Some(session) = self.state.read(cx).current() else {
            return;
        };
        if !session.can_edit_draft() || !session.pending_ui.is_empty() {
            return;
        }
        let text = self.input.read(cx).value();
        self.slash.file_query =
            range.and_then(|range| text.get(range.clone()).map(|text| (range, text.to_owned())));
        let query = self
            .slash
            .file_query
            .as_ref()
            .map_or("@", |(_, text)| text.as_str())
            .to_owned();
        self.slash.file_dismissed = true;
        self.command_panel = Some(CommandPalette::open(
            Some((cx.weak_entity(), self.state.clone())),
            false,
            query,
            window,
            cx,
        ));
    }

    pub(crate) fn select_file(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self
            .slash
            .file_query
            .take()
            .and_then(|(range, text)| {
                (self.input.read(cx).value().get(range.clone()) == Some(&text)).then_some(range)
            })
            .unwrap_or_else(|| self.input.read(cx).selected_range());
        self.insert_file_reference(&path, range, window, cx);
    }

    pub(super) fn insert_file_reference(
        &mut self,
        path: &std::path::Path,
        mut range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .state
            .read(cx)
            .current()
            .is_some_and(|s| s.can_edit_draft())
        {
            return;
        }
        let text = self.input.read(cx).value();
        let leading_space = range.start > 0 && !text[..range.start].ends_with(char::is_whitespace);
        if text[range.end..].starts_with(' ') {
            range.end += 1;
        }
        let token = crate::foundation::composer_resources::file_token(path, leading_space);
        self.slash.file_dismissed = true;
        self.input.update(cx, |input, cx| {
            // Paths containing line breaks cannot be atomic tokens. Preserve
            // their exact reference as editable text instead of losing the file.
            match input.replace_range_with_token(range.clone(), token.clone(), window, cx) {
                Ok(()) => {}
                Err(gpui_kit::component::input::InlineTokenError::InvalidToken) => {
                    input.set_selected_range(range, cx);
                    input.replace(token.text(), window, cx);
                }
                Err(_) => return,
            }
            input.focus(window, cx);
        });
    }
}

fn file_query(text: &str, cursor: usize) -> Option<std::ops::Range<usize>> {
    let before = text.get(..cursor)?;
    let start = before.rfind('@')?;
    if start > 0 && !before[..start].ends_with(char::is_whitespace) {
        return None;
    }
    if before[start..].contains(char::is_whitespace) {
        return None;
    }
    Some(start..cursor)
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
