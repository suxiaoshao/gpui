use std::{
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use ai_chat_core::{AgentRunId, ConversationItemId};
use ai_chat_db::{AgentRunRecord, ConversationItemRecord};
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

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

pub(super) type OnToggleAgent = Rc<dyn Fn(AgentRunId, &mut Window, &mut App) + 'static>;
pub(super) type OnCopy = Rc<dyn Fn(String, &mut Window, &mut App) -> bool + 'static>;

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

    pub(super) fn contains_item(&self, item_id: &ConversationItemId) -> bool {
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
    pub(super) item: ConversationItemRecord,
    pub(super) text_state: Option<Entity<TextViewState>>,
    pub(super) on_copy: OnCopy,
}

impl RenderOnce for UserMessageRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = format!("conversation-user-message-{}", self.item.id);
        let markdown = format::item_markdown(&self.item);
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
                    .gap_1()
                    .child(
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
    pub(super) items: Vec<ConversationItemRecord>,
    pub(super) final_item: Option<ConversationItemRecord>,
    pub(super) text_states: HashMap<ConversationItemId, Entity<TextViewState>>,
    pub(super) expanded: bool,
    pub(super) on_toggle: OnToggleAgent,
    pub(super) on_copy: OnCopy,
}

impl RenderOnce for AgentTurnRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id_suffix = self
            .run_id
            .clone()
            .or_else(|| self.items.first().map(|item| item.id.clone()))
            .unwrap_or_else(|| "agent".to_string());
        let group = format!("conversation-agent-turn-{id_suffix}");
        let copy_text = agent_copy_text(&self);
        let on_copy = self.on_copy.clone();
        let i18n = cx.global::<I18n>();
        let copy_tooltip = i18n.t("conversation-copy-tooltip");
        let copied_tooltip = i18n.t("conversation-copy-success");
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
        let final_markdown = self
            .final_item
            .as_ref()
            .map(format::item_markdown)
            .unwrap_or_default();
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
            .when(self.expanded, |this| this.child(self.render_details(cx)))
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
            if format::is_terminal_run(run) && self.final_item.is_some() {
                (
                    duration_arg_label(
                        i18n,
                        "conversation-agent-processed",
                        format::run_duration_label(run),
                    ),
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

    fn render_details(&self, cx: &mut App) -> AnyElement {
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

        v_flex()
            .max_w(px(760.))
            .min_w_0()
            .gap_2()
            .children(detail_items.into_iter().map(|item| {
                let text_state = text_states.get(&item.id).cloned();
                detail_block(item, text_state, cx)
            }))
            .into_any_element()
    }
}

fn detail_block(
    item: ConversationItemRecord,
    text_state: Option<Entity<TextViewState>>,
    cx: &mut App,
) -> AnyElement {
    let label = payload_label(&item);
    let markdown = format::item_markdown(&item);

    v_flex()
        .id(format!("conversation-agent-detail-{}", item.id))
        .min_w_0()
        .gap_1()
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().border.opacity(0.7))
        .bg(cx.theme().muted.opacity(0.28))
        .px_3()
        .py_2()
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .when(!markdown.is_empty(), |this| {
            this.child(markdown_view(
                format!("conversation-agent-detail-markdown-{}", item.id),
                text_state,
                &markdown,
            ))
        })
        .into_any_element()
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

fn payload_label(item: &ConversationItemRecord) -> String {
    match &item.payload {
        ai_chat_core::ConversationItemPayload::Message { role, .. } => format!("{role:?}"),
        ai_chat_core::ConversationItemPayload::SkillActivation(skill) => {
            format!("Skill {}", skill.name)
        }
        ai_chat_core::ConversationItemPayload::Reasoning { .. } => "Reasoning".to_string(),
        ai_chat_core::ConversationItemPayload::ToolCall(call) => {
            format!("Tool call {}", call.runtime_tool_name)
        }
        ai_chat_core::ConversationItemPayload::ToolResult(result) => {
            format!("Tool result {}", result.call_id)
        }
        ai_chat_core::ConversationItemPayload::ApprovalRequest(_) => "Approval request".to_string(),
        ai_chat_core::ConversationItemPayload::ApprovalDecision(_) => {
            "Approval decision".to_string()
        }
        ai_chat_core::ConversationItemPayload::Status(status) => status.label.clone(),
        ai_chat_core::ConversationItemPayload::Error(_) => "Error".to_string(),
    }
}

fn agent_copy_text(row: &AgentTurnRow) -> String {
    if let Some(final_item) = &row.final_item
        && !row.expanded
    {
        return format::item_markdown(final_item);
    }
    row.items
        .iter()
        .map(format::item_markdown)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
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
