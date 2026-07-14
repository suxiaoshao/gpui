use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    text::{TextView, TextViewState},
    v_flex,
};
use jaco_core::ConversationEntryPayload;
use jaco_db::ConversationEntryRecord;

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

use super::message::OnApprovalDecision;

struct DetailBlockState {
    expanded: bool,
}

#[derive(Clone, Copy)]
enum DetailTone {
    Normal,
    Success,
    Warning,
    Danger,
}

pub(super) fn detail_block(
    item: ConversationEntryRecord,
    text_state: Option<Entity<TextViewState>>,
    approval_decidable: bool,
    on_approval_decision: OnApprovalDecision,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let default_expanded = default_expanded(&item.payload);
    let state: Entity<DetailBlockState> = window.use_keyed_state(
        format!("conversation-agent-detail-state-{}", item.id),
        cx,
        move |_window, _cx| DetailBlockState {
            expanded: default_expanded,
        },
    );
    let expanded = state.read(cx).expanded;
    let title = detail_title(&item, cx.global::<I18n>());
    let icon = detail_icon(&item.payload);
    let tone = detail_tone(&item.payload);
    let markdown = format::item_markdown(&item);
    let approval_actions = if approval_decidable {
        approval_action_buttons(&item.payload, on_approval_decision.clone(), cx)
    } else {
        None
    };
    let toggle_state = state.clone();
    let chevron = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    v_flex()
        .id(format!("conversation-agent-detail-{}", item.id))
        .min_w_0()
        .gap_1()
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().border.opacity(0.7))
        .bg(cx.theme().muted.opacity(0.28))
        .px_2()
        .py_1()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_1p5()
                .child(tinted_icon(icon, tone, cx))
                .child(
                    Label::new(title)
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                        .truncate(),
                )
                .child(div().flex_1())
                .when_some(approval_actions, |this, actions| this.child(actions))
                .child(
                    Button::new(format!("conversation-agent-detail-toggle-{}", item.id))
                        .ghost()
                        .xsmall()
                        .icon(chevron)
                        .on_click(move |_, _window, cx| {
                            toggle_state.update(cx, |state, cx| {
                                state.expanded = !state.expanded;
                                cx.notify();
                            });
                        }),
                ),
        )
        .when(expanded && !markdown.is_empty(), |this| {
            this.child(div().px_1().pb_1().child(markdown_view(
                format!("conversation-agent-detail-markdown-{}", item.id),
                text_state,
                &markdown,
            )))
        })
        .into_any_element()
}

fn approval_action_buttons(
    payload: &ConversationEntryPayload,
    on_approval_decision: OnApprovalDecision,
    cx: &mut App,
) -> Option<AnyElement> {
    let ConversationEntryPayload::ApprovalRequest(request) = payload else {
        return None;
    };
    let approve_id = request.tool_invocation_id.clone();
    let deny_id = request.tool_invocation_id.clone();
    let approve = cx.global::<I18n>().t("conversation-approval-approve");
    let deny = cx.global::<I18n>().t("conversation-approval-deny");
    let approve_callback = on_approval_decision.clone();

    Some(
        h_flex()
            .items_center()
            .gap_1()
            .child(
                Button::new(format!("conversation-approval-approve-{approve_id}"))
                    .small()
                    .icon(IconName::ShieldCheck)
                    .label(approve)
                    .on_click(move |_, window, cx| {
                        approve_callback(approve_id.clone(), true, window, cx);
                    }),
            )
            .child(
                Button::new(format!("conversation-approval-deny-{deny_id}"))
                    .ghost()
                    .small()
                    .icon(IconName::ShieldAlert)
                    .label(deny)
                    .on_click(move |_, window, cx| {
                        on_approval_decision(deny_id.clone(), false, window, cx);
                    }),
            )
            .into_any_element(),
    )
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

fn default_expanded(payload: &ConversationEntryPayload) -> bool {
    !matches!(
        payload,
        ConversationEntryPayload::ToolCall(_)
            | ConversationEntryPayload::ToolResult(_)
            | ConversationEntryPayload::ApprovalRequest(_)
            | ConversationEntryPayload::ApprovalDecision(_)
    )
}

fn detail_title(item: &ConversationEntryRecord, i18n: &I18n) -> String {
    match &item.payload {
        ConversationEntryPayload::ToolCall(call) => label_with_name(
            i18n,
            "conversation-tool-call",
            call.runtime_tool_name.as_str(),
        ),
        ConversationEntryPayload::ToolResult(result) => {
            label_with_name(i18n, "conversation-tool-result", result.call_id.as_str())
        }
        ConversationEntryPayload::ApprovalRequest(_) => i18n.t("conversation-approval-request"),
        ConversationEntryPayload::ApprovalDecision(decision) => {
            i18n.t(if decision.decision.approved {
                "conversation-approval-approved"
            } else {
                "conversation-approval-denied"
            })
        }
        ConversationEntryPayload::SkillActivation(skill) => {
            label_with_name(i18n, "conversation-skill-activation", skill.name.as_str())
        }
        ConversationEntryPayload::Reasoning { .. } => i18n.t("conversation-reasoning"),
        ConversationEntryPayload::Message { role, .. } => format!("{role:?}"),
        ConversationEntryPayload::Status(status) => i18n.t(format::status_i18n_key(status.code)),
        ConversationEntryPayload::Error(_) => i18n.t("conversation-error"),
    }
}

fn detail_icon(payload: &ConversationEntryPayload) -> IconName {
    match payload {
        ConversationEntryPayload::ToolCall(call) => tool_icon(&call.runtime_tool_name),
        ConversationEntryPayload::ToolResult(result) => {
            if result.is_error {
                IconName::CircleAlert
            } else {
                IconName::CircleCheck
            }
        }
        ConversationEntryPayload::ApprovalRequest(_) => IconName::ShieldAlert,
        ConversationEntryPayload::ApprovalDecision(decision) => {
            if decision.decision.approved {
                IconName::ShieldCheck
            } else {
                IconName::ShieldAlert
            }
        }
        ConversationEntryPayload::SkillActivation(_) => IconName::Sparkles,
        ConversationEntryPayload::Reasoning { .. } => IconName::Lightbulb,
        ConversationEntryPayload::Error(_) => IconName::CircleAlert,
        ConversationEntryPayload::Message { .. } => IconName::MessageSquare,
        ConversationEntryPayload::Status(_) => IconName::CircleCheck,
    }
}

fn detail_tone(payload: &ConversationEntryPayload) -> DetailTone {
    match payload {
        ConversationEntryPayload::ToolResult(result) if result.is_error => DetailTone::Danger,
        ConversationEntryPayload::ToolResult(_) => DetailTone::Success,
        ConversationEntryPayload::ApprovalRequest(_) => DetailTone::Warning,
        ConversationEntryPayload::ApprovalDecision(decision) if decision.decision.approved => {
            DetailTone::Success
        }
        ConversationEntryPayload::ApprovalDecision(_) | ConversationEntryPayload::Error(_) => {
            DetailTone::Danger
        }
        _ => DetailTone::Normal,
    }
}

fn tool_icon(tool_name: &str) -> IconName {
    match tool_name {
        "read_file" => IconName::FileText,
        "list_directory" => IconName::FolderOpen,
        "find_path" => IconName::FileSearch,
        "grep" => IconName::Search,
        "write_file" | "edit_file" => IconName::FilePen,
        name if name.contains("shell") || name.contains("exec") => IconName::Terminal,
        _ => IconName::Wrench,
    }
}

fn tinted_icon(icon: IconName, tone: DetailTone, cx: &mut App) -> Icon {
    let icon = Icon::new(icon).size_4();
    match tone {
        DetailTone::Normal => icon.text_color(cx.theme().muted_foreground),
        DetailTone::Success => icon.text_color(cx.theme().success),
        DetailTone::Warning => icon.text_color(cx.theme().warning),
        DetailTone::Danger => icon.text_color(cx.theme().danger),
    }
}

fn label_with_name(i18n: &I18n, key: &str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name);
    i18n.t_with_args(key, &args)
}
