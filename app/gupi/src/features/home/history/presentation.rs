use super::*;
use crate::state::history::HistoryKind;
use gpui_kit::component::Selectable;
use gpui_kit::prelude::FluentBuilder;
use time::{OffsetDateTime, UtcOffset};

/// Keep List's selection behavior while using a background-only history row.
#[derive(IntoElement)]
pub(crate) struct HistoryListItem {
    base: Stateful<Div>,
    selected: bool,
    secondary_selected: bool,
}

impl HistoryListItem {
    pub(super) fn new(id: String, content: impl IntoElement) -> Self {
        Self {
            base: div()
                .id(id)
                .w_full()
                .h(px(graph::ROW_HEIGHT))
                .child(content),
            selected: false,
            secondary_selected: false,
        }
    }
}

impl Selectable for HistoryListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn secondary_selected(mut self, selected: bool) -> Self {
        self.secondary_selected = selected;
        self
    }
}

impl RenderOnce for HistoryListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.base
            .when(self.selected, |row| {
                row.bg(if cx.theme().list.active_highlight {
                    cx.theme().list_active
                } else {
                    cx.theme().accent
                })
            })
            .when(!self.selected && self.secondary_selected, |row| {
                row.bg(cx.theme().tokens.list_hover)
            })
            .when(!self.selected && !self.secondary_selected, |row| {
                row.hover(|row| row.bg(cx.theme().tokens.list_hover))
            })
    }
}

pub(super) fn role(row: &HistoryRow, cx: &App) -> (IconName, Hsla, String) {
    let (icon, color, label) = match row.kind {
        HistoryKind::User => (
            IconName::UserRound,
            cx.theme().blue,
            "conversation-role-user",
        ),
        HistoryKind::Assistant => (
            IconName::Bot,
            cx.theme().green,
            "conversation-history-reply",
        ),
        HistoryKind::AssistantProgress => (
            IconName::MessageCircle,
            cx.theme().muted_foreground,
            "conversation-history-progress",
        ),
        HistoryKind::Thinking => (
            IconName::Brain,
            cx.theme().muted_foreground,
            "conversation-history-thinking",
        ),
        HistoryKind::ToolCall => (
            IconName::Wrench,
            cx.theme().muted_foreground,
            "conversation-history-calls",
        ),
        HistoryKind::ToolResult => (
            tool_icon(row.tool.as_deref()),
            cx.theme().muted_foreground,
            "conversation-role-tool",
        ),
        HistoryKind::Failed => (
            IconName::CircleAlert,
            cx.theme().danger,
            "conversation-history-failed",
        ),
        HistoryKind::Stopped => (
            IconName::Square,
            cx.theme().warning,
            "conversation-history-stopped",
        ),
        HistoryKind::EmptyAssistant => (
            IconName::Bot,
            cx.theme().muted_foreground,
            "conversation-history-empty",
        ),
        HistoryKind::Compaction => (
            IconName::Database,
            cx.theme().muted_foreground,
            "conversation-compaction",
        ),
        HistoryKind::BranchSummary => (
            IconName::GitBranch,
            cx.theme().muted_foreground,
            "conversation-branch-summary",
        ),
        HistoryKind::Event => (
            IconName::Ellipsis,
            cx.theme().muted_foreground,
            "conversation-event",
        ),
    };
    (icon, color, t(cx, label))
}

fn tool_icon(name: Option<&str>) -> IconName {
    match name {
        Some("read") => IconName::BookOpen,
        Some("bash") => IconName::Terminal,
        Some("edit") => IconName::FilePenLine,
        Some("write") => IconName::FilePlus,
        Some("grep" | "find") => IconName::Search,
        Some("ls") => IconName::Folder,
        _ => IconName::Wrench,
    }
}

pub(super) fn is_process(kind: HistoryKind) -> bool {
    matches!(
        kind,
        HistoryKind::AssistantProgress
            | HistoryKind::Thinking
            | HistoryKind::ToolCall
            | HistoryKind::ToolResult
            | HistoryKind::EmptyAssistant
            | HistoryKind::Event
    )
}

pub(super) fn tooltip(row: &HistoryRow, preview: bool, cx: &App) -> String {
    let mut lines = vec![role(row, cx).2];
    if let Ok(date) = OffsetDateTime::parse(
        &row.timestamp,
        &time::format_description::well_known::Rfc3339,
    ) {
        let offset = UtcOffset::local_offset_at(date).unwrap_or(UtcOffset::UTC);
        let date = date.to_offset(offset);
        lines.push(format!(
            "{} {:02}:{:02}:{:02}",
            date.date(),
            date.hour(),
            date.minute(),
            date.second()
        ));
    }
    if row.current {
        lines.push(t(cx, "conversation-graph-current"));
    }
    if preview {
        lines.push(t(cx, "conversation-graph-preview"));
    }
    lines.join("\n")
}
