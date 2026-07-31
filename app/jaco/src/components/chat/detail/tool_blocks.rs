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
use jaco_core::{ConversationEntry, ConversationEntryPayload};

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

use super::message::OnApprovalDecision;

struct DetailBlockState {
    expanded: bool,
}

#[derive(IntoElement)]
pub(super) struct DetailBlock {
    state: Entity<DetailBlockState>,
    item: ConversationEntry,
    text_state: Option<Entity<TextViewState>>,
    approval_decidable: bool,
    on_approval_decision: OnApprovalDecision,
}

impl DetailBlock {
    pub(super) fn new(
        item: ConversationEntry,
        text_state: Option<Entity<TextViewState>>,
        approval_decidable: bool,
        on_approval_decision: OnApprovalDecision,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let default_expanded = default_expanded(&item.payload);
        let state = window.use_keyed_state(
            format!("conversation-agent-detail-state-{}", item.id),
            cx,
            move |_window, _cx| DetailBlockState {
                expanded: default_expanded,
            },
        );
        Self {
            state,
            item,
            text_state,
            approval_decidable,
            on_approval_decision,
        }
    }
}

#[derive(Clone, Copy)]
enum DetailTone {
    Normal,
    Success,
    Warning,
    Danger,
}

impl View for DetailBlock {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let expanded = self.state.read(cx).expanded;
        let title = detail_title(&self.item, cx.global::<I18n>());
        let icon = detail_icon(&self.item.payload);
        let tone = detail_tone(&self.item.payload);
        let markdown = format::item_markdown(&self.item);
        let approval_actions = if self.approval_decidable {
            approval_action_buttons(&self.item.payload, self.on_approval_decision.clone(), cx)
        } else {
            None
        };
        let toggle_state = self.state.clone();
        let item_id = self.item.id;
        let text_state = self.text_state;
        let chevron = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };

        v_flex()
            .id(format!("conversation-agent-detail-{item_id}"))
            .min_w_0()
            .gap_1()
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border.opacity(0.7))
            .bg(cx.theme().tokens.muted.background.opacity(0.28))
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
                        Button::new(format!("conversation-agent-detail-toggle-{item_id}"))
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
                    format!("conversation-agent-detail-markdown-{item_id}"),
                    text_state,
                    &markdown,
                )))
            })
    }
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

fn detail_title(item: &ConversationEntry, i18n: &I18n) -> String {
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{Context, Entity, EntityId, IntoElement, Render, TestAppContext, Window, div};
    use jaco_core::{
        ConversationEntry, ConversationEntryPayload, ConversationEntryStatus, TranscriptRole,
    };
    use time::OffsetDateTime;

    use super::DetailBlock;

    struct Snapshot {
        identity: Option<EntityId>,
        sibling_identity: Option<EntityId>,
    }

    struct TestRoot {
        revision: usize,
        snapshots: Rc<RefCell<[Option<Snapshot>; 2]>>,
    }

    impl Render for TestRoot {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            DetailBlockTestView {
                state: cx.entity().clone(),
            }
        }
    }

    #[derive(IntoElement)]
    struct DetailBlockTestView {
        state: Entity<TestRoot>,
    }

    impl gpui::View for DetailBlockTestView {
        fn entity_id(&self) -> Option<EntityId> {
            Some(self.state.entity_id())
        }

        fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
            let (revision, snapshots) = {
                let state = self.state.read(cx);
                (state.revision, state.snapshots.clone())
            };
            let (search_text, approval_decidable) = if revision == 0 {
                ("first payload", false)
            } else {
                ("refreshed payload", true)
            };
            let block = DetailBlock::new(
                entry("detail-1", search_text),
                None,
                approval_decidable,
                Rc::new(|_, _, _, _| {}),
                window,
                cx,
            );
            let sibling = DetailBlock::new(
                entry("detail-2", "sibling payload"),
                None,
                false,
                Rc::new(|_, _, _, _| {}),
                window,
                cx,
            );
            snapshots.borrow_mut()[revision] = Some(Snapshot {
                identity: block.entity_id(),
                sibling_identity: sibling.entity_id(),
            });
            div()
        }
    }

    #[gpui::test]
    fn detail_block_keyed_state_preserves_identity_across_rebuilds_and_siblings(
        cx: &mut TestAppContext,
    ) {
        let snapshots = Rc::new(RefCell::new([None, None]));
        let (root, cx) = cx.add_window_view(|_, _| TestRoot {
            revision: 0,
            snapshots: snapshots.clone(),
        });
        cx.run_until_parked();
        root.update(cx, |root, cx| {
            root.revision = 1;
            cx.notify();
        });
        cx.run_until_parked();

        let snapshots = snapshots.borrow();
        let first = snapshots[0].as_ref().expect("first render snapshot");
        let refreshed = snapshots[1].as_ref().expect("refreshed render snapshot");

        assert_eq!(refreshed.identity, first.identity);
        assert_ne!(first.sibling_identity, first.identity);
        assert_ne!(refreshed.sibling_identity, refreshed.identity);
    }

    fn entry(id: &str, text: &str) -> ConversationEntry {
        let payload = ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            content: vec![jaco_core::ContentPart::Text {
                text: text.to_string(),
            }],
        };
        ConversationEntry {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            seq: 1,
            kind: payload.kind(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            search_text: payload.search_text(),
            payload,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
