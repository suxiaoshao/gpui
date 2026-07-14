use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    text::{TextView, TextViewState},
    v_flex,
};
use jaco_core::{AgentRunId, AgentRunStatus, ConversationEntryId, ToolInvocationId};
use jaco_db::{AgentRunRecord, ConversationEntryRecord};

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

use super::attachments::{UserImageAttachment, render_user_image_attachments};

pub(super) type OnToggleAgent = Rc<dyn Fn(AgentRunId, &mut Window, &mut App) + 'static>;
pub(super) type OnCopy = Rc<dyn Fn(String, &mut Window, &mut App) -> bool + 'static>;
pub(super) type OnApprovalDecision =
    Rc<dyn Fn(ToolInvocationId, bool, &mut Window, &mut App) + 'static>;

const COPIED_STATE_DURATION: Duration = Duration::from_secs(2);

struct CopyButtonState {
    copied_at: Option<Instant>,
}

impl CopyButtonState {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { copied_at: None }
    }

    fn is_copied(&self) -> bool {
        self.copied_at
            .is_some_and(|copied_at| copied_at.elapsed() < COPIED_STATE_DURATION)
    }

    fn mark_copied(&mut self) {
        self.copied_at = Some(Instant::now());
    }
}

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
                    .or_else(|| row.items.first().map(|item| item.id.clone()))
                    .unwrap_or_else(|| "agent".to_string()),
            ),
        }
    }

    pub(super) fn contains_item(&self, item_id: &ConversationEntryId) -> bool {
        match self {
            TimelineRow::User(row) => &row.item.id == item_id,
            TimelineRow::Agent(row) => row.items.iter().any(|item| &item.id == item_id),
        }
    }
}

impl RenderOnce for TimelineRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            TimelineRow::User(row) => (*row).render(window, cx).into_any_element(),
            TimelineRow::Agent(row) => (*row).render(window, cx).into_any_element(),
        }
    }
}

#[derive(Clone)]
pub(super) struct UserMessageRow {
    pub(super) item: ConversationEntryRecord,
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
        let copy_button = copy_button(
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
                                .bg(cx.theme().primary.opacity(0.12))
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
    pub(super) run: Option<AgentRunRecord>,
    pub(super) items: Vec<ConversationEntryRecord>,
    pub(super) final_item: Option<ConversationEntryRecord>,
    pub(super) text_states: HashMap<ConversationEntryId, Entity<TextViewState>>,
    pub(super) expanded: bool,
    pub(super) on_toggle: OnToggleAgent,
    pub(super) on_copy: OnCopy,
    pub(super) on_approval_decision: OnApprovalDecision,
}

impl RenderOnce for AgentTurnRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id_suffix = self
            .run_id
            .clone()
            .or_else(|| self.items.first().map(|item| item.id.clone()))
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
                        .first()
                        .map(|item| format::timestamp_label(item.created_at, i18n))
                        .unwrap_or_default()
                });
            (
                i18n.t("conversation-copy-tooltip"),
                i18n.t("conversation-copy-success"),
                hover_time,
                agent_final_markdown(self.final_item.as_ref(), i18n),
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
            .final_item
            .as_ref()
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
            .bg(cx.theme().border.opacity(0.7))
            .into_any_element()
    }

    fn render_details(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let detail_items = self
            .items
            .iter()
            .filter(|item| {
                self.final_item
                    .as_ref()
                    .is_none_or(|final_item| final_item.id != item.id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let text_states = self.text_states.clone();
        let decided_tool_invocation_ids = self
            .items
            .iter()
            .filter_map(|item| match &item.payload {
                jaco_core::ConversationEntryPayload::ApprovalDecision(decision) => {
                    Some(decision.tool_invocation_id.clone())
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        let terminal_tool_invocation_ids = self
            .items
            .iter()
            .filter_map(|item| match &item.payload {
                jaco_core::ConversationEntryPayload::ToolResult(_) => {
                    item.tool_invocation_id.clone()
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        let on_approval_decision = self.on_approval_decision.clone();
        let mut blocks = Vec::with_capacity(detail_items.len());
        for item in detail_items {
            let text_state = text_states.get(&item.id).cloned();
            let approval_decidable = match &item.payload {
                jaco_core::ConversationEntryPayload::ApprovalRequest(request) => {
                    approval_request_decidable(
                        request,
                        &decided_tool_invocation_ids,
                        &terminal_tool_invocation_ids,
                    )
                }
                _ => false,
            };
            blocks.push(super::tool_blocks::detail_block(
                item,
                text_state,
                approval_decidable,
                on_approval_decision.clone(),
                window,
                cx,
            ));
        }

        v_flex()
            .max_w(px(760.))
            .min_w_0()
            .gap_2()
            .children(blocks)
            .into_any_element()
    }
}

fn approval_request_decidable(
    request: &jaco_core::ApprovalRequestEntry,
    decided_tool_invocation_ids: &HashSet<ToolInvocationId>,
    terminal_tool_invocation_ids: &HashSet<ToolInvocationId>,
) -> bool {
    !decided_tool_invocation_ids.contains(&request.tool_invocation_id)
        && !terminal_tool_invocation_ids.contains(&request.tool_invocation_id)
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
    let copy_button = copy_button(
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

fn copy_button(
    id: String,
    copy_text: String,
    on_copy: OnCopy,
    copy_tooltip: String,
    copied_tooltip: String,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state_key = format!("{id}-copied-state");
    let state: Entity<CopyButtonState> =
        window.use_keyed_state(state_key, cx, CopyButtonState::new);
    let is_copied = state.read(cx).is_copied();
    let icon = if is_copied {
        Icon::new(IconName::Check).text_color(cx.theme().success)
    } else {
        Icon::new(IconName::Copy)
    };
    let tooltip = if is_copied {
        copied_tooltip
    } else {
        copy_tooltip
    };

    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .tooltip(tooltip)
        .disabled(is_copied)
        .on_click(move |_, window, cx| {
            if !on_copy(copy_text.clone(), window, cx) {
                return;
            }

            state.update(cx, |state, cx| {
                state.mark_copied();
                cx.notify();
            });

            let state_id = state.entity_id();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(COPIED_STATE_DURATION).await;
                cx.update(|cx| {
                    cx.notify(state_id);
                })
            })
            .detach();
        })
        .into_any_element()
}

fn agent_copy_text(row: &AgentTurnRow, i18n: &I18n) -> String {
    let final_markdown = agent_final_markdown(row.final_item.as_ref(), i18n);
    if !row.expanded && !final_markdown.trim().is_empty() {
        return final_markdown;
    }

    let parts = row
        .items
        .iter()
        .map(format::item_markdown)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>();
    parts.join("\n\n")
}

fn agent_final_markdown(final_item: Option<&ConversationEntryRecord>, i18n: &I18n) -> String {
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

fn agent_terminal_status_label(
    status: AgentRunStatus,
    run: &AgentRunRecord,
    i18n: &I18n,
) -> String {
    let key = match status {
        AgentRunStatus::Completed => "conversation-agent-processed",
        AgentRunStatus::Failed => "conversation-agent-failed",
        AgentRunStatus::Canceled => "conversation-agent-canceled",
        AgentRunStatus::Queued | AgentRunStatus::Running => "conversation-agent-processing",
    };
    duration_arg_label(i18n, key, format::run_duration_label(run))
}

fn agent_hover_time(run: &AgentRunRecord, i18n: &I18n) -> String {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use jaco_core::{ApprovalRequestEntry, ApprovalRequestPayload, ToolSource};

    use super::approval_request_decidable;

    fn approval_request() -> ApprovalRequestEntry {
        ApprovalRequestEntry {
            tool_invocation_id: "tool-1".to_string(),
            request: ApprovalRequestPayload {
                reason: "needs approval".to_string(),
                tool_source: ToolSource::Local,
                tool_name: "write_file".to_string(),
                arguments_preview: "{}".to_string(),
                access_requests: Vec::new(),
            },
        }
    }

    #[test]
    fn approval_request_is_not_decidable_after_decision_item() {
        let request = approval_request();
        let decided_approval_ids = HashSet::from([request.tool_invocation_id.clone()]);
        let terminal_tool_invocation_ids = HashSet::new();

        assert!(!approval_request_decidable(
            &request,
            &decided_approval_ids,
            &terminal_tool_invocation_ids
        ));
    }

    #[test]
    fn approval_request_is_not_decidable_after_terminal_tool_result() {
        let request = approval_request();
        let decided_approval_ids = HashSet::new();
        let terminal_tool_invocation_ids = HashSet::from([request.tool_invocation_id.clone()]);

        assert!(!approval_request_decidable(
            &request,
            &decided_approval_ids,
            &terminal_tool_invocation_ids
        ));
    }
}
