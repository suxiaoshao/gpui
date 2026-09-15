use super::home::{
    HomeView,
    actions::{Kind, Run},
    palette::{APP, SESSION},
};
use crate::{
    app::menus,
    foundation::{assets::IconName, i18n::t},
    state::conversation::ConversationState,
};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    command::{Command, CommandGroup, CommandItem, CommandState},
    h_flex,
    input::{self, InputEvent, Textarea, TextareaState},
    kbd::Kbd,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use pi_rpc::protocol::StreamingBehavior;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Id {
    Local(Kind),
    Pi(String),
}
#[derive(Clone)]
struct Row {
    id: Id,
    group: usize,
    title: String,
    description: String,
    detail: String,
    tooltip: String,
    icon: IconName,
    enabled: bool,
}
impl Row {
    fn matches(&self, query: &str) -> bool {
        let keywords = match self.id {
            Id::Local(kind) => kind.search_terms(),
            Id::Pi(_) => "",
        };
        let haystack = format!("{} {} {keywords}", self.title, self.description).to_lowercase();
        query.split_whitespace().all(|word| haystack.contains(word))
    }
}
type Home = (WeakEntity<HomeView>, Entity<ConversationState>);
pub(crate) struct CommandPalette {
    home: Option<Home>,
    from_composer: bool,
    target: Option<(String, u64)>,
    input: Entity<TextareaState>,
    list: Entity<CommandState>,
    rows: Vec<Row>,
    completed: Option<(String, usize)>,
    submission: Option<Task<()>>,
    original_focus: Option<FocusHandle>,
    pub(crate) is_open: bool,
    _subscriptions: Vec<Subscription>,
}
/// Only the editable name participates in completion. Arguments remain ordinary text.
fn query(text: &str, cursor: usize) -> Option<String> {
    if text.starts_with('/') {
        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        if cursor > end || text[1..end].contains('/') {
            return None;
        }
        Some(text[1..end].to_lowercase())
    } else {
        Some(text.to_lowercase())
    }
}
fn complete(text: &str, name: &str) -> String {
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let suffix = &text[end..];
    format!("/{name}{}", if suffix.is_empty() { " " } else { suffix })
}
fn group_label(group: usize) -> &'static str {
    match group {
        0 => "command-group-app",
        1 => "command-scope-current",
        2 => "command-group-extensions",
        3 => "command-group-skills",
        _ => "command-group-prompts",
    }
}
fn local_icon(kind: Kind) -> IconName {
    match kind {
        Kind::New => IconName::Plus,
        Kind::Settings => IconName::Settings,
        Kind::Model => IconName::Brain,
        Kind::History | Kind::OpenHistory => IconName::GitBranch,
        Kind::Export => IconName::Download,
        Kind::Clone | Kind::CopyLastAnswer => IconName::Copy,
        Kind::Compact => IconName::FileText,
        Kind::Sidebar => IconName::PanelRight,
        Kind::QuickOpen => IconName::Search,
        Kind::Scan | Kind::Reconnect => IconName::RotateCw,
        Kind::Rename => IconName::SquarePen,
        Kind::CopyPath => IconName::Copy,
        Kind::Reveal => IconName::FolderOpen,
        Kind::FocusInput | Kind::ShowMain => IconName::MessageCircle,
        Kind::Stop => IconName::Square,
        _ => IconName::X,
    }
}
pub(crate) fn binding(kind: Kind, window: &Window) -> Option<Kbd> {
    match kind {
        Kind::Settings => Kbd::binding_for_action(&menus::ShowSettings, None, window),
        Kind::Quit => Kbd::binding_for_action(&menus::Quit, None, window),
        _ => Kbd::binding_for_action(&Run(kind), Some("Gupi"), window),
    }
}
impl CommandPalette {
    pub(crate) fn opened_from_composer(&self) -> bool {
        self.from_composer
    }
    pub(crate) fn open(
        home: Option<Home>,
        from_composer: bool,
        initial: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let target = home.as_ref().and_then(|(_, state)| {
            let state = state.read(cx);
            state
                .selected
                .as_ref()
                .and_then(|key| state.sessions.get(key).map(|s| (key.clone(), s.binding)))
        });
        let original_focus = window.focused(cx);
        let input = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .placeholder(t(cx, "command-search-placeholder"))
                .submit_on_enter(true);
            let cursor = initial.len();
            input.set_value(initial, window, cx);
            input.set_selected_range(cursor..cursor, cx);
            input
        });
        let panel = cx.new(|cx| {
            let mut subscriptions = vec![
                cx.subscribe_in(&input, window, |this: &mut Self, _, event, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.completed = None;
                        cx.notify();
                    }
                }),
                cx.observe(&input, |_, _, cx| cx.notify()),
            ];
            if let Some((_, state)) = &home {
                subscriptions.push(cx.observe_in(
                    state,
                    window,
                    |this: &mut Self, _, window, cx| this.sync(window, cx),
                ));
            }
            Self {
                home,
                from_composer,
                target,
                input: input.clone(),
                list: cx.new(|cx| CommandState::new(window, cx)),
                rows: vec![],
                completed: None,
                submission: None,
                original_focus,
                is_open: true,
                _subscriptions: subscriptions,
            }
        });
        let shown = panel.clone();
        let weak = panel.downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .close_button(false)
                .p_0()
                .w(px(640.))
                .child(shown.clone())
                .on_close(move |_, window, cx| {
                    let _ = weak.update(cx, |panel, cx| panel.finished(window, cx));
                })
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        panel
    }
    pub(crate) fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.completed = None;
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    fn finished(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_open = false;
        if let Some((owner, _)) = &self.home {
            let from_composer = self.from_composer;
            let _ = owner.update(cx, |home, cx| home.commands_closed(from_composer, cx));
        }
        if let Some(focus) = &self.original_focus {
            focus.focus(window, cx);
        }
        if let Some((home, state)) = &self.home
            && state
                .read(cx)
                .current()
                .is_some_and(|s| !s.pending_ui.is_empty())
        {
            let _ = home.update(cx, |home, cx| home.focus_composer(window, cx));
        }
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        self.finished(window, cx);
    }
    fn target_valid(&self, cx: &App) -> bool {
        let Some((_, state)) = &self.home else {
            return false;
        };
        let state = state.read(cx);
        self.target.as_ref().is_some_and(|(key, binding)| {
            state.selected.as_ref() == Some(key)
                && state
                    .sessions
                    .get(key)
                    .is_some_and(|s| s.binding == *binding)
        })
    }
    fn can_send(&self, cx: &App) -> bool {
        self.target_valid(cx)
            && self.home.as_ref().is_some_and(|(home, _)| {
                home.read_with(cx, |home, cx| home.command_input_allowed(cx))
                    .unwrap_or(false)
            })
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, state)) = &self.home else {
            return;
        };
        let state = state.read(cx);
        let Some((key, binding)) = self.target.clone() else {
            cx.notify();
            return;
        };
        if state.selected.as_ref() != Some(&key) || !state.sessions.contains_key(&key) {
            self.close(window, cx);
            return;
        }
        let s = &state.sessions[&key];
        if binding != s.binding || !s.pending_ui.is_empty() {
            self.close(window, cx);
            return;
        }
        cx.notify();
    }
    fn candidates(&self, cx: &App) -> Vec<Row> {
        let input = self.input.read(cx);
        let text = input.value();
        if self
            .completed
            .as_ref()
            .is_some_and(|(value, cursor)| value == text.as_ref() && *cursor == input.cursor())
        {
            return vec![];
        }
        let Some(query) = query(&text, input.cursor()) else {
            return vec![];
        };
        let mut rows = vec![];
        let local = if self.home.is_some() {
            APP
        } else {
            &[Kind::ShowMain, Kind::Settings, Kind::Quit]
        };
        for (group, kinds) in [
            (0, local),
            (1, if self.target.is_some() { SESSION } else { &[] }),
        ] {
            for &kind in kinds {
                let (title, enabled) = if let Some((home, _)) = &self.home {
                    home.read_with(cx, |home, cx| {
                        (home.command_label(kind, cx), home.action_enabled(kind, cx))
                    })
                    .unwrap_or_default()
                } else {
                    (
                        t(
                            cx,
                            match kind {
                                Kind::Settings => "menu-settings",
                                Kind::Quit => "menu-quit",
                                _ => "menu-show-main",
                            },
                        ),
                        true,
                    )
                };
                rows.push(Row {
                    id: Id::Local(kind),
                    group,
                    title,
                    description: if kind == Kind::OpenHistory {
                        t(cx, "command-history-description")
                    } else {
                        String::new()
                    },
                    detail: String::new(),
                    tooltip: String::new(),
                    icon: local_icon(kind),
                    enabled: enabled && (group == 0 || self.target_valid(cx)),
                });
            }
        }
        if self.target_valid(cx)
            && let Some((_, state)) = &self.home
            && let Some(s) = state.read(cx).current()
            && s.instance.is_some()
            && s.state.is_some()
        {
            let mut seen = HashSet::new();
            for c in s
                .commands
                .data()
                .into_iter()
                .flatten()
                .filter(|c| seen.insert(c.name.clone()))
            {
                let (group, icon) = match c.source.as_str() {
                    "skill" => (
                        3,
                        crate::foundation::tool_presentation::ToolKind::Skill.icon(),
                    ),
                    "prompt" => (4, IconName::FileText),
                    _ => (2, IconName::Puzzle),
                };
                let scope = c.source_info.get("scope").and_then(|v| v.as_str());
                let detail = match scope {
                    Some("user") => t(cx, "command-scope-user"),
                    Some("project") => t(cx, "command-scope-project"),
                    Some("temporary") => t(cx, "command-scope-temporary"),
                    _ => String::new(),
                };
                rows.push(Row {
                    id: Id::Pi(c.name.clone()),
                    group,
                    title: format!("/{}", c.name),
                    description: c.description.clone().unwrap_or_default(),
                    detail,
                    tooltip: [
                        c.description.as_deref(),
                        c.source_info.get("path").and_then(|v| v.as_str()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n"),
                    icon,
                    enabled: self.can_send(cx),
                });
            }
        }
        rows.retain(|row| row.matches(&query));
        rows.sort_by_key(|row| row.group);
        rows
    }
    fn selected(&self, cx: &App) -> Option<Id> {
        let path = self.list.read(cx).selected_index()?;
        self.rows
            .iter()
            .filter(|row| row.group == path.section)
            .nth(path.row)
            .map(|row| row.id.clone())
    }
    fn choose(
        &mut self,
        id: Option<Id>,
        execute: bool,
        mode: StreamingBehavior,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let rows = self.candidates(cx);
        if let Some(id) = id {
            let Some(row) = rows.iter().find(|r| r.id == id && r.enabled) else {
                return;
            };
            match &row.id {
                Id::Local(kind) => {
                    if !execute {
                        return;
                    }
                    let kind = *kind;
                    let home = self.home.as_ref().map(|(home, _)| home.clone());
                    self.close(window, cx);
                    window.defer(cx, move |window, cx| {
                        if let Some(home) = home {
                            let _ =
                                home.update(cx, |home, cx| home.run_action(&Run(kind), window, cx));
                        } else {
                            let action: Box<dyn Action> = match kind {
                                Kind::Settings => Box::new(menus::ShowSettings),
                                Kind::Quit => Box::new(menus::Quit),
                                _ => Box::new(menus::ShowMainWindow),
                            };
                            window.dispatch_action(action, cx);
                        }
                    });
                    return;
                }
                Id::Pi(name) => {
                    let text = self.input.read(cx).value().to_string();
                    let value = complete(&text, name);
                    let end = text.find(char::is_whitespace).unwrap_or(text.len());
                    let replacement = if end == text.len() {
                        format!("/{name} ")
                    } else {
                        format!("/{name}")
                    };
                    self.input.update(cx, |input, cx| {
                        input.set_selected_range(0..end, cx);
                        input.replace(replacement, window, cx);
                    });
                    self.completed = Some((value, self.input.read(cx).cursor()));
                    self.input.update(cx, |input, cx| input.focus(window, cx));
                }
            }
        } else if !rows.is_empty() {
            return;
        }
        if execute && self.can_send(cx) {
            let value = self.input.read(cx).value().to_string();
            if value.trim().is_empty() {
                return;
            }
            if let (Some((_, state)), Some((key, _))) = (&self.home, &self.target)
                && let Some(result) =
                    state.update(cx, |state, cx| state.send_text(key, value, mode, cx))
            {
                self.submission = Some(cx.spawn_in(window, async move |panel, cx| {
                    let accepted = result.await.unwrap_or(false);
                    let _ = panel.update_in(cx, |panel, window, cx| {
                        panel.submission = None;
                        if accepted && panel.is_open {
                            panel.close(window, cx);
                        }
                        cx.notify();
                    });
                }));
            }
        }
        cx.notify();
    }
    fn key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return false;
        }
        if key == "escape" {
            self.close(window, cx);
            return true;
        }
        if matches!(key, "up" | "down") {
            if self.rows.iter().all(|r| !r.enabled) {
                return false;
            }
            let action: Box<dyn Action> = if key == "up" {
                Box::new(gpui_kit::base::actions::SelectUp)
            } else {
                Box::new(gpui_kit::base::actions::SelectDown)
            };
            // Target Command's frame: dispatching through the textarea would
            // interpret SelectUp/Down as extending the text selection.
            let focus = self.list.read(cx).focus_handle(cx);
            window.defer(cx, move |window, cx| {
                focus.dispatch_action(action.as_ref(), window, cx)
            });
            return true;
        }
        let selected = self.selected(cx);
        if key == "tab" && matches!(selected, Some(Id::Local(_))) {
            return false;
        }
        self.choose(
            selected,
            key != "tab",
            if key == "followup" {
                StreamingBehavior::FollowUp
            } else {
                StreamingBehavior::Steer
            },
            window,
            cx,
        );
        true
    }
}
impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rows = self.candidates(cx);
        let mut content = v_flex()
            .min_w_0()
            .key_context("GupiPalette")
            .capture_action(cx.listener(|this, a: &input::Enter, w, cx| {
                if a.shift || !this.key(if a.secondary { "followup" } else { "enter" }, w, cx) {
                    cx.propagate();
                } else {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &input::IndentInline, w, cx| {
                if !this.key("tab", w, cx) {
                    cx.propagate();
                } else {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &input::MoveUp, w, cx| {
                if !this.key("up", w, cx) {
                    cx.propagate();
                } else {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &input::MoveDown, w, cx| {
                if !this.key("down", w, cx) {
                    cx.propagate();
                } else {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &input::Escape, w, cx| {
                if !this.key("escape", w, cx) {
                    cx.propagate();
                } else {
                    cx.stop_propagation();
                }
            }))
            .on_action(
                cx.listener(|this, _: &menus::ShowCommandPalette, w, cx| this.focus_input(w, cx)),
            )
            .on_action(cx.listener(|this, action: &Run, w, cx| {
                if action.0 == Kind::Palette {
                    this.focus_input(w, cx);
                } else if action.0 == Kind::QuickOpen {
                    let home = this.home.as_ref().map(|(home, _)| home.clone());
                    this.close(w, cx);
                    if let Some(home) = home {
                        w.defer(cx, move |w, cx| {
                            let _ = home.update(cx, |home, cx| {
                                home.run_action(&Run(Kind::QuickOpen), w, cx)
                            });
                        });
                    }
                }
            }));
        let busy = self.submission.is_some();
        let mut status = v_flex().gap_2().px_3().py_2();
        let mut has_status = false;
        if let Some((_, state)) = &self.home
            && let Some(s) = state.read(cx).current()
        {
            if s.instance.is_none() || s.state.is_none() {
                let message =
                    if s.core_read.running() || s.command.reconnecting() || s.instance.is_some() {
                        "command-loading"
                    } else {
                        "command-connection-unavailable"
                    };
                has_status = true;
                status = status.child(div().text_sm().child(t(cx, message)));
            } else if s.commands.running() {
                has_status = true;
                status = status.child(div().text_sm().child(t(cx, "command-loading")));
            } else if s
                .commands
                .data()
                .is_some_and(|commands| commands.is_empty())
            {
                has_status = true;
                status = status.child(div().text_sm().child(t(cx, "command-empty")));
            }
            if let Some(error) = s.commands.error() {
                has_status = true;
                status = status.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.to_owned()),
                );
                status = status.child(
                    Button::new("retry-commands")
                        .small()
                        .label(t(cx, "action-retry"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let (Some((_, state)), Some((key, _))) = (&this.home, &this.target) {
                                state.update(cx, |s, cx| s.read_commands(key, cx));
                            }
                        })),
                );
            }
        }
        let input = self.input.clone();
        let mut command = Command::new(&self.list)
            .searchable(false)
            .bordered(false)
            .max_h(px(320.))
            .header(move |_, _, cx| {
                div().border_b_1().border_color(cx.theme().border).child(
                    Textarea::new(&input)
                        .appearance(false)
                        .readonly(busy)
                        .aria_label(t(cx, "command-search-placeholder")),
                )
            });
        let mut paths = Vec::new();
        for group in 0..5 {
            let mut section = CommandGroup::new().label(t(cx, group_label(group)));
            for (index, row) in self.rows.iter().filter(|r| r.group == group).enumerate() {
                let path = IndexPath::new(index).section(group);
                paths.push((path, row.id.clone()));
                let row = row.clone();
                section = section.item(
                    CommandItem::new()
                        .label(row.title.clone())
                        .disabled(!row.enabled)
                        .child(move |window, cx| {
                            let mut el = h_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(row.icon).size_4())
                                .child(div().flex_shrink_0().child(row.title.clone()))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(row.description.clone()),
                                );
                            if !row.enabled {
                                el = el.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(t(cx, "command-unavailable")),
                                );
                            }
                            match row.id {
                                Id::Local(kind) => {
                                    if let Some(kbd) = binding(kind, window) {
                                        el = el.child(kbd);
                                    }
                                }
                                Id::Pi(_) => {
                                    el = el.child(
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(row.detail.clone()),
                                    );
                                }
                            }
                            let tooltip = row.tooltip.clone();
                            el.id(SharedString::from(format!(
                                "command-row-{}-{}",
                                row.group, row.title
                            )))
                            .when(!tooltip.is_empty(), |el| {
                                el.tooltip(move |window, cx| {
                                    Tooltip::new(tooltip.clone()).build(window, cx)
                                })
                            })
                        }),
                );
            }
            command = command.group(section);
        }
        let owner = cx.weak_entity();
        let observer = cx.weak_entity();
        command = command
            .on_select(move |_, _, cx| {
                let _ = observer.update(cx, |_, cx| cx.notify());
            })
            .on_confirm(move |path, w, cx| {
                if let Some((_, id)) = paths.iter().find(|(p, _)| *p == path) {
                    let id = id.clone();
                    let execute = matches!(id, Id::Local(_));
                    let _ = owner.update(cx, |this, cx| {
                        this.choose(Some(id), execute, StreamingBehavior::Steer, w, cx)
                    });
                }
            })
            .empty(|_, _, cx| {
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "command-no-matches"))
            });
        content = content
            .child(command)
            .when(has_status, |content| content.child(status));
        let selected = self.selected(cx);
        let execute_label = if selected.is_none() && self.can_send(cx) {
            "command-send-text"
        } else {
            "command-execute"
        };
        let input_focus = self.input.read(cx).focus_handle(cx);
        let enabled = selected
            .as_ref()
            .and_then(|id| self.rows.iter().find(|r| &r.id == id))
            .map(|r| r.enabled)
            .unwrap_or_else(|| {
                self.rows.is_empty()
                    && self.can_send(cx)
                    && !self.input.read(cx).value().trim().is_empty()
            });
        content.child(
            h_flex()
                .px_3()
                .py_1()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .justify_end()
                .items_center()
                .child(
                    Button::new("command-dismiss")
                        .small()
                        .ghost()
                        .label(t(cx, "command-dismiss"))
                        .when_some(
                            Kbd::binding_for_action_in(&input::Escape, &input_focus, window),
                            |button, key| button.child(key),
                        )
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.key("escape", w, cx);
                        })),
                )
                .when(matches!(selected, Some(Id::Pi(_))), |footer| {
                    footer.child(
                        Button::new("command-complete")
                            .small()
                            .ghost()
                            .label(t(cx, "command-complete"))
                            .when_some(
                                Kbd::binding_for_action_in(
                                    &input::IndentInline,
                                    &input_focus,
                                    window,
                                ),
                                |button, key| button.child(key),
                            )
                            .disabled(busy || !enabled)
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.key("tab", w, cx);
                            })),
                    )
                })
                .child(
                    Button::new("command-send")
                        .small()
                        .ghost()
                        .label(t(cx, execute_label))
                        .when_some(
                            Kbd::binding_for_action_in(
                                &input::Enter {
                                    secondary: false,
                                    shift: false,
                                },
                                &input_focus,
                                window,
                            ),
                            |button, key| button.child(key),
                        )
                        .accessibility_label(t(cx, execute_label))
                        .loading(busy)
                        .disabled(!enabled)
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.choose(this.selected(cx), true, StreamingBehavior::Steer, w, cx)
                        })),
                ),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::{APP, Id, Kind, Row, SESSION, complete, local_icon, query};
    fn local_row(kind: Kind) -> Row {
        Row {
            id: Id::Local(kind),
            group: 0,
            title: "本地操作".into(),
            description: String::new(),
            detail: String::new(),
            tooltip: String::new(),
            icon: local_icon(kind),
            enabled: true,
        }
    }
    #[test]
    fn builtin_search_names_resolve_to_existing_local_actions() {
        let rows = APP
            .iter()
            .chain(SESSION)
            .copied()
            .map(local_row)
            .collect::<Vec<_>>();
        for (name, kind) in [
            ("settings", Kind::Settings),
            ("model", Kind::Model),
            ("thinking", Kind::Model),
            ("tree", Kind::OpenHistory),
            ("fork", Kind::OpenHistory),
            ("export", Kind::Export),
            ("copy", Kind::CopyLastAnswer),
            ("compact", Kind::Compact),
            ("name", Kind::Rename),
            ("clone", Kind::Clone),
            ("new", Kind::New),
            ("resume", Kind::QuickOpen),
            ("reload", Kind::Reconnect),
            ("quit", Kind::Quit),
        ] {
            for text in [name.to_owned(), format!("/{name}"), name.to_uppercase()] {
                let query = query(&text, text.len()).unwrap();
                assert_eq!(
                    rows.iter()
                        .filter(|row| row.id == Id::Local(kind) && row.matches(&query))
                        .count(),
                    1,
                    "{text}"
                );
            }
        }
        assert!(local_row(Kind::New).matches("本地"));
        for name in [
            "import",
            "share",
            "scoped-models",
            "changelog",
            "trust",
            "login",
            "logout",
            "hotkeys",
        ] {
            assert!(!rows.iter().any(|row| row.matches(name)), "{name}");
        }
    }
    #[test]
    fn search_aliases_do_not_replace_same_named_extension_candidates() {
        let local = local_row(Kind::Model);
        let extension = Row {
            id: Id::Pi("model".into()),
            title: "/model".into(),
            ..local.clone()
        };
        let rows = [local, extension];
        assert_eq!(rows.iter().filter(|row| row.matches("model")).count(), 2);
        assert!(query("/model provider/name", 20).is_none());
    }
    #[test]
    fn completion_preserves_parameters_and_queries_do_not_block_raw_input() {
        assert_eq!(complete("/rev 参数\n正文", "review"), "/review 参数\n正文");
        assert_eq!(complete("/技", "skill:写作"), "/skill:写作 ");
        assert_eq!(query("/review 参数", 9), None);
        assert_eq!(query("/review 参数", 4), Some("review".into()));
        assert_eq!(query("anything", 8), Some("anything".into()));
    }
}
