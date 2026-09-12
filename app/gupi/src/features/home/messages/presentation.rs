use super::activity::Tool;
use super::viewport::ActivityViewport;
use super::*;
use crate::foundation::i18n::t_with_args;
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
    icon: Option<IconName>,
    level: Level,
    loading: bool,
    failed: bool,
}
impl Disclosure {
    pub fn compaction(title: String) -> Self {
        Self {
            title,
            icon: Some(IconName::FileText),
            level: Level::Tool,
            loading: false,
            failed: false,
        }
    }
    pub fn run(title: String) -> Self {
        Self {
            title,
            icon: None,
            level: Level::Run,
            loading: false,
            failed: false,
        }
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

impl HomeView {
    pub(super) fn fold(
        &self,
        key: &str,
        id: &str,
        heading: Disclosure,
        default: bool,
        content: AnyElement,
        cx: &App,
    ) -> AnyElement {
        let open = self
            .views
            .get(key)
            .and_then(|v| v.process_open.get(id))
            .copied()
            .unwrap_or(default);
        let owner = self.history_list.read(cx).delegate().owner.clone();
        let key = key.to_owned();
        let id = id.to_owned();
        let target = id.clone();
        let action = Rc::new(move |_: &mut Window, cx: &mut App| {
            let _ = owner.update(cx, |this, cx| this.toggle_process(&key, &target, open, cx));
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
            .loading(heading.loading)
            .with_loading_style(MarkerLoadingStyle::Shimmer)
            .min_w_0()
            .min_h_6()
            .gap_2()
            .when(heading.failed, |m| m.text_color(cx.theme().danger))
            .when_some(heading.icon, |m, icon| {
                m.icon(MarkerIcon::new().child(Icon::new(icon).size_4()))
            })
            .content(
                MarkerContent::new()
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .when(tool, |c| c.flex_1())
                    .text(heading.title.clone()),
            )
            .child(arrow);
        let trigger = div()
            .id(id.clone())
            .group(group)
            .w_full()
            .min_w_0()
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

    pub(super) fn render_block(&self, key: &str, block: ActivityBlock<'_>, cx: &App) -> AnyElement {
        match block {
            ActivityBlock::Message(Activity::Text { id, text, .. }) => Message::new()
                .content(MessageContent::new().child(text_view(id.clone(), text.clone())))
                .into_any_element(),
            ActivityBlock::Message(_) => unreachable!("Only assistant prose separates groups"),
            ActivityBlock::Group { id, items } => {
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
                let running = tools.iter().any(|tool| tool.status == ToolStatus::Running);
                let mut args = FluentArgs::new();
                args.set("count", tools.len());
                let kind = tools.first().map(|tool| tool_kind(&tool.name));
                let homogeneous = tools.iter().all(|tool| Some(tool_kind(&tool.name)) == kind);
                let exploration = tools
                    .iter()
                    .all(|tool| matches!(tool_kind(&tool.name), "read" | "search" | "bash"));
                let summary_key = if tools.is_empty() {
                    "conversation-thinking-content"
                } else if running {
                    "conversation-tool-group-running"
                } else if !homogeneous && exploration {
                    if tools.iter().any(|tool| tool.name == "bash") {
                        "conversation-tool-group-explore-commands"
                    } else {
                        "conversation-tool-group-explore"
                    }
                } else if homogeneous {
                    match kind.unwrap() {
                        "read" => "conversation-tool-group-read",
                        "bash" => "conversation-tool-group-bash",
                        "search" => "conversation-tool-group-search",
                        "write" | "edit" => "conversation-tool-group-edit",
                        _ => "conversation-tool-group",
                    }
                } else {
                    "conversation-tool-group"
                };
                let title = t_with_args(cx, summary_key, &args);
                let heading = Disclosure {
                    title,
                    icon: Some(if tools.is_empty() {
                        IconName::Brain
                    } else if homogeneous {
                        tool_icon(&tools[0].name)
                    } else if exploration {
                        IconName::Search
                    } else {
                        IconName::Wrench
                    }),
                    level: Level::Group,
                    loading: running,
                    failed: false,
                };
                let content = v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_1()
                    .children(items.iter().map(|item| {
                        match item {
                            Activity::Tool(tool) => self.render_tool(key, tool, cx),
                            Activity::Text { id, text, .. } => self.fold(
                                key,
                                id,
                                Disclosure {
                                    title: t(cx, "conversation-thinking-content"),
                                    icon: Some(IconName::Brain),
                                    level: Level::Tool,
                                    loading: false,
                                    failed: false,
                                },
                                false,
                                div()
                                    .pl_6()
                                    .child(text_view(id.clone(), text.clone()))
                                    .into_any_element(),
                                cx,
                            ),
                        }
                    }))
                    .into_any_element();
                self.fold(
                    key,
                    &id,
                    heading,
                    false,
                    ActivityViewport::new(format!("{key}-{id}"), content).into_any_element(),
                    cx,
                )
            }
        }
    }

    fn render_tool(&self, key: &str, tool: &Tool, cx: &App) -> AnyElement {
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
        let action = t_with_args(
            cx,
            &format!("conversation-tool-action-{}", tool_kind(&tool.name)),
            &args,
        );
        args.set("action", action);
        args.set(
            "summary",
            tool.summary().unwrap_or_default().replace('\n', " "),
        );
        let title = t_with_args(cx, "conversation-tool-line", &args);
        let heading = Disclosure {
            title,
            icon: Some(tool_icon(&tool.name)),
            level: Level::Tool,
            loading: tool.status == ToolStatus::Running,
            failed: tool.status == ToolStatus::Failed,
        };
        let mut details = v_flex().w_full().min_w_0().gap_2().pl_6().py_1();
        if let Some(args) = &tool.args {
            details = details.child(text_view(
                format!("args-{}", tool.id),
                fenced(
                    "json",
                    &serde_json::to_string_pretty(args).unwrap_or_default(),
                ),
            ));
        }
        if !tool.output.is_empty() {
            details = details.child(text_view(
                format!("output-{}", tool.id),
                fenced("text", &tool.output),
            ));
        }
        self.fold(
            key,
            &tool.id,
            heading,
            false,
            details.into_any_element(),
            cx,
        )
    }
}

fn tool_kind(name: &str) -> &'static str {
    match name {
        "read" => "read",
        "write" => "write",
        "edit" => "edit",
        "bash" => "bash",
        "grep" | "find" | "ls" => "search",
        _ => "other",
    }
}
fn tool_icon(name: &str) -> IconName {
    match tool_kind(name) {
        "read" => IconName::BookOpen,
        "write" => IconName::FilePlus,
        "edit" => IconName::FilePenLine,
        "bash" => IconName::Terminal,
        "search" => IconName::Search,
        _ => IconName::Wrench,
    }
}
fn fenced(language: &str, text: &str) -> String {
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
