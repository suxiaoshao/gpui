use super::*;
use crate::app::menus;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(crate) enum Kind {
    Palette,
    QuickOpen,
    New,
    Settings,
    Sidebar,
    Scan,
    FocusInput,
    ShowMain,
    Quit,
    Model,
    History,
    OpenHistory,
    Export,
    Clone,
    CopyLastAnswer,
    Compact,
    Reconnect,
    Rename,
    Stop,
    Close,
    Reveal,
    CopyPath,
    Delete,
}
impl Kind {
    pub(crate) fn search_terms(self) -> &'static str {
        match self {
            Self::Palette => "command palette",
            Self::QuickOpen => "resume search sessions quick open",
            Self::New => "new conversation",
            Self::Settings => "settings preferences",
            Self::Sidebar => "sidebar",
            Self::Scan => "scan refresh session directory",
            Self::FocusInput => "focus input composer",
            Self::ShowMain => "show main window",
            Self::Quit => "quit exit",
            Self::Model => "model thinking",
            Self::History | Self::OpenHistory => "tree fork history",
            Self::Reconnect => "reload reconnect",
            Self::Rename => "name rename",
            Self::Export => "export html",
            Self::Clone => "clone duplicate conversation",
            Self::CopyLastAnswer => "copy last assistant answer",
            Self::Compact => "compact context",
            Self::Stop => "stop abort",
            Self::Close => "close connection",
            Self::Reveal => "reveal locate file",
            Self::CopyPath => "copy path",
            Self::Delete => "delete trash",
        }
    }
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace = gupi, no_json)]
pub(crate) struct Run(pub Kind);

actions!(gupi, [ConfirmExtension, CancelExtension]);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ConfirmExtension, Some("GupiExtension")),
        KeyBinding::new("escape", CancelExtension, Some("GupiExtension")),
    ]);
    for context in ["Gupi", "GupiPalette"] {
        cx.bind_keys(
            [
                ("secondary-shift-p", Kind::Palette),
                ("secondary-p", Kind::QuickOpen),
                ("secondary-n", Kind::New),
                ("secondary-l", Kind::FocusInput),
                ("secondary-b", Kind::Sidebar),
                ("secondary-alt-b", Kind::History),
                ("secondary-r", Kind::Reconnect),
                ("secondary-shift-r", Kind::Scan),
                ("secondary-alt-/", Kind::Model),
            ]
            .map(|(key, action)| KeyBinding::new(key, Run(action), Some(context))),
        );
    }
    cx.bind_keys([KeyBinding::new("escape", Run(Kind::Stop), Some("Gupi"))]);
}
impl HomeView {
    pub(crate) fn action_enabled(&self, kind: Kind, cx: &App) -> bool {
        let state = self.state.read(cx);
        let s = state.current();
        match kind {
            Kind::Palette | Kind::QuickOpen | Kind::Settings | Kind::ShowMain | Kind::Quit => true,
            Kind::New | Kind::Sidebar => !state.draining(),
            Kind::Scan => !state.draining() && !state.scanning(),
            Kind::FocusInput | Kind::History | Kind::OpenHistory => s.is_some(),
            Kind::Export => state
                .selected
                .as_ref()
                .is_some_and(|key| state.can_export(key, cx)),
            Kind::Clone => state
                .selected
                .as_ref()
                .is_some_and(|key| state.can_clone(key, cx)),
            Kind::CopyLastAnswer => s.and_then(|s| s.last_assistant_text()).is_some(),
            Kind::Compact => state
                .selected
                .as_ref()
                .is_some_and(|key| state.can_compact(key, cx)),
            Kind::Reconnect => state
                .selected
                .as_ref()
                .is_some_and(|key| state.can_reconnect(key, cx)),
            Kind::Rename => {
                state
                    .selected
                    .as_ref()
                    .is_some_and(|key| state.can_rename(key, cx))
                    && s.is_some_and(|s| !s.info.path.as_os_str().is_empty())
            }
            Kind::Delete => state
                .selected
                .as_ref()
                .is_some_and(|key| state.can_delete(key)),
            Kind::Reveal | Kind::CopyPath => s.is_some_and(|s| !s.info.path.as_os_str().is_empty()),
            Kind::Stop => s.is_some_and(|s| {
                s.busy() && !s.stopping && (!s.command.running() || s.command.compacting())
            }),
            Kind::Close => s.is_some_and(|s| s.instance.is_some() && !s.settings_busy()),
            Kind::Model => s.is_some_and(|s| !s.settings_busy()),
        }
    }
    pub(crate) fn run_action(&mut self, action: &Run, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(action.0, Kind::Palette | Kind::QuickOpen) {
            self.open_palette(action.0 == Kind::QuickOpen, window, cx);
            return;
        }
        if window.has_active_dialog(cx) || !self.action_enabled(action.0, cx) {
            return;
        }
        let key = self.state.read(cx).selected.clone();
        match action.0 {
            Kind::New => self.new_conversation(window, cx),
            Kind::Settings => window.dispatch_action(Box::new(menus::ShowSettings), cx),
            Kind::ShowMain => window.dispatch_action(Box::new(menus::ShowMainWindow), cx),
            Kind::Quit => window.dispatch_action(Box::new(menus::Quit), cx),
            Kind::Sidebar => {
                self.show_sidebar = !self.show_sidebar;
                if !self.show_sidebar {
                    self.focus_composer(window, cx);
                }
            }
            Kind::History => self.toggle_history(window, cx),
            Kind::OpenHistory => {
                if !self.show_history {
                    self.toggle_history(window, cx);
                }
            }
            Kind::CopyLastAnswer => {
                if let Some(text) = self
                    .state
                    .read(cx)
                    .current()
                    .and_then(|s| s.last_assistant_text())
                {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            Kind::Scan => self.state.update(cx, |s, cx| s.scan(cx)),
            Kind::FocusInput => self.focus_composer(window, cx),
            Kind::Model => {
                if let Some(view) = key.as_ref().and_then(|key| self.views.get(key)) {
                    view.model_picker
                        .update(cx, |picker, cx| picker.open(window, cx));
                }
            }
            Kind::Rename => {
                if let Some(key) = key {
                    self.rename_dialog(key, window, cx);
                }
            }
            Kind::Reveal | Kind::CopyPath => {
                if let Some(s) = self.state.read(cx).current() {
                    if action.0 == Kind::Reveal {
                        cx.reveal_path(&s.info.path);
                    } else {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            s.info.path.to_string_lossy().into_owned(),
                        ));
                    }
                }
            }
            kind => {
                if let Some(key) = key {
                    self.state.update(cx, |s, cx| match kind {
                        Kind::Reconnect => s.reconnect(&key, cx),
                        Kind::Export => s.export_html(&key, cx),
                        Kind::Clone => s.clone_session(&key, cx),
                        Kind::Compact => s.compact(&key, cx),
                        Kind::Stop => s.abort(&key, cx),
                        Kind::Close => s.close(&key, cx),
                        Kind::Delete => s.delete(&key, cx),
                        _ => {}
                    });
                }
            }
        }
        cx.notify();
    }
    pub(crate) fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.state.read(cx).current() {
            if let Some(pending) = s.pending_ui.front() {
                if matches!(
                    pending.request.method,
                    pi_rpc::protocol::UiMethod::Input { .. }
                        | pi_rpc::protocol::UiMethod::Editor { .. }
                ) {
                    self.extension_input
                        .update(cx, |input, cx| input.focus(window, cx));
                } else {
                    self.extension_focus.focus(window, cx);
                }
                return;
            }
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
    }
    pub(super) fn toggle_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_history = !self.show_history;
        self.sync(true, window, cx);
        if !self.show_history
            && let Some(view) = self.shown_key.as_ref().and_then(|key| self.views.get(key))
        {
            view.history_canvas
                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
        }
        if self.show_history && self.history_view != HistoryView::Tree {
            self.history_list.update(cx, |list, cx| {
                if let Some(last) = list.delegate().rows.len().checked_sub(1) {
                    list.scroll_to_item(
                        gpui_kit::component::IndexPath::new(last),
                        ScrollStrategy::Bottom,
                        window,
                        cx,
                    );
                }
            });
        }
        if !self.show_history {
            self.focus_composer(window, cx);
        }
        cx.notify();
    }
}
