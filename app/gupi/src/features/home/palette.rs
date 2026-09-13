use super::actions::{Kind, Run};
use super::*;
use crate::foundation::session_catalog::SessionInfo;
use gpui_kit::component::command::{Command, CommandItem, CommandState};

pub(super) struct Palette {
    owner: WeakEntity<HomeView>,
    state: Entity<ConversationState>,
    search: Entity<CommandState>,
    sessions: Vec<(String, SessionInfo)>,
    error: Option<String>,
    _subscription: Subscription,
}
pub(crate) const APP: &[Kind] = &[
    Kind::New,
    Kind::QuickOpen,
    Kind::Settings,
    Kind::Sidebar,
    Kind::Scan,
    Kind::FocusInput,
    Kind::ShowMain,
    Kind::Quit,
];
pub(crate) const SESSION: &[Kind] = &[
    Kind::Model,
    Kind::Compact,
    Kind::OpenHistory,
    Kind::Export,
    Kind::Clone,
    Kind::CopyLastAnswer,
    Kind::Reconnect,
    Kind::Rename,
    Kind::Stop,
    Kind::Close,
    Kind::Reveal,
    Kind::CopyPath,
    Kind::Delete,
];
pub(crate) fn label(kind: Kind, sidebar: bool, history: bool, cx: &App) -> String {
    t(
        cx,
        match kind {
            Kind::New => "conversation-new",
            Kind::QuickOpen => "conversation-search",
            Kind::Settings => "menu-settings",
            Kind::Sidebar => {
                if sidebar {
                    "conversation-hide-sidebar"
                } else {
                    "conversation-show-sidebar"
                }
            }
            Kind::Scan => "conversation-refresh",
            Kind::FocusInput => "command-focus-input",
            Kind::ShowMain => "menu-show-main",
            Kind::Quit => "menu-quit",
            Kind::Model => "command-model",
            Kind::Compact => "command-compact",
            Kind::OpenHistory => "conversation-history",
            Kind::Export => "conversation-export",
            Kind::Clone => "conversation-clone",
            Kind::CopyLastAnswer => "command-copy-last-answer",
            Kind::History => {
                if history {
                    "command-hide-history"
                } else {
                    "command-show-history"
                }
            }
            Kind::Reconnect => "conversation-reconnect",
            Kind::Rename => "conversation-rename",
            Kind::Stop => "conversation-stop",
            Kind::Close => "conversation-close-run",
            Kind::Reveal => "action-locate",
            Kind::CopyPath => "conversation-copy-path",
            Kind::Delete => "conversation-delete",
            Kind::Palette => "command-palette",
        },
    )
}
impl HomeView {
    pub(super) fn open_palette(
        &mut self,
        quick: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !quick {
            self.open_commands(false, window, cx);
            return;
        }
        if self.command_panel.is_some() {
            self.close_commands(window, cx);
        }
        if window.has_active_dialog(cx) {
            if let Some((_, panel)) = &self.palette {
                panel
                    .read(cx)
                    .search
                    .clone()
                    .update(cx, |s, cx| s.focus(window, cx));
            }
            return;
        }
        let focus = window.focused(cx);
        let owner = cx.weak_entity();
        let state = self.state.clone();
        let sessions = state.read(cx).infos();
        let panel = cx.new(|cx| Palette {
            owner: owner.clone(),
            _subscription: cx.observe(&state, |_, _, cx| cx.notify()),
            state,
            sessions,
            search: cx.new(|cx| CommandState::new(window, cx)),
            error: None,
        });
        let id = panel.entity_id();
        self.palette = Some((true, panel.clone()));
        let shown = panel.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone();
            let focus = focus.clone();
            dialog
                .title(t(cx, "conversation-search"))
                .w(px(560.))
                .child(shown.clone())
                .on_close(move |_, window, cx| {
                    if let Some(focus) = &focus {
                        focus.focus(window, cx);
                    }
                    let owner = owner.clone();
                    window.defer(cx, move |_, cx| {
                        let _ = owner.update(cx, |home, _| {
                            if home
                                .palette
                                .as_ref()
                                .is_some_and(|(_, p)| p.entity_id() == id)
                            {
                                home.palette = None;
                            }
                        });
                    });
                })
        });
        panel
            .read(cx)
            .search
            .clone()
            .update(cx, |s, cx| s.focus(window, cx));
    }
}
impl Palette {
    fn confirm(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, _)) = self.sessions.get(row) else {
            return;
        };
        if !self.state.read(cx).infos().iter().any(|(k, _)| k == key) {
            self.error = Some(t(cx, "command-target-changed"));
            cx.notify();
            return;
        }
        let key = key.clone();
        window.close_dialog(cx);
        let _ = self.owner.update(cx, |home, _| home.palette = None);
        self.state.update(cx, |state, cx| state.open(&key, cx));
    }
}
impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let infos = self.state.read(cx).infos();
        for (key, info) in &infos {
            if let Some((_, previous)) = self.sessions.iter_mut().find(|(k, _)| k == key) {
                *previous = info.clone();
            } else {
                self.sessions.push((key.clone(), info.clone()));
            }
        }
        let items = self
            .sessions
            .iter()
            .map(|(key, info)| {
                let title = navigation::display_title(info, cx);
                let path = info.cwd.to_string_lossy().into_owned();
                CommandItem::new()
                    .label(title.clone())
                    .keywords([path.clone(), info.first_message.clone()])
                    .disabled(!infos.iter().any(|(k, _)| k == key))
                    .child(move |_, cx| {
                        v_flex().min_w_0().child(title.clone()).child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .text_ellipsis()
                                .child(path.clone()),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let scanning = self.state.read(cx).scanning();
        let empty = self.sessions.is_empty();
        let mut content = v_flex().gap_2().key_context("GupiPalette");
        if scanning {
            content = content.child(div().text_sm().child(t(cx, "command-scanning")));
        }
        if let Some(error) = self.state.read(cx).catalog.error() {
            content = content
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.to_owned()),
                )
                .child(
                    Button::new("retry-catalog")
                        .small()
                        .label(t(cx, "action-retry"))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.scan(cx))),
                        ),
                );
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        let owner = self.owner.clone();
        let panel = cx.weak_entity();
        content
            .on_action(move |action: &Run, window, cx| {
                let _ = owner.update(cx, |home, cx| home.run_action(action, window, cx));
            })
            .child(
                Command::new(&self.search)
                    .items(items)
                    .placeholder(t(cx, "conversation-search-placeholder"))
                    .empty(move |_, _, cx| {
                        div().p_4().child(t(
                            cx,
                            if scanning {
                                "command-scanning"
                            } else if empty {
                                "conversation-catalog-empty"
                            } else {
                                "conversation-search-empty"
                            },
                        ))
                    })
                    .on_confirm(move |index, window, cx| {
                        let _ = panel.update(cx, |this, cx| this.confirm(index.row, window, cx));
                    }),
            )
    }
}
