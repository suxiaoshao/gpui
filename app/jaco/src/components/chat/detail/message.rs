use std::{collections::HashMap, rc::Rc};

use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, h_flex,
    label::Label,
    text::{TextView, TextViewState},
    v_flex,
};
use jaco_core::{
    AgentRun, AgentRunId, AgentRunStatus, ConversationEntry, ConversationEntryId, ToolInvocationId,
};

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

use super::attachments::{UserImageAttachment, render_user_image_attachments};
use super::copy_button::{CopyButton, OnCopy};
use super::tool_invocation::{AgentDetailItem, OnToggleToolInvocation};

pub(super) type OnToggleAgent = Rc<dyn Fn(AgentRunId, &mut Window, &mut App) + 'static>;
pub(super) type OnApprovalDecision =
    Rc<dyn Fn(ToolInvocationId, bool, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub(super) enum TimelineRow {
    User(Box<UserMessageRow>),
    Agent(Box<AgentTurnRow>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TimelineRowKey {
    User(String),
    Agent(String),
}

impl TimelineRow {
    pub(super) fn key(&self) -> TimelineRowKey {
        match self {
            TimelineRow::User(row) => TimelineRowKey::User(row.item.id.clone()),
            TimelineRow::Agent(row) => TimelineRowKey::Agent(
                row.run_id
                    .clone()
                    .or_else(|| row.items.first().map(AgentDetailItem::stable_id_suffix))
                    .unwrap_or_else(|| "agent".to_string()),
            ),
        }
    }

    pub(super) fn contains_item(&self, item_id: &ConversationEntryId) -> bool {
        match self {
            TimelineRow::User(row) => &row.item.id == item_id,
            TimelineRow::Agent(row) => row.items.iter().any(|item| item.contains_entry_id(item_id)),
        }
    }
}

impl RenderOnce for TimelineRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            TimelineRow::User(row) => gpui::RenderOnce::render(*row, window, cx).into_any_element(),
            TimelineRow::Agent(row) => {
                gpui::RenderOnce::render(*row, window, cx).into_any_element()
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct UserMessageRow {
    pub(super) item: ConversationEntry,
    pub(super) image_attachments: Vec<UserImageAttachment>,
    pub(super) text_state: Option<Entity<TextViewState>>,
    pub(super) on_copy: OnCopy,
}

impl RenderOnce for UserMessageRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = format!("conversation-user-message-{}", self.item.id);
        let markdown = format::item_markdown(&self.item);
        let has_markdown = !markdown.trim().is_empty();
        let has_image_attachments = !self.image_attachments.is_empty();
        let copy_text = markdown.clone();
        let on_copy = self.on_copy;
        let i18n = cx.global::<I18n>();
        let copy_tooltip = i18n.t("conversation-copy-tooltip");
        let copied_tooltip = i18n.t("conversation-copy-success");
        let sent_time = timestamp_arg_label(
            i18n,
            "conversation-user-sent-time",
            format::timestamp_label(self.item.created_at, i18n),
        );
        let copy_button = CopyButton::new(
            format!("conversation-copy-user-{}", self.item.id),
            copy_text,
            on_copy,
            copy_tooltip,
            copied_tooltip,
            window,
            cx,
        );

        h_flex()
            .id(format!("conversation-user-row-{}", self.item.id))
            .group(group.clone())
            .w_full()
            .justify_end()
            .px_6()
            .py_3()
            .child(
                v_flex()
                    .items_end()
                    .max_w(px(680.))
                    .min_w_0()
                    .gap_2()
                    .when(has_image_attachments, |this| {
                        this.child(render_user_image_attachments(
                            &self.item.id,
                            self.image_attachments,
                            cx,
                        ))
                    })
                    .when(has_markdown || !has_image_attachments, |this| {
                        this.child(
                            div()
                                .rounded(px(8.))
                                .px_3()
                                .py_2()
                                .bg(cx.theme().tokens.primary.background.opacity(0.12))
                                .border_1()
                                .border_color(cx.theme().primary.opacity(0.18))
                                .text_color(cx.theme().foreground)
                                .child(markdown_view(
                                    format!("conversation-user-message-markdown-{}", self.item.id),
                                    self.text_state,
                                    &markdown,
                                )),
                        )
                    })
                    .child(
                        h_flex()
                            .h(px(24.))
                            .items_center()
                            .justify_end()
                            .gap_1()
                            .opacity(0.)
                            .group_hover(group.clone(), |this| this.opacity(1.))
                            .child(
                                Label::new(sent_time)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(copy_button),
                    ),
            )
    }
}

#[derive(Clone)]
pub(super) struct AgentTurnRow {
    pub(super) run_id: Option<AgentRunId>,
    pub(super) run: Option<AgentRun>,
    pub(super) items: Vec<AgentDetailItem>,
    pub(super) text_states: HashMap<ConversationEntryId, Entity<TextViewState>>,
    pub(super) expanded: bool,
    pub(super) on_toggle: OnToggleAgent,
    pub(super) on_toggle_tool_invocation: OnToggleToolInvocation,
    pub(super) on_copy: OnCopy,
    pub(super) on_approval_decision: OnApprovalDecision,
}

impl RenderOnce for AgentTurnRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id_suffix = self
            .run_id
            .clone()
            .or_else(|| self.items.first().map(AgentDetailItem::stable_id_suffix))
            .unwrap_or_else(|| "agent".to_string());
        let group = format!("conversation-agent-turn-{id_suffix}");
        let copy_text = {
            let i18n = cx.global::<I18n>();
            agent_copy_text(&self, i18n)
        };
        let on_copy = self.on_copy.clone();
        let (copy_tooltip, copied_tooltip, hover_time, final_markdown) = {
            let i18n = cx.global::<I18n>();
            let hover_time = self
                .run
                .as_ref()
                .map(|run| agent_hover_time(run, i18n))
                .unwrap_or_else(|| {
                    self.items
                        .iter()
                        .find_map(|item| match item {
                            AgentDetailItem::Entry(entry) => Some(entry.created_at),
                            AgentDetailItem::ToolInvocation(detail) => Some(detail.created_at),
                            AgentDetailItem::UnresolvedToolLifecycle(_) => None,
                        })
                        .map(|created_at| format::timestamp_label(created_at, i18n))
                        .unwrap_or_default()
                });
            (
                i18n.t("conversation-copy-tooltip"),
                i18n.t("conversation-copy-success"),
                hover_time,
                agent_final_markdown(self.final_item(), i18n),
            )
        };
        let status_row = self.render_status_row(&id_suffix, cx);
        let separator = self.render_separator(&id_suffix, cx);
        let action_row = agent_action_row(
            AgentActionRow {
                id_suffix: id_suffix.clone(),
                copy_text,
                on_copy,
                copy_tooltip,
                copied_tooltip,
                hover_time,
            },
            window,
            cx,
        );
        let final_text_state = self
            .final_item()
            .and_then(|item| self.text_states.get(&item.id).cloned());

        v_flex()
            .id(format!("conversation-agent-row-{id_suffix}"))
            .group(group.clone())
            .relative()
            .w_full()
            .min_w_0()
            .px_6()
            .py_3()
            .gap_2()
            .child(status_row)
            .child(separator)
            .when(self.expanded, |this| {
                this.child(self.render_details(window, cx))
            })
            .when(!final_markdown.is_empty(), |this| {
                this.child(
                    div()
                        .max_w(px(760.))
                        .min_w_0()
                        .text_color(cx.theme().foreground)
                        .child(markdown_view(
                            format!("conversation-agent-final-markdown-{id_suffix}"),
                            final_text_state,
                            &final_markdown,
                        )),
                )
            })
            .child(action_row)
    }
}

impl AgentTurnRow {
    pub(super) fn final_item(&self) -> Option<&ConversationEntry> {
        let final_entry_id = &self.run.as_ref()?.output.as_ref()?.final_entry_id;
        self.items
            .iter()
            .filter_map(AgentDetailItem::entry)
            .find(|item| &item.id == final_entry_id)
    }

    fn render_status_row(&self, id_suffix: &str, cx: &mut App) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let (label, icon) = if let Some(run) = &self.run {
            if format::is_terminal_run(run) {
                (
                    agent_terminal_status_label(run.status, run, i18n),
                    if self.expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                )
            } else {
                (
                    duration_arg_label(
                        i18n,
                        "conversation-agent-processing",
                        format::elapsed_since_label(format::run_started_time(run)),
                    ),
                    IconName::ChevronDown,
                )
            }
        } else {
            (i18n.t("conversation-agent-details"), IconName::ChevronDown)
        };

        let run_id = self.run_id.clone();
        let on_toggle = self.on_toggle.clone();
        h_flex()
            .id(format!("conversation-agent-status-{id_suffix}"))
            .w_full()
            .max_w(px(760.))
            .items_center()
            .gap_1()
            .text_color(cx.theme().muted_foreground)
            .when(run_id.is_some(), |this| this.cursor_pointer())
            .on_click(move |_, window, cx| {
                if let Some(run_id) = run_id.clone() {
                    on_toggle(run_id, window, cx);
                }
            })
            .child(Label::new(label).text_xs().whitespace_nowrap())
            .child(Icon::new(icon).size_3())
            .into_any_element()
    }

    fn render_separator(&self, id_suffix: &str, cx: &mut App) -> AnyElement {
        div()
            .id(format!("conversation-agent-separator-{id_suffix}"))
            .w_full()
            .max_w(px(760.))
            .h(px(1.))
            .bg(cx.theme().tokens.border.background.opacity(0.7))
            .into_any_element()
    }

    fn render_details(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let final_item_id = self.final_item().map(|item| &item.id);
        let detail_items = self.items.iter().filter(|item| {
            item.entry()
                .is_none_or(|entry| final_item_id != Some(&entry.id))
        });
        let text_states = self.text_states.clone();
        let mut blocks = Vec::with_capacity(detail_items.size_hint().0);
        for item in detail_items.cloned() {
            match item {
                AgentDetailItem::Entry(item) => {
                    let text_state = text_states.get(&item.id).cloned();
                    blocks.push(
                        super::tool_blocks::DetailBlock::new(item, text_state, window, cx)
                            .into_any_element(),
                    );
                }
                AgentDetailItem::ToolInvocation(detail) => {
                    blocks.push(
                        super::tool_invocation::ToolInvocationBlock::new(
                            detail,
                            self.on_toggle_tool_invocation.clone(),
                            self.on_copy.clone(),
                            self.on_approval_decision.clone(),
                        )
                        .into_any_element(),
                    );
                }
                AgentDetailItem::UnresolvedToolLifecycle(unresolved) => {
                    blocks.push(
                        super::tool_invocation::UnresolvedToolLifecycleBlock::new(unresolved)
                            .into_any_element(),
                    );
                }
            }
        }

        v_flex()
            .max_w(px(760.))
            .min_w_0()
            .gap_2()
            .children(blocks)
            .into_any_element()
    }
}

fn markdown_view(
    id: impl Into<ElementId>,
    text_state: Option<Entity<TextViewState>>,
    fallback_markdown: &str,
) -> AnyElement {
    if let Some(text_state) = text_state {
        TextView::new(&text_state)
            .selectable(true)
            .into_any_element()
    } else {
        TextView::markdown(id, fallback_markdown)
            .selectable(true)
            .into_any_element()
    }
}

struct AgentActionRow {
    id_suffix: String,
    copy_text: String,
    on_copy: OnCopy,
    copy_tooltip: String,
    copied_tooltip: String,
    hover_time: String,
}

fn agent_action_row(row: AgentActionRow, window: &mut Window, cx: &mut App) -> AnyElement {
    let action_group = format!("conversation-agent-actions-{}", row.id_suffix);
    let copy_button = CopyButton::new(
        format!("conversation-copy-agent-{}", row.id_suffix),
        row.copy_text,
        row.on_copy,
        row.copy_tooltip,
        row.copied_tooltip,
        window,
        cx,
    );

    h_flex()
        .id(action_group.clone())
        .group(action_group.clone())
        .w_full()
        .max_w(px(760.))
        .h(px(24.))
        .items_center()
        .gap_1()
        .child(copy_button)
        .child(
            div()
                .opacity(0.)
                .group_hover(action_group, |this| this.opacity(1.))
                .child(
                    Label::new(row.hover_time)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .into_any_element()
}

fn agent_copy_text(row: &AgentTurnRow, i18n: &I18n) -> String {
    let final_markdown = agent_final_markdown(row.final_item(), i18n);
    if !row.expanded && !final_markdown.trim().is_empty() {
        return final_markdown;
    }

    let parts = row
        .items
        .iter()
        .filter_map(AgentDetailItem::entry)
        .filter(|entry| !super::tool_invocation::is_tool_lifecycle_entry(entry))
        .map(format::item_markdown)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>();
    parts.join("\n\n")
}

fn agent_final_markdown(final_item: Option<&ConversationEntry>, i18n: &I18n) -> String {
    let Some(final_item) = final_item else {
        return String::new();
    };
    match &final_item.payload {
        jaco_core::ConversationEntryPayload::Error(error) => {
            format!("**{}:** {}", i18n.t("conversation-error"), error.message)
        }
        jaco_core::ConversationEntryPayload::Status(status) => {
            i18n.t(format::status_i18n_key(status.code))
        }
        _ => format::item_markdown(final_item),
    }
}

fn agent_terminal_status_label(status: AgentRunStatus, run: &AgentRun, i18n: &I18n) -> String {
    let key = match status {
        AgentRunStatus::Completed => "conversation-agent-processed",
        AgentRunStatus::Failed => "conversation-agent-failed",
        AgentRunStatus::Canceled => "conversation-agent-canceled",
        AgentRunStatus::Running => "conversation-agent-processing",
    };
    duration_arg_label(i18n, key, format::run_duration_label(run))
}

fn agent_hover_time(run: &AgentRun, i18n: &I18n) -> String {
    if format::is_terminal_run(run) {
        timestamp_arg_label(
            i18n,
            "conversation-agent-completed-time",
            format::timestamp_label(format::run_completed_time(run), i18n),
        )
    } else {
        timestamp_arg_label(
            i18n,
            "conversation-agent-started-time",
            format::timestamp_label(format::run_started_time(run), i18n),
        )
    }
}

fn timestamp_arg_label(i18n: &I18n, key: &str, time: String) -> String {
    let mut args = FluentArgs::new();
    args.set("time", time);
    i18n.t_with_args(key, &args)
}

fn duration_arg_label(i18n: &I18n, key: &str, duration: String) -> String {
    let mut args = FluentArgs::new();
    args.set("duration", duration);
    i18n.t_with_args(key, &args)
}
