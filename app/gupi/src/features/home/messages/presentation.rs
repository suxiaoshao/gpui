use super::activity::Tool;
use super::viewport::ActivityViewport;
use super::*;
use crate::foundation::i18n::t_with_args;
use crate::foundation::tool_presentation::ToolKind;
use fluent_bundle::FluentArgs;
use gpui_kit::component::{
    Icon,
    collapsible::Collapsible,
    marker::{Marker, MarkerContent, MarkerIcon, MarkerLoadingStyle, MarkerVariant},
};
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone, Copy, PartialEq)]
enum Level {
    Run,
    Group,
    Tool,
}
pub(super) struct Disclosure {
    title: String,
    clock: Option<Entity<super::super::progress::ProcessClock>>,
    icon: Option<IconName>,
    level: Level,
    loading: bool,
    failed: bool,
    locked: bool,
}
impl Disclosure {
    pub fn run(title: String) -> Self {
        Self {
            title,
            clock: None,
            icon: None,
            level: Level::Run,
            loading: false,
            failed: false,
            locked: false,
        }
    }
    pub fn clock(mut self, clock: Option<Entity<super::super::progress::ProcessClock>>) -> Self {
        self.clock = clock;
        self
    }
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

impl HomeView {
    pub(super) fn fold(
        &self,
        location: (&str, &str),
        id: &str,
        heading: Disclosure,
        default: bool,
        content: AnyElement,
        cx: &App,
    ) -> AnyElement {
        let (key, row) = location;
        let open = heading.locked
            || self
                .views
                .get(key)
                .and_then(|v| v.process_open.get(id))
                .copied()
                .unwrap_or(default);
        let owner = self.history_list.read(cx).delegate().owner.clone();
        let key = key.to_owned();
        let id = id.to_owned();
        let target = id.clone();
        let row = row.to_owned();
        let action = Rc::new(move |_: &mut Window, cx: &mut App| {
            let _ = owner.update(cx, |this, cx| {
                this.toggle_process(&key, &row, &target, open, cx)
            });
        });
        let click = action.clone();
        let keyboard = action.clone();
        let group = SharedString::from(format!("disclosure-{id}"));
        let tool = heading.level == Level::Tool;
        let arrow = div()
            .flex_none()
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size_3(),
            )
            .when(tool, |arrow| {
                arrow
                    .opacity(0.)
                    .group_hover(group.clone(), |s| s.opacity(1.))
            });
        let marker = Marker::new()
            .with_variant(if heading.level == Level::Run {
                MarkerVariant::Border
            } else {
                MarkerVariant::Plain
            })
            .loading(heading.loading && heading.level != Level::Run)
            .with_loading_style(MarkerLoadingStyle::Shimmer)
            .min_w_0()
            .gap_1()
            .when(heading.failed, |m| m.text_color(cx.theme().danger))
            .when_some(heading.icon, |m, icon| {
                m.icon(MarkerIcon::new().child(Icon::new(icon).size_4()))
            })
            .content(
                MarkerContent::new()
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .map(|content| {
                        if let Some(clock) = heading.clock {
                            content.child(clock)
                        } else {
                            content.text(heading.title.clone())
                        }
                    }),
            )
            .when(!heading.locked, |m| m.child(arrow));
        let trigger = div()
            .id(id.clone())
            .group(group)
            .w_full()
            .min_w_0()
            .when(!heading.locked, |trigger| {
                trigger
                    .role(Role::Button)
                    .aria_label(heading.title)
                    .aria_expanded(open)
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .focus_visible(|s| s.bg(cx.theme().muted))
                    .hover(|s| s.text_color(cx.theme().foreground))
                    .on_click(move |_, window, cx| click(window, cx))
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            keyboard(window, cx);
                        }
                    })
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        action(window, cx)
                    })
            })
            .child(marker);
        v_flex()
            .w_full()
            .min_w_0()
            .when(open, |s| s.gap_1())
            .child(trigger)
            .child(
                Collapsible::new()
                    .w_full()
                    .min_w_0()
                    .open(open)
                    .content(content),
            )
            .into_any_element()
    }

    pub(super) fn render_block(
        &self,
        key: &str,
        row: &str,
        block: ActivityBlock<'_>,
        current: bool,
        cx: &App,
    ) -> AnyElement {
        match block {
            ActivityBlock::Message(Activity::Text { id, text, .. }) => Message::new()
                .content(
                    MessageContent::new().child(
                        self.text_view(key, row, id.clone(), text.clone())
                            .stream_fade(),
                    ),
                )
                .into_any_element(),
            ActivityBlock::Message(_) => unreachable!("Only assistant prose separates groups"),
            ActivityBlock::Group { id, items } => {
                if let [item] = items {
                    return self.render_activity(key, row, item, cx);
                }
                let tools: Vec<_> = items
                    .iter()
                    .filter_map(|a| {
                        if let Activity::Tool(tool) = a {
                            Some(tool)
                        } else {
                            None
                        }
                    })
                    .collect();
                let running_tool = current
                    .then(|| {
                        tools
                            .iter()
                            .rev()
                            .find(|tool| tool.status == ToolStatus::Running)
                    })
                    .flatten();
                let thinking = items.iter().any(|item| {
                    matches!(
                        item,
                        Activity::Text {
                            thinking: true,
                            running: true,
                            ..
                        }
                    )
                });
                let title = if let Some(tool) = running_tool {
                    tool_title(tool, cx)
                } else if thinking {
                    t(cx, "conversation-thinking-running")
                } else if tools.is_empty() {
                    t(cx, "conversation-thinking-content")
                } else {
                    group_summary(&tools, cx)
                };
                let heading = Disclosure {
                    title,
                    clock: None,
                    icon: None,
                    level: Level::Group,
                    loading: running_tool.is_some() || thinking,
                    failed: tools.iter().any(|tool| tool.status == ToolStatus::Failed),
                    locked: false,
                };
                let content = v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_1()
                    .children(
                        items
                            .iter()
                            .map(|item| self.render_activity(key, row, item, cx)),
                    )
                    .into_any_element();
                self.fold(
                    (key, row),
                    &id,
                    heading,
                    false,
                    ActivityViewport::new(format!("{key}-{id}"), content).into_any_element(),
                    cx,
                )
            }
        }
    }

    fn render_activity(&self, key: &str, row: &str, item: &Activity, cx: &App) -> AnyElement {
        match item {
            Activity::Tool(tool) => self.render_tool(key, tool, cx),
            Activity::Text {
                id, text, running, ..
            } => self.fold(
                (key, row),
                id,
                Disclosure {
                    clock: None,
                    title: t(
                        cx,
                        if *running {
                            "conversation-thinking-running"
                        } else {
                            "conversation-thinking-content"
                        },
                    ),
                    icon: Some(IconName::Brain),
                    level: Level::Tool,
                    loading: *running,
                    failed: false,
                    locked: text.is_empty(),
                },
                false,
                div()
                    .pl_6()
                    .child(
                        self.text_view(key, row, id.clone(), text.clone())
                            .stream_fade(),
                    )
                    .into_any_element(),
                cx,
            ),
        }
    }

    fn render_tool(&self, key: &str, tool: &Tool, cx: &App) -> AnyElement {
        self.tool_trigger(key, tool, cx).into_any_element()
    }
}

pub(super) fn tool_action(tool: &Tool, cx: &App) -> String {
    let mut args = FluentArgs::new();
    args.set(
        "state",
        match tool.status {
            ToolStatus::Running => "running",
            ToolStatus::Complete => "complete",
            ToolStatus::Failed => "failed",
            ToolStatus::Unfinished => "unfinished",
        },
    );
    args.set("name", tool.name.clone());
    t_with_args(cx, tool.kind().action_key(), &args)
}

pub(super) fn tool_title(tool: &Tool, cx: &App) -> String {
    let mut args = FluentArgs::new();
    args.set("action", tool_action(tool, cx));
    args.set(
        "summary",
        tool.summary().unwrap_or_default().replace('\n', " "),
    );
    if tool.kind() == ToolKind::Shell {
        args.set(
            "shell",
            if tool.name == "powershell" {
                "PowerShell"
            } else {
                "Bash"
            },
        );
        t_with_args(cx, "conversation-shell-line", &args)
            .trim()
            .to_owned()
    } else {
        t_with_args(cx, "conversation-tool-line", &args)
            .trim()
            .to_owned()
    }
}

fn group_summary(tools: &[&Tool], cx: &App) -> String {
    let categories = [
        (ToolKind::Skill, "conversation-tool-group-skill"),
        (ToolKind::Read, "conversation-tool-group-read"),
        (ToolKind::Search, "conversation-tool-group-search"),
        (ToolKind::Shell, "conversation-tool-group-bash"),
        (ToolKind::Edit, "conversation-tool-group-edit"),
        (ToolKind::Other, "conversation-tool-group"),
    ];
    let category = |tool: &&Tool| match tool.kind() {
        ToolKind::List => ToolKind::Search,
        ToolKind::Write => ToolKind::Edit,
        kind => kind,
    };
    let mut parts = vec![];
    for (kind, key) in categories {
        let count = tools.iter().filter(|tool| category(tool) == kind).count();
        if count == 0 {
            continue;
        }
        let mut args = FluentArgs::new();
        args.set("count", count);
        parts.push(t_with_args(cx, key, &args));
    }
    // Independent category labels, not fragments of a translated sentence.
    parts.join(" · ")
}
pub(super) fn fenced(language: &str, text: &str) -> String {
    // Tool output may contain Markdown fences itself.
    let fence = "`".repeat(
        text.split(|c| c != '`')
            .map(str::len)
            .max()
            .unwrap_or(0)
            .max(2)
            + 1,
    );
    format!("{fence}{language}\n{text}\n{fence}")
}
