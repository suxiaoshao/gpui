use super::actions::{Kind, Run};
use super::*;
use crate::foundation::session_catalog::SessionInfo;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::{
    Icon,
    command::{Command, CommandItem, CommandState},
    kbd::Kbd,
    tooltip::Tooltip,
};

pub(super) struct Palette {
    owner: WeakEntity<HomeView>,
    state: Entity<ConversationState>,
    search: Entity<CommandState>,
    sessions: Vec<(String, SessionInfo)>,
    error: Option<String>,
    original_focus: Option<FocusHandle>,
    focus: FocusHandle,
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
            Kind::TemporaryActions => "temporary-actions",
            Kind::PasteAnswer => "temporary-paste-answer",
            Kind::CopyTemporaryAnswer => "command-copy-last-answer",
            Kind::RevealWorkspace => "temporary-reveal-workspace",
            Kind::TrashTemporary => "conversation-delete",
            Kind::HideTemporary => "temporary-hide",
            Kind::TemporarySession(_) => "temporary-switch-session",
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
        if let Some(panel) = &self.palette {
            panel
                .read(cx)
                .search
                .clone()
                .update(cx, |search, cx| search.focus(window, cx));
            return;
        }
        if window.has_active_dialog(cx) {
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
            original_focus: focus,
            focus: cx.focus_handle(),
        });
        self.palette = Some(panel.clone());
        cx.notify();
        panel
            .read(cx)
            .search
            .clone()
            .update(cx, |s, cx| s.focus(window, cx));
    }
    pub(super) fn close_session_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = self.palette.take() {
            if let Some(focus) = panel.read(cx).original_focus.clone() {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }
    pub(super) fn render_session_search(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let panel = self.palette.clone()?;
        let focus = panel.read(cx).focus.clone();
        Some(
            deferred(
                div()
                    .id("session-search-overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .occlude()
                    .bg(cx.theme().overlay)
                    .flex()
                    .justify_center()
                    .items_start()
                    .pt_8()
                    .px_6()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.close_session_search(window, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .id("session-search-surface")
                            .debug_selector(|| "session-search-surface".into())
                            .w_full()
                            .max_w(px(640.))
                            .min_w_0()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius_lg)
                            .shadow_lg()
                            .overflow_hidden()
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .track_focus(&focus)
                            .child(panel)
                            .focus_trap("session-search-focus", &focus),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
impl Palette {
    fn dismiss(&self, window: &mut Window, cx: &mut App) {
        if let Some(focus) = &self.original_focus {
            focus.focus(window, cx);
        }
        let _ = self.owner.update(cx, |home, cx| {
            home.palette = None;
            cx.notify();
        });
    }
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
        self.dismiss(window, cx);
        self.state.update(cx, |state, cx| state.open(&key, cx));
    }
}
impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                let row_id = format!("session-search-row-{key}");
                CommandItem::new()
                    .label(title.clone())
                    .keywords([path.clone(), info.first_message.clone()])
                    .disabled(!infos.iter().any(|(k, _)| k == key))
                    .child(move |_, cx| {
                        let tooltip = format!("{title}\n{path}");
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap_3()
                            .child(Icon::new(IconName::MessageCircle).size_4().flex_none())
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().w_full().truncate().child(title.clone()))
                                    .child(
                                        div()
                                            .w_full()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(path.clone()),
                                    ),
                            )
                            .id(row_id.clone())
                            .tooltip(move |window, cx| {
                                Tooltip::new(tooltip.clone()).build(window, cx)
                            })
                    })
            })
            .collect::<Vec<_>>();
        let scanning = self.state.read(cx).scanning();
        let empty = self.sessions.is_empty();
        let mut content = v_flex().min_w_0().key_context("GupiSessionSearch");
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
        let cancel = cx.weak_entity();
        let dismiss = cx.weak_entity();
        let confirm = cx.weak_entity();
        let max_height = (f32::from(window.viewport_size().height) - 180.).clamp(120., 360.);
        content
            .on_action(move |action: &Run, window, cx| {
                if action.0 == Kind::Stop {
                    let _ = owner.update(cx, |home, cx| home.close_session_search(window, cx));
                } else if matches!(action.0, Kind::Palette | Kind::QuickOpen) {
                    let _ = owner.update(cx, |home, cx| home.run_action(action, window, cx));
                }
            })
            .child(
                Command::new(&self.search)
                    .bordered(false)
                    .max_h(px(max_height))
                    .items(items)
                    .on_cancel(move |window, cx| {
                        let _ = cancel.update(cx, |panel, cx| {
                            panel.dismiss(window, cx);
                        });
                    })
                    .footer(move |state, window, cx| {
                        let dismiss = dismiss.clone();
                        let confirm = confirm.clone();
                        let selected = state.selected_index();
                        h_flex()
                            .px_3()
                            .py_1()
                            .gap_2()
                            .justify_end()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                Button::new("session-search-dismiss")
                                    .debug_selector(|| "session-search-dismiss".into())
                                    .ghost()
                                    .small()
                                    .label(t(cx, "command-dismiss"))
                                    .children(Kbd::binding_for_action(
                                        &gpui_kit::base::actions::Cancel,
                                        Some("Command"),
                                        window,
                                    ))
                                    .on_click(move |_, window, cx| {
                                        let _ = dismiss.update(cx, |panel, cx| {
                                            panel.dismiss(window, cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("session-search-open")
                                    .ghost()
                                    .small()
                                    .label(t(cx, "session-search-open"))
                                    .disabled(selected.is_none())
                                    .children(Kbd::binding_for_action(
                                        &gpui_kit::base::actions::Confirm { secondary: false },
                                        Some("Command"),
                                        window,
                                    ))
                                    .on_click(move |_, window, cx| {
                                        if let Some(index) = selected {
                                            let _ = confirm.update(cx, |panel, cx| {
                                                panel.confirm(index.row, window, cx)
                                            });
                                        }
                                    }),
                            )
                    })
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

#[cfg(test)]
mod tests {
    use super::HomeView;
    use crate::{
        foundation::session_catalog::{Catalog, SessionInfo},
        state::{
            config::AppLanguage,
            conversation::{ConversationState, catalog::CatalogState},
            pi,
        },
    };
    use gpui_kit::{
        AppContext, Entity, Modifiers, TestAppContext, VisualTestContext,
        component::{Root, WindowExt as _},
        point, px, size,
    };

    fn setup(cx: &mut TestAppContext) -> (Entity<HomeView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            app_theme::init(cx);
            crate::state::theme::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
            pi::init(cx);
            cx.set_global(crate::state::layout::LayoutState::default());
        });
        let state = cx.new(|cx| ConversationState::new("unused-pi".into(), cx));
        state.update(cx, |s, _| {
            s.selected = Some("unloaded".into());
            s.catalog = CatalogState::Ready(Catalog {
                sessions: vec![SessionInfo {
                    path: "/tmp/search-fixture.jsonl".into(),
                    id: "search-fixture".into(),
                    cwd: "/tmp/long-project-path".into(),
                    name: Some("Long conversation title ".repeat(30)),
                    first_message: String::new(),
                    activity: "1".into(),
                    parent_session: None,
                }],
                ..Default::default()
            });
        });
        let mut home = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| HomeView::with_state(state, window, cx));
            home = Some(view.clone());
            Root::new(view, window, cx)
        });
        let home = home.unwrap();
        visual.simulate_resize(size(px(960.), px(740.)));
        visual.update(|window, cx| {
            home.update(cx, |home, cx| {
                home.focus_handle.focus(window, cx);
                home.open_palette(true, window, cx);
            })
        });
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        (home, visual)
    }
    #[gpui_kit::test]
    fn search_overlay_is_not_a_dialog_and_outside_click_restores_focus(cx: &mut TestAppContext) {
        let (home, cx) = setup(cx);
        cx.update(|window, cx| assert!(!window.has_active_dialog(cx)));
        assert!(cx.debug_bounds("session-search-surface").is_some());
        cx.simulate_click(point(px(8.), px(500.)), Modifiers::default());
        cx.update(|window, cx| {
            assert!(home.read(cx).palette.is_none());
            assert!(home.read(cx).focus_handle.is_focused(window));
        });
    }
    #[gpui_kit::test]
    fn search_escape_and_footer_dismiss_without_entity_reentry(cx: &mut TestAppContext) {
        let (home, cx) = setup(cx);
        cx.simulate_input("unmatched query");
        cx.simulate_keystrokes("escape");
        // Command clears its query first, then dismisses on the second Escape.
        cx.simulate_keystrokes("escape");
        cx.update(|_, cx| assert!(home.read(cx).palette.is_none()));
        cx.update(|window, cx| home.update(cx, |home, cx| home.open_palette(true, window, cx)));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let button = cx.debug_bounds("session-search-dismiss").unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        cx.update(|_, cx| assert!(home.read(cx).palette.is_none()));
    }
}
