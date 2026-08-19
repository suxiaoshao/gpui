use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex,
    label::Label,
    v_flex,
};
use jaco_core::{
    AgentRunId, ApprovalStatus, ContentPart, ConversationEntry, ConversationEntryId,
    ConversationEntryPayload, ToolAccessKind, ToolAccessRequestPayload, ToolInvocation,
    ToolInvocationId, ToolInvocationStatus, ToolSource,
};
use serde_json::Value;
use time::{Duration, OffsetDateTime};

use crate::foundation::{I18n, assets::IconName, conversation_format as format};

use super::{
    copy_button::{CopyButton, OnCopy},
    message::OnApprovalDecision,
};

pub(super) type OnToggleToolInvocation =
    Rc<dyn Fn(ToolInvocationId, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ToolPreviewLimits {
    max_depth: usize,
    max_nodes: usize,
    max_input_bytes: usize,
    max_string_bytes: usize,
    max_output_bytes: usize,
    max_lines: usize,
}

const TOOL_PREVIEW_LIMITS: ToolPreviewLimits = ToolPreviewLimits {
    max_depth: 12,
    max_nodes: 2_048,
    max_input_bytes: 256 * 1_024,
    max_string_bytes: 8 * 1_024,
    max_output_bytes: 64 * 1_024,
    max_lines: 1_000,
};

const TRUNCATION_MARKER: &str = "[TRUNCATED]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BoundedPreview {
    pub(super) text: String,
    pub(super) truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolAccessPreview {
    pub(super) kind: ToolAccessKind,
    pub(super) target: BoundedPreview,
    pub(super) normalized_path: Option<BoundedPreview>,
    pub(super) within_project: bool,
    pub(super) reason_key: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolErrorPreview {
    pub(super) code: BoundedPreview,
    pub(super) message: BoundedPreview,
    pub(super) retryable: bool,
    pub(super) provider: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolApprovalDecisionPreview {
    pub(super) approved: bool,
    pub(super) decided_by: BoundedPreview,
    pub(super) reason: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolApprovalPreview {
    pub(super) status: ApprovalStatus,
    pub(super) request_reason: BoundedPreview,
    pub(super) decision: Option<ToolApprovalDecisionPreview>,
    pub(super) requested_at: OffsetDateTime,
    pub(super) decided_at: Option<OffsetDateTime>,
    pub(super) expires_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolInvocationPreview {
    pub(super) arguments: BoundedPreview,
    pub(super) access_requests: Vec<ToolAccessPreview>,
    pub(super) access_requests_truncated: bool,
    pub(super) approval: Option<ToolApprovalPreview>,
    pub(super) text_output: Option<BoundedPreview>,
    pub(super) structured_output: Option<BoundedPreview>,
    pub(super) error: Option<ToolErrorPreview>,
    pub(super) provider_raw_hidden: bool,
}

#[derive(Clone)]
pub(super) struct ToolInvocationPreviewCacheEntry {
    pub(super) revision: OffsetDateTime,
    pub(super) preview: Arc<ToolInvocationPreview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolSourceKind {
    Local,
    Mcp,
    ProviderHosted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ToolOutcomeSummary {
    Succeeded,
    Failed { code: Option<BoundedPreview> },
    Denied { code: Option<BoundedPreview> },
    Canceled { code: Option<BoundedPreview> },
}

#[derive(Clone)]
pub(super) struct ToolInvocationDetail {
    pub(super) id: ToolInvocationId,
    pub(super) id_label: BoundedPreview,
    pub(super) agent_run_id: AgentRunId,
    pub(super) call_id: BoundedPreview,
    pub(super) source_kind: ToolSourceKind,
    pub(super) namespace: Option<BoundedPreview>,
    pub(super) server_or_provider_id: Option<BoundedPreview>,
    pub(super) tool_name: BoundedPreview,
    pub(super) runtime_tool_name: BoundedPreview,
    pub(super) status: ToolInvocationStatus,
    pub(super) approval_status: Option<ApprovalStatus>,
    pub(super) outcome: Option<ToolOutcomeSummary>,
    pub(super) created_at: OffsetDateTime,
    pub(super) started_at: Option<OffsetDateTime>,
    pub(super) completed_at: Option<OffsetDateTime>,
    pub(super) updated_at: OffsetDateTime,
    pub(super) expanded: bool,
    pub(super) approval_decidable: bool,
    pub(super) preview: Option<Arc<ToolInvocationPreview>>,
}

impl ToolInvocationDetail {
    pub(super) fn persisted_duration(&self) -> Option<Duration> {
        let started_at = self.started_at?;
        let end = if matches!(
            self.status,
            ToolInvocationStatus::Succeeded
                | ToolInvocationStatus::Failed
                | ToolInvocationStatus::Denied
                | ToolInvocationStatus::Canceled
        ) {
            self.completed_at?
        } else {
            self.updated_at
        };
        let duration = end - started_at;
        (!duration.is_negative()).then_some(duration)
    }
}

#[derive(Clone)]
pub(super) struct UnresolvedToolLifecycle {
    pub(super) anchor_entry_id: ConversationEntryId,
    pub(super) outer_invocation_id: Option<ToolInvocationId>,
    pub(super) outer_id_label: Option<BoundedPreview>,
    pub(super) entry_kinds: ToolLifecycleEntryKinds,
    pub(super) entry_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ToolLifecycleEntryKinds {
    pub(super) tool_call: bool,
    pub(super) tool_result: bool,
    pub(super) approval_request: bool,
    pub(super) approval_decision: bool,
}

impl ToolLifecycleEntryKinds {
    fn insert(&mut self, entry: &ConversationEntry) {
        match entry.payload {
            ConversationEntryPayload::ToolCall(_) => self.tool_call = true,
            ConversationEntryPayload::ToolResult(_) => self.tool_result = true,
            ConversationEntryPayload::ApprovalRequest(_) => self.approval_request = true,
            ConversationEntryPayload::ApprovalDecision(_) => self.approval_decision = true,
            _ => {}
        }
    }
}

#[derive(Clone)]
pub(super) enum AgentDetailItem {
    Entry(ConversationEntry),
    ToolInvocation(ToolInvocationDetail),
    UnresolvedToolLifecycle(UnresolvedToolLifecycle),
}

impl AgentDetailItem {
    pub(super) fn entry(&self) -> Option<&ConversationEntry> {
        match self {
            Self::Entry(entry) => Some(entry),
            Self::ToolInvocation(_) | Self::UnresolvedToolLifecycle(_) => None,
        }
    }

    pub(super) fn contains_entry_id(&self, id: &ConversationEntryId) -> bool {
        match self {
            Self::Entry(entry) => &entry.id == id,
            Self::UnresolvedToolLifecycle(unresolved) => &unresolved.anchor_entry_id == id,
            Self::ToolInvocation(_) => false,
        }
    }

    pub(super) fn stable_id_suffix(&self) -> String {
        match self {
            Self::Entry(entry) => format!("entry-{}", entry.id),
            Self::ToolInvocation(detail) => format!("tool-invocation-{}", detail.id),
            Self::UnresolvedToolLifecycle(unresolved) => {
                format!("unresolved-tool-{}", unresolved.anchor_entry_id)
            }
        }
    }
}

pub(super) fn is_tool_lifecycle_entry(entry: &ConversationEntry) -> bool {
    matches!(
        entry.payload,
        ConversationEntryPayload::ToolCall(_)
            | ConversationEntryPayload::ToolResult(_)
            | ConversationEntryPayload::ApprovalRequest(_)
            | ConversationEntryPayload::ApprovalDecision(_)
    )
}

pub(super) fn build_tool_invocation_preview(invocation: &ToolInvocation) -> ToolInvocationPreview {
    let arguments = bounded_json_preview(&invocation.input.arguments.value, TOOL_PREVIEW_LIMITS);
    let (access_requests, access_requests_truncated) = invocation
        .approval
        .as_ref()
        .map(|approval| {
            bounded_access_requests(&approval.request.access_requests, TOOL_PREVIEW_LIMITS)
        })
        .unwrap_or_default();
    let approval = invocation
        .approval
        .as_ref()
        .map(|approval| ToolApprovalPreview {
            status: approval.status,
            request_reason: bounded_metadata_preview(&approval.request.reason, TOOL_PREVIEW_LIMITS),
            decision: approval
                .decision
                .as_ref()
                .map(|decision| ToolApprovalDecisionPreview {
                    approved: decision.approved,
                    decided_by: bounded_metadata_preview(&decision.decided_by, TOOL_PREVIEW_LIMITS),
                    reason: decision
                        .reason
                        .as_deref()
                        .map(|reason| bounded_metadata_preview(reason, TOOL_PREVIEW_LIMITS)),
                }),
            requested_at: approval.requested_at,
            decided_at: approval.decided_at,
            expires_at: approval.expires_at,
        });

    let text_output = invocation
        .output
        .as_ref()
        .and_then(|output| bounded_content_parts(&output.content, TOOL_PREVIEW_LIMITS));
    let structured_output = invocation
        .output
        .as_ref()
        .and_then(|output| output.structured_output.as_ref())
        .map(|output| bounded_json_preview(&output.value, TOOL_PREVIEW_LIMITS));
    let error = invocation.error.as_ref().map(|error| ToolErrorPreview {
        code: bounded_metadata_preview(&error.code, TOOL_PREVIEW_LIMITS),
        message: bounded_text_preview(&error.message, TOOL_PREVIEW_LIMITS),
        retryable: error.retryable,
        provider: error
            .provider
            .as_deref()
            .map(|provider| bounded_metadata_preview(provider, TOOL_PREVIEW_LIMITS)),
    });

    ToolInvocationPreview {
        arguments,
        access_requests,
        access_requests_truncated,
        approval,
        text_output,
        structured_output,
        error,
        provider_raw_hidden: invocation
            .output
            .as_ref()
            .is_some_and(|output| output.raw_output.is_some())
            || invocation
                .error
                .as_ref()
                .is_some_and(|error| error.raw.is_some()),
    }
}

pub(super) fn project_tool_invocation_detail(
    invocation: &ToolInvocation,
    expanded: bool,
    preview: Option<Arc<ToolInvocationPreview>>,
    broker_decidable: bool,
) -> ToolInvocationDetail {
    let (source_kind, server_or_provider_id) = match &invocation.source {
        ToolSource::Local => (ToolSourceKind::Local, None),
        ToolSource::Mcp { server_id } => (
            ToolSourceKind::Mcp,
            Some(bounded_metadata_preview(
                invocation.server_id.as_deref().unwrap_or(server_id),
                TOOL_PREVIEW_LIMITS,
            )),
        ),
        ToolSource::ProviderHosted { provider_id } => (
            ToolSourceKind::ProviderHosted,
            Some(bounded_metadata_preview(provider_id, TOOL_PREVIEW_LIMITS)),
        ),
    };
    let approval_status = invocation.approval.as_ref().map(|approval| approval.status);
    let error_code = || {
        invocation
            .error
            .as_ref()
            .map(|error| bounded_metadata_preview(&error.code, TOOL_PREVIEW_LIMITS))
    };
    let outcome = match invocation.status {
        ToolInvocationStatus::Succeeded => Some(ToolOutcomeSummary::Succeeded),
        ToolInvocationStatus::Failed => Some(ToolOutcomeSummary::Failed { code: error_code() }),
        ToolInvocationStatus::Denied => Some(ToolOutcomeSummary::Denied { code: error_code() }),
        ToolInvocationStatus::Canceled => Some(ToolOutcomeSummary::Canceled { code: error_code() }),
        ToolInvocationStatus::Requested
        | ToolInvocationStatus::AwaitingApproval
        | ToolInvocationStatus::Running => None,
    };

    ToolInvocationDetail {
        id: invocation.id.clone(),
        id_label: bounded_metadata_preview(&invocation.id, TOOL_PREVIEW_LIMITS),
        agent_run_id: invocation.agent_run_id.clone(),
        call_id: bounded_metadata_preview(&invocation.call_id, TOOL_PREVIEW_LIMITS),
        source_kind,
        namespace: invocation
            .namespace
            .as_deref()
            .map(|namespace| bounded_metadata_preview(namespace, TOOL_PREVIEW_LIMITS)),
        server_or_provider_id,
        tool_name: bounded_metadata_preview(&invocation.tool_name, TOOL_PREVIEW_LIMITS),
        runtime_tool_name: bounded_metadata_preview(
            &invocation.runtime_tool_name,
            TOOL_PREVIEW_LIMITS,
        ),
        status: invocation.status,
        approval_status,
        outcome,
        created_at: invocation.created_at,
        started_at: invocation.started_at,
        completed_at: invocation.completed_at,
        updated_at: invocation.updated_at,
        expanded,
        approval_decidable: broker_decidable
            && invocation.status == ToolInvocationStatus::AwaitingApproval
            && approval_status == Some(ApprovalStatus::Pending),
        preview,
    }
}

pub(super) fn project_agent_details<'a>(
    entries: impl IntoIterator<Item = &'a ConversationEntry>,
    invocations: impl IntoIterator<Item = &'a ToolInvocation>,
    expanded: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
) -> Vec<AgentDetailItem> {
    let mut ordered_entries = entries.into_iter().collect::<Vec<_>>();
    ordered_entries.sort_by_key(|entry| entry.seq);
    let invocations = invocations.into_iter().collect::<Vec<_>>();
    let invocation_by_id = invocations
        .iter()
        .copied()
        .map(|invocation| (invocation.id.as_str(), invocation))
        .collect::<HashMap<_, _>>();
    let mut unresolved_by_id = HashMap::<&str, UnresolvedToolLifecycle>::new();
    for entry in ordered_entries
        .iter()
        .copied()
        .filter(|entry| is_tool_lifecycle_entry(entry))
    {
        let Some(id) = entry.tool_invocation_id.as_deref() else {
            continue;
        };
        if invocation_by_id
            .get(id)
            .is_some_and(|invocation| entry.agent_run_id.as_ref() == Some(&invocation.agent_run_id))
        {
            continue;
        }
        let unresolved = unresolved_by_id.entry(id).or_insert_with(|| {
            let mut entry_kinds = ToolLifecycleEntryKinds::default();
            entry_kinds.insert(entry);
            UnresolvedToolLifecycle {
                anchor_entry_id: entry.id.clone(),
                outer_invocation_id: Some(id.to_owned()),
                outer_id_label: Some(bounded_metadata_preview(id, TOOL_PREVIEW_LIMITS)),
                entry_kinds,
                entry_count: 0,
            }
        });
        unresolved.entry_kinds.insert(entry);
        unresolved.entry_count += 1;
    }

    let mut emitted_invocations = HashSet::<&str>::new();
    let mut emitted_unresolved = HashSet::<&str>::new();
    let mut items = Vec::with_capacity(ordered_entries.len().saturating_add(invocations.len()));

    for entry in ordered_entries {
        if !is_tool_lifecycle_entry(entry) {
            items.push(AgentDetailItem::Entry(entry.clone()));
            continue;
        }

        let Some(id) = entry.tool_invocation_id.as_deref() else {
            let mut entry_kinds = ToolLifecycleEntryKinds::default();
            entry_kinds.insert(entry);
            items.push(AgentDetailItem::UnresolvedToolLifecycle(
                UnresolvedToolLifecycle {
                    anchor_entry_id: entry.id.clone(),
                    outer_invocation_id: None,
                    outer_id_label: None,
                    entry_kinds,
                    entry_count: 1,
                },
            ));
            continue;
        };

        if let Some(invocation) = invocation_by_id.get(id).copied()
            && entry.agent_run_id.as_ref() == Some(&invocation.agent_run_id)
        {
            if emitted_invocations.insert(id) {
                items.push(AgentDetailItem::ToolInvocation(
                    detail_from_projection_state(
                        invocation,
                        expanded,
                        previews,
                        approval_decidable,
                    ),
                ));
            }
        } else if emitted_unresolved.insert(id)
            && let Some(unresolved) = unresolved_by_id.get(id)
        {
            items.push(AgentDetailItem::UnresolvedToolLifecycle(unresolved.clone()));
        }
    }

    let mut orphans = invocations
        .iter()
        .copied()
        .filter(|invocation| !emitted_invocations.contains(invocation.id.as_str()))
        .collect::<Vec<_>>();
    orphans.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    items.extend(orphans.into_iter().map(|invocation| {
        AgentDetailItem::ToolInvocation(detail_from_projection_state(
            invocation,
            expanded,
            previews,
            approval_decidable,
        ))
    }));
    items
}

fn detail_from_projection_state(
    invocation: &ToolInvocation,
    expanded: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
) -> ToolInvocationDetail {
    let is_expanded = expanded.get(&invocation.id).copied().unwrap_or(false);
    let preview = is_expanded
        .then(|| previews.get(&invocation.id))
        .flatten()
        .filter(|entry| entry.revision == invocation.updated_at)
        .map(|entry| entry.preview.clone());
    project_tool_invocation_detail(
        invocation,
        is_expanded,
        preview,
        approval_decidable.contains(&invocation.id),
    )
}

#[derive(IntoElement)]
pub(super) struct ToolInvocationBlock {
    detail: ToolInvocationDetail,
    on_toggle: OnToggleToolInvocation,
    on_copy: OnCopy,
    on_approval_decision: OnApprovalDecision,
}

impl ToolInvocationBlock {
    pub(super) fn new(
        detail: ToolInvocationDetail,
        on_toggle: OnToggleToolInvocation,
        on_copy: OnCopy,
        on_approval_decision: OnApprovalDecision,
    ) -> Self {
        Self {
            detail,
            on_toggle,
            on_copy,
            on_approval_decision,
        }
    }
}

impl RenderOnce for ToolInvocationBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let id = self.detail.id.clone();
        let model_name = self.detail.runtime_tool_name.text.clone();
        let title = label_with_name(i18n, "conversation-tool-invocation-title", &model_name);
        let source = source_summary_label(&self.detail, i18n);
        let status = self
            .detail
            .outcome
            .as_ref()
            .map(|outcome| outcome_label(outcome, i18n))
            .unwrap_or_else(|| i18n.t(status_key(self.detail.status)));
        let approval_status = self
            .detail
            .approval_status
            .map(|status| i18n.t(approval_status_key(status)));
        let duration = duration_summary_label(&self.detail, i18n);
        let expanded = self.detail.expanded;
        let toggle_tooltip = label_with_name(
            i18n,
            if expanded {
                "conversation-tool-collapse"
            } else {
                "conversation-tool-expand"
            },
            &model_name,
        );
        let toggle_icon = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let toggle_id = id.clone();
        let on_toggle = self.on_toggle;
        let approval_actions = self
            .detail
            .approval_decidable
            .then(|| approval_action_buttons(&id, self.on_approval_decision, i18n));
        let content = render_tool_invocation_content(&self.detail, self.on_copy, window, cx);

        div()
            .id(format!("tool-invocation-{id}-root"))
            .w_full()
            .min_w_0()
            .child(
                Collapsible::new()
                    .w_full()
                    .min_w_0()
                    .gap_1()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(cx.theme().border.opacity(0.7))
                    .bg(cx.theme().tokens.muted.background.opacity(0.28))
                    .px_2()
                    .py_1()
                    .open(expanded)
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(IconName::Wrench).text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Label::new(title)
                                    .text_xs()
                                    .font_medium()
                                    .text_color(cx.theme().foreground)
                                    .truncate(),
                            )
                            .child(
                                Label::new(source)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Label::new(status)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .when_some(approval_status, |this, approval_status| {
                                this.child(
                                    Label::new(approval_status)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            })
                            .child(
                                Label::new(duration)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(div().flex_1())
                            .when_some(approval_actions, |this, actions| this.child(actions))
                            .child(
                                Button::new(format!("tool-invocation-{id}-toggle"))
                                    .ghost()
                                    .xsmall()
                                    .icon(toggle_icon)
                                    .tooltip(toggle_tooltip)
                                    .on_click(move |_, window, cx| {
                                        on_toggle(toggle_id.clone(), window, cx);
                                    }),
                            ),
                    )
                    .content(content),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct UnresolvedToolLifecycleBlock {
    unresolved: UnresolvedToolLifecycle,
}

impl UnresolvedToolLifecycleBlock {
    pub(super) fn new(unresolved: UnresolvedToolLifecycle) -> Self {
        Self { unresolved }
    }
}

impl RenderOnce for UnresolvedToolLifecycleBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let id_label = self
            .unresolved
            .outer_id_label
            .as_ref()
            .map(|label| label.text.clone())
            .unwrap_or_else(|| i18n.t("conversation-tool-unavailable"));
        let title = label_with_id(i18n, "conversation-tool-unresolved", &id_label);
        let kinds = unresolved_kind_labels(self.unresolved.entry_kinds, i18n).join(" · ");
        let identity = self
            .unresolved
            .outer_invocation_id
            .clone()
            .unwrap_or_else(|| self.unresolved.anchor_entry_id.clone());

        v_flex()
            .id(format!("unresolved-tool-{identity}-root"))
            .w_full()
            .min_w_0()
            .gap_1()
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().warning.opacity(0.55))
            .bg(cx.theme().warning.opacity(0.08))
            .px_2()
            .py_1()
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(Icon::new(IconName::CircleAlert).text_color(cx.theme().warning))
                    .child(
                        Label::new(title)
                            .text_xs()
                            .font_medium()
                            .text_color(cx.theme().foreground)
                            .truncate(),
                    ),
            )
            .child(
                Label::new(format!("{kinds} ×{}", self.unresolved.entry_count))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .when(
                self.unresolved
                    .outer_id_label
                    .as_ref()
                    .is_some_and(|label| label.truncated),
                |this| {
                    this.child(
                        Label::new(i18n.t("conversation-tool-preview-truncated"))
                            .text_xs()
                            .text_color(cx.theme().warning),
                    )
                },
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DisplayField {
    label: String,
    value: String,
}

fn metadata_display_fields(detail: &ToolInvocationDetail, i18n: &I18n) -> Vec<DisplayField> {
    let unavailable = i18n.t("conversation-tool-unavailable");
    vec![
        DisplayField {
            label: i18n.t("conversation-tool-field-model-name"),
            value: detail.runtime_tool_name.text.clone(),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-original-name"),
            value: detail.tool_name.text.clone(),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-namespace"),
            value: detail
                .namespace
                .as_ref()
                .map(|namespace| namespace.text.clone())
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-source"),
            value: i18n.t(source_key(detail.source_kind)),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-server"),
            value: detail
                .server_or_provider_id
                .as_ref()
                .map(|value| value.text.clone())
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-invocation-id"),
            value: detail.id_label.text.clone(),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-call-id"),
            value: detail.call_id.text.clone(),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-created-at"),
            value: format::timestamp_label(detail.created_at, i18n),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-started-at"),
            value: detail
                .started_at
                .map(|time| format::timestamp_label(time, i18n))
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-completed-at"),
            value: detail
                .completed_at
                .map(|time| format::timestamp_label(time, i18n))
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-field-updated-at"),
            value: format::timestamp_label(detail.updated_at, i18n),
        },
    ]
}

fn approval_display_fields(approval: &ToolApprovalPreview, i18n: &I18n) -> Vec<DisplayField> {
    let unavailable = i18n.t("conversation-tool-unavailable");
    let decision = approval.decision.as_ref();
    vec![
        DisplayField {
            label: i18n.t("conversation-tool-field-approval"),
            value: i18n.t(approval_status_key(approval.status)),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-request-reason"),
            value: approval.request_reason.text.clone(),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-requested-at"),
            value: format::timestamp_label(approval.requested_at, i18n),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-decision"),
            value: decision
                .map(|decision| {
                    i18n.t(if decision.approved {
                        "conversation-tool-approval-approved"
                    } else {
                        "conversation-tool-approval-denied"
                    })
                })
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-decided-by"),
            value: decision
                .map(|decision| decision.decided_by.text.clone())
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-decision-reason"),
            value: decision
                .and_then(|decision| decision.reason.as_ref())
                .map(|reason| reason.text.clone())
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-decided-at"),
            value: approval
                .decided_at
                .map(|time| format::timestamp_label(time, i18n))
                .unwrap_or_else(|| unavailable.clone()),
        },
        DisplayField {
            label: i18n.t("conversation-tool-approval-expires-at"),
            value: approval
                .expires_at
                .map(|time| format::timestamp_label(time, i18n))
                .unwrap_or(unavailable),
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ErrorDisplay {
    Missing(DisplayField),
    Present(Vec<DisplayField>),
}

fn error_display(
    detail: &ToolInvocationDetail,
    preview: &ToolInvocationPreview,
    i18n: &I18n,
) -> Option<ErrorDisplay> {
    let unavailable = i18n.t("conversation-tool-unavailable");
    if let Some(error) = &preview.error {
        return Some(ErrorDisplay::Present(vec![
            DisplayField {
                label: i18n.t("conversation-tool-error-code"),
                value: error.code.text.clone(),
            },
            DisplayField {
                label: i18n.t("conversation-tool-error-message"),
                value: error.message.text.clone(),
            },
            DisplayField {
                label: i18n.t("conversation-tool-error-retryable"),
                value: i18n.t(if error.retryable {
                    "conversation-tool-value-yes"
                } else {
                    "conversation-tool-value-no"
                }),
            },
            DisplayField {
                label: i18n.t("conversation-tool-error-provider"),
                value: error
                    .provider
                    .as_ref()
                    .map(|provider| provider.text.clone())
                    .unwrap_or_else(|| unavailable.clone()),
            },
        ]));
    }
    matches!(
        detail.outcome,
        Some(
            ToolOutcomeSummary::Failed { .. }
                | ToolOutcomeSummary::Denied { .. }
                | ToolOutcomeSummary::Canceled { .. }
        )
    )
    .then(|| {
        ErrorDisplay::Missing(DisplayField {
            label: i18n.t("conversation-tool-field-error"),
            value: unavailable,
        })
    })
}

fn render_tool_invocation_content(
    detail: &ToolInvocationDetail,
    on_copy: OnCopy,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let i18n = cx.global::<I18n>();
    let unavailable = i18n.t("conversation-tool-unavailable");
    let mut content = v_flex().w_full().min_w_0().gap_2().px_1().pb_1();
    for field in metadata_display_fields(detail, i18n) {
        content = content.child(field_row(field.label, field.value, cx));
    }

    let Some(preview) = detail.preview.as_deref() else {
        return content
            .child(
                Label::new(unavailable)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element();
    };

    content = content.child(preview_section(
        i18n.t("conversation-tool-field-arguments"),
        preview.arguments.clone(),
        i18n,
        cx,
    ));
    if !preview.access_requests.is_empty() {
        content = content.child(render_access_requests(preview, i18n, cx));
    }
    if let Some(approval) = &preview.approval {
        content = content.child(render_approval(approval, i18n, cx));
    }
    if let Some(text) = &preview.text_output {
        content = content.child(preview_section(
            i18n.t("conversation-tool-field-text-output"),
            text.clone(),
            i18n,
            cx,
        ));
    }
    if let Some(structured) = &preview.structured_output {
        content = content.child(preview_section(
            i18n.t("conversation-tool-field-structured-output"),
            structured.clone(),
            i18n,
            cx,
        ));
    }
    if let Some(display) = error_display(detail, preview, i18n) {
        content = content.child(render_error(&display, preview.error.as_ref(), i18n, cx));
    }
    if preview.provider_raw_hidden {
        content = content.child(
            Label::new(i18n.t("conversation-tool-raw-hidden"))
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        );
    }

    let copy_text = tool_invocation_copy_text(detail, preview, i18n);
    let copy_tooltip = label_with_name(
        i18n,
        "conversation-tool-copy-preview",
        &detail.runtime_tool_name.text,
    );
    content
        .child(h_flex().justify_end().child(CopyButton::new(
            tool_invocation_copy_button_id(&detail.id),
            copy_text,
            on_copy,
            copy_tooltip,
            i18n.t("conversation-copy-success"),
            window,
            cx,
        )))
        .into_any_element()
}

fn tool_invocation_copy_button_id(id: &ToolInvocationId) -> String {
    format!("tool-invocation-{id}-copy")
}

fn field_row(label: String, value: String, cx: &App) -> AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .gap_2()
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(div().min_w_0().flex_1().child(value))
        .into_any_element()
}

fn preview_section(label: String, preview: BoundedPreview, i18n: &I18n, cx: &App) -> AnyElement {
    let truncated = preview.truncated;
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .child(
            Label::new(label)
                .text_xs()
                .font_medium()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .rounded(px(6.))
                .bg(cx.theme().tokens.muted.background.opacity(0.5))
                .px_2()
                .py_1()
                .child(LiteralPreview::new(preview)),
        )
        .when(truncated, |this| {
            this.child(
                Label::new(i18n.t("conversation-tool-preview-truncated"))
                    .text_xs()
                    .text_color(cx.theme().warning),
            )
        })
        .into_any_element()
}

fn render_access_requests(preview: &ToolInvocationPreview, i18n: &I18n, cx: &App) -> AnyElement {
    let mut section = v_flex().w_full().min_w_0().gap_1().child(
        Label::new(i18n.t("conversation-tool-field-access"))
            .text_xs()
            .font_medium()
            .text_color(cx.theme().muted_foreground),
    );
    for request in &preview.access_requests {
        let kind = i18n.t(match request.kind {
            ToolAccessKind::Read => "conversation-tool-access-kind-read",
            ToolAccessKind::Write => "conversation-tool-access-kind-write",
            ToolAccessKind::Execute => "conversation-tool-access-kind-execute",
            ToolAccessKind::Network => "conversation-tool-access-kind-network",
        });
        let mut request_view = v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .rounded(px(6.))
            .bg(cx.theme().tokens.muted.background.opacity(0.5))
            .px_2()
            .py_1()
            .child(Label::new(kind).text_xs().font_medium())
            .child(field_row(
                i18n.t("conversation-tool-access-target"),
                request.target.text.clone(),
                cx,
            ));
        if let Some(path) = &request.normalized_path {
            request_view = request_view.child(field_row(
                i18n.t("conversation-tool-access-normalized-path"),
                path.text.clone(),
                cx,
            ));
        }
        request_view = request_view.child(field_row(
            i18n.t("conversation-tool-access-within-project"),
            i18n.t(if request.within_project {
                "conversation-tool-value-yes"
            } else {
                "conversation-tool-value-no"
            }),
            cx,
        ));
        if let Some(reason) = &request.reason_key {
            request_view = request_view.child(field_row(
                i18n.t("conversation-tool-access-reason-key"),
                reason.text.clone(),
                cx,
            ));
        }
        let request_truncated = request.target.truncated
            || request
                .normalized_path
                .as_ref()
                .is_some_and(|value| value.truncated)
            || request
                .reason_key
                .as_ref()
                .is_some_and(|value| value.truncated);
        request_view = request_view.when(request_truncated, |this| {
            this.child(
                Label::new(i18n.t("conversation-tool-preview-truncated"))
                    .text_xs()
                    .text_color(cx.theme().warning),
            )
        });
        section = section.child(request_view);
    }
    section
        .when(preview.access_requests_truncated, |this| {
            this.child(
                Label::new(i18n.t("conversation-tool-preview-truncated"))
                    .text_xs()
                    .text_color(cx.theme().warning),
            )
        })
        .into_any_element()
}

fn render_approval(approval: &ToolApprovalPreview, i18n: &I18n, cx: &App) -> AnyElement {
    let mut section = v_flex().w_full().min_w_0().gap_1().child(
        Label::new(i18n.t("conversation-tool-field-approval"))
            .text_xs()
            .font_medium()
            .text_color(cx.theme().muted_foreground),
    );
    for field in approval_display_fields(approval, i18n) {
        section = section.child(field_row(field.label, field.value, cx));
    }
    section.into_any_element()
}

fn render_error(
    display: &ErrorDisplay,
    error: Option<&ToolErrorPreview>,
    i18n: &I18n,
    cx: &App,
) -> AnyElement {
    if let ErrorDisplay::Missing(field) = display {
        return field_row(field.label.clone(), field.value.clone(), cx);
    }
    let mut section = v_flex().w_full().min_w_0().gap_1().child(
        Label::new(i18n.t("conversation-tool-field-error"))
            .text_xs()
            .font_medium()
            .text_color(cx.theme().danger),
    );
    let ErrorDisplay::Present(fields) = display else {
        unreachable!("missing error display returned above")
    };
    for field in fields {
        section = section.child(field_row(field.label.clone(), field.value.clone(), cx));
    }
    let error = error.expect("present error display requires persisted error preview");
    section
        .when(
            error.code.truncated
                || error.message.truncated
                || error.provider.as_ref().is_some_and(|value| value.truncated),
            |this| {
                this.child(
                    Label::new(i18n.t("conversation-tool-preview-truncated"))
                        .text_xs()
                        .text_color(cx.theme().warning),
                )
            },
        )
        .into_any_element()
}

fn approval_action_buttons(
    id: &ToolInvocationId,
    on_approval_decision: OnApprovalDecision,
    i18n: &I18n,
) -> AnyElement {
    let approve_id = id.clone();
    let deny_id = id.clone();
    let on_approve = on_approval_decision.clone();
    h_flex()
        .items_center()
        .gap_1()
        .child(
            Button::new(format!("tool-invocation-{id}-approve"))
                .small()
                .icon(IconName::ShieldCheck)
                .label(i18n.t("conversation-approval-approve"))
                .on_click(move |_, window, cx| {
                    on_approve(approve_id.clone(), true, window, cx);
                }),
        )
        .child(
            Button::new(format!("tool-invocation-{id}-deny"))
                .ghost()
                .small()
                .icon(IconName::ShieldAlert)
                .label(i18n.t("conversation-approval-deny"))
                .on_click(move |_, window, cx| {
                    on_approval_decision(deny_id.clone(), false, window, cx);
                }),
        )
        .into_any_element()
}

fn tool_invocation_copy_text(
    detail: &ToolInvocationDetail,
    preview: &ToolInvocationPreview,
    i18n: &I18n,
) -> String {
    let mut output = String::new();
    for field in metadata_display_fields(detail, i18n) {
        append_copy_field(&mut output, &field.label, &field.value);
    }
    append_copy_field(
        &mut output,
        &i18n.t("conversation-tool-field-arguments"),
        &preview.arguments.text,
    );
    for request in &preview.access_requests {
        append_copy_field(
            &mut output,
            &i18n.t("conversation-tool-field-access"),
            &i18n.t(match request.kind {
                ToolAccessKind::Read => "conversation-tool-access-kind-read",
                ToolAccessKind::Write => "conversation-tool-access-kind-write",
                ToolAccessKind::Execute => "conversation-tool-access-kind-execute",
                ToolAccessKind::Network => "conversation-tool-access-kind-network",
            }),
        );
        append_copy_field(
            &mut output,
            &i18n.t("conversation-tool-access-target"),
            &request.target.text,
        );
        if let Some(path) = &request.normalized_path {
            append_copy_field(
                &mut output,
                &i18n.t("conversation-tool-access-normalized-path"),
                &path.text,
            );
        }
        append_copy_field(
            &mut output,
            &i18n.t("conversation-tool-access-within-project"),
            &i18n.t(if request.within_project {
                "conversation-tool-value-yes"
            } else {
                "conversation-tool-value-no"
            }),
        );
        if let Some(reason) = &request.reason_key {
            append_copy_field(
                &mut output,
                &i18n.t("conversation-tool-access-reason-key"),
                &reason.text,
            );
        }
    }
    if let Some(approval) = &preview.approval {
        for field in approval_display_fields(approval, i18n) {
            append_copy_field(&mut output, &field.label, &field.value);
        }
    }
    if let Some(text) = &preview.text_output {
        append_copy_field(
            &mut output,
            &i18n.t("conversation-tool-field-text-output"),
            &text.text,
        );
    }
    if let Some(structured) = &preview.structured_output {
        append_copy_field(
            &mut output,
            &i18n.t("conversation-tool-field-structured-output"),
            &structured.text,
        );
    }
    if let Some(display) = error_display(detail, preview, i18n) {
        match display {
            ErrorDisplay::Missing(field) => {
                append_copy_field(&mut output, &field.label, &field.value);
            }
            ErrorDisplay::Present(fields) => {
                append_copy_field(&mut output, &i18n.t("conversation-tool-field-error"), "");
                for field in fields {
                    append_copy_field(&mut output, &field.label, &field.value);
                }
            }
        }
    }
    if preview.provider_raw_hidden {
        output.push('\n');
        output.push_str(&i18n.t("conversation-tool-raw-hidden"));
    }
    if tool_invocation_preview_is_truncated(detail, preview) {
        output.push('\n');
        output.push_str(&i18n.t("conversation-tool-preview-truncated"));
    }
    output
}

fn tool_invocation_preview_is_truncated(
    detail: &ToolInvocationDetail,
    preview: &ToolInvocationPreview,
) -> bool {
    detail.id_label.truncated
        || detail.call_id.truncated
        || detail.tool_name.truncated
        || detail.runtime_tool_name.truncated
        || detail
            .namespace
            .as_ref()
            .is_some_and(|value| value.truncated)
        || detail
            .server_or_provider_id
            .as_ref()
            .is_some_and(|value| value.truncated)
        || preview.arguments.truncated
        || preview.access_requests_truncated
        || preview.access_requests.iter().any(|request| {
            request.target.truncated
                || request
                    .normalized_path
                    .as_ref()
                    .is_some_and(|value| value.truncated)
                || request
                    .reason_key
                    .as_ref()
                    .is_some_and(|value| value.truncated)
        })
        || preview.approval.as_ref().is_some_and(|approval| {
            approval.request_reason.truncated
                || approval.decision.as_ref().is_some_and(|decision| {
                    decision.decided_by.truncated
                        || decision
                            .reason
                            .as_ref()
                            .is_some_and(|value| value.truncated)
                })
        })
        || preview
            .text_output
            .as_ref()
            .is_some_and(|value| value.truncated)
        || preview
            .structured_output
            .as_ref()
            .is_some_and(|value| value.truncated)
        || preview.error.as_ref().is_some_and(|error| {
            error.code.truncated
                || error.message.truncated
                || error.provider.as_ref().is_some_and(|value| value.truncated)
        })
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ToolInspectionEvidence {
    pub(super) summary: String,
    pub(super) detail: Vec<(String, String)>,
    pub(super) truncated: bool,
    pub(super) provider_raw_hidden: bool,
    pub(super) copy_text: String,
}

#[cfg(test)]
pub(super) fn tool_inspection_evidence_for_test(
    invocation: &ToolInvocation,
) -> ToolInspectionEvidence {
    let i18n = I18n::english_for_test();
    let preview = build_tool_invocation_preview(invocation);
    let detail = project_tool_invocation_detail(invocation, true, None, false);
    let outcome = detail
        .outcome
        .as_ref()
        .map(|outcome| outcome_label(outcome, &i18n))
        .unwrap_or_else(|| i18n.t(status_key(detail.status)));
    let summary = format!(
        "{} | {outcome} | {}",
        source_summary_label(&detail, &i18n),
        duration_summary_label(&detail, &i18n)
    );
    let metadata = metadata_display_fields(&detail, &i18n)
        .into_iter()
        .map(|field| (field.label, field.value))
        .collect();
    ToolInspectionEvidence {
        summary,
        detail: metadata,
        truncated: tool_invocation_preview_is_truncated(&detail, &preview),
        provider_raw_hidden: preview.provider_raw_hidden,
        copy_text: tool_invocation_copy_text(&detail, &preview, &i18n),
    }
}

fn append_copy_field(output: &mut String, label: &str, value: &str) {
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(label);
    output.push_str(": ");
    output.push_str(value);
}

fn source_key(source: ToolSourceKind) -> &'static str {
    match source {
        ToolSourceKind::Local => "conversation-tool-source-local",
        ToolSourceKind::Mcp => "conversation-tool-source-mcp",
        ToolSourceKind::ProviderHosted => "conversation-tool-source-provider-hosted",
    }
}

fn source_summary_label(detail: &ToolInvocationDetail, i18n: &I18n) -> String {
    let source = i18n.t(source_key(detail.source_kind));
    detail
        .server_or_provider_id
        .as_ref()
        .map(|id| format!("{source} · {}", id.text))
        .unwrap_or(source)
}

fn status_key(status: ToolInvocationStatus) -> &'static str {
    match status {
        ToolInvocationStatus::Requested => "conversation-tool-status-requested",
        ToolInvocationStatus::AwaitingApproval => "conversation-tool-status-awaiting-approval",
        ToolInvocationStatus::Running => "conversation-tool-status-running",
        ToolInvocationStatus::Succeeded => "conversation-tool-status-succeeded",
        ToolInvocationStatus::Failed => "conversation-tool-status-failed",
        ToolInvocationStatus::Denied => "conversation-tool-status-denied",
        ToolInvocationStatus::Canceled => "conversation-tool-status-canceled",
    }
}

fn approval_status_key(status: ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "conversation-tool-approval-pending",
        ApprovalStatus::Approved => "conversation-tool-approval-approved",
        ApprovalStatus::Denied => "conversation-tool-approval-denied",
        ApprovalStatus::Expired => "conversation-tool-approval-expired",
        ApprovalStatus::Canceled => "conversation-tool-approval-canceled",
    }
}

fn outcome_label(outcome: &ToolOutcomeSummary, i18n: &I18n) -> String {
    let (key, code) = match outcome {
        ToolOutcomeSummary::Succeeded => ("conversation-tool-status-succeeded", None),
        ToolOutcomeSummary::Failed { code } => ("conversation-tool-status-failed", code.as_ref()),
        ToolOutcomeSummary::Denied { code } => ("conversation-tool-status-denied", code.as_ref()),
        ToolOutcomeSummary::Canceled { code } => {
            ("conversation-tool-status-canceled", code.as_ref())
        }
    };
    let label = i18n.t(key);
    code.map(|code| format!("{label} ({})", code.text))
        .unwrap_or(label)
}

fn is_terminal_status(status: ToolInvocationStatus) -> bool {
    matches!(
        status,
        ToolInvocationStatus::Succeeded
            | ToolInvocationStatus::Failed
            | ToolInvocationStatus::Denied
            | ToolInvocationStatus::Canceled
    )
}

fn label_with_name(i18n: &I18n, key: &str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name);
    i18n.t_with_args(key, &args)
}

fn label_with_id(i18n: &I18n, key: &str, id: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("id", id);
    i18n.t_with_args(key, &args)
}

fn label_with_duration(i18n: &I18n, key: &str, duration: String) -> String {
    let mut args = FluentArgs::new();
    args.set("duration", duration);
    i18n.t_with_args(key, &args)
}

fn duration_label(duration: Duration) -> String {
    let milliseconds = duration.whole_milliseconds();
    if milliseconds == 0 {
        return "0s".to_string();
    }
    if milliseconds < 1_000 {
        return format!("{milliseconds}ms");
    }
    let total_seconds = milliseconds / 1_000;
    let fractional_milliseconds = milliseconds % 1_000;
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    let seconds = if fractional_milliseconds == 0 {
        format!("{seconds}s")
    } else {
        let fraction = format!("{fractional_milliseconds:03}")
            .trim_end_matches('0')
            .to_string();
        format!("{seconds}.{fraction}s")
    };
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}")
    } else {
        seconds
    }
}

fn duration_summary_label(detail: &ToolInvocationDetail, i18n: &I18n) -> String {
    let key = if is_terminal_status(detail.status) {
        "conversation-tool-duration"
    } else {
        "conversation-tool-duration-updated"
    };
    let duration = detail
        .persisted_duration()
        .map(duration_label)
        .unwrap_or_else(|| i18n.t("conversation-tool-unavailable"));
    label_with_duration(i18n, key, duration)
}

fn unresolved_kind_labels(kinds: ToolLifecycleEntryKinds, i18n: &I18n) -> Vec<String> {
    let mut labels = Vec::with_capacity(4);
    if kinds.tool_call {
        labels.push(label_with_name(i18n, "conversation-tool-call", ""));
    }
    if kinds.approval_request {
        labels.push(i18n.t("conversation-approval-request"));
    }
    if kinds.approval_decision {
        labels.push(i18n.t("conversation-tool-field-approval"));
    }
    if kinds.tool_result {
        labels.push(label_with_name(i18n, "conversation-tool-result", ""));
    }
    labels
}

#[derive(IntoElement)]
pub(super) struct LiteralPreview {
    preview: BoundedPreview,
}

impl LiteralPreview {
    pub(super) fn new(preview: BoundedPreview) -> Self {
        Self { preview }
    }
}

impl RenderOnce for LiteralPreview {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut root = div()
            .min_w_0()
            .text_xs()
            .font_family(cx.theme().mono_font_family.clone())
            .whitespace_normal();
        for line in literal_preview_lines(&self.preview.text) {
            root = root.child(div().min_w_0().child(line));
        }
        root
    }
}

fn literal_preview_lines(text: &str) -> impl Iterator<Item = SharedString> + '_ {
    text.split('\n')
        .map(|line| SharedString::from(line.to_owned()))
}

fn bounded_json_preview(value: &Value, limits: ToolPreviewLimits) -> BoundedPreview {
    let mut writer = BoundedWriter::new(limits);
    write_json_value(value, 0, &mut writer);
    writer.finish()
}

fn write_json_value(value: &Value, depth: usize, writer: &mut BoundedWriter) {
    if writer.stopped() {
        return;
    }
    if depth > writer.limits.max_depth || !writer.consume_node() {
        writer.truncate();
        return;
    }

    match value {
        Value::Null => writer.write_static("null"),
        Value::Bool(value) => writer.write_static(if *value { "true" } else { "false" }),
        Value::Number(value) => writer.write_static(&value.to_string()),
        Value::String(value) => writer.write_json_string(value),
        Value::Array(values) => {
            writer.write_static("[");
            for (index, value) in values.iter().enumerate() {
                if writer.stopped() {
                    break;
                }
                if index > 0 {
                    writer.write_static(",");
                }
                writer.write_newline_and_indent(depth + 1);
                write_json_value(value, depth + 1, writer);
            }
            if !values.is_empty() && !writer.stopped() {
                writer.write_newline_and_indent(depth);
            }
            writer.write_static("]");
        }
        Value::Object(values) => {
            writer.write_static("{");
            for (index, (key, value)) in values.iter().enumerate() {
                if writer.stopped() {
                    break;
                }
                if index > 0 {
                    writer.write_static(",");
                }
                writer.write_newline_and_indent(depth + 1);
                writer.write_json_string(key);
                writer.write_static(": ");
                write_json_value(value, depth + 1, writer);
            }
            if !values.is_empty() && !writer.stopped() {
                writer.write_newline_and_indent(depth);
            }
            writer.write_static("}");
        }
    }
}

fn bounded_text_preview(value: &str, limits: ToolPreviewLimits) -> BoundedPreview {
    let mut writer = BoundedWriter::new(limits);
    writer.write_text(value, true);
    writer.finish()
}

fn bounded_metadata_preview(value: &str, limits: ToolPreviewLimits) -> BoundedPreview {
    bounded_text_preview(value, limits)
}

fn bounded_content_parts(
    parts: &[ContentPart],
    limits: ToolPreviewLimits,
) -> Option<BoundedPreview> {
    let mut writer = BoundedWriter::new(limits);
    for (index, part) in parts.iter().enumerate() {
        if !writer.consume_node() {
            writer.truncate();
            break;
        }
        if index > 0 {
            writer.write_static("\n");
        }
        match part {
            ContentPart::Text { text } => writer.write_text(text, true),
            ContentPart::Image { attachment_id } => {
                write_content_part_metadata("image", attachment_id, &mut writer);
            }
            ContentPart::File { attachment_id } => {
                write_content_part_metadata("file", attachment_id, &mut writer);
            }
            ContentPart::Audio { attachment_id } => {
                write_content_part_metadata("audio", attachment_id, &mut writer);
            }
            ContentPart::Attachment { attachment_id } => {
                write_content_part_metadata("attachment", attachment_id, &mut writer);
            }
        }
        if writer.stopped() {
            break;
        }
    }
    (!parts.is_empty()).then(|| writer.finish())
}

fn write_content_part_metadata(
    kind: &'static str,
    attachment_id: &str,
    writer: &mut BoundedWriter,
) {
    writer.write_static("[");
    writer.write_static(kind);
    writer.write_static(" attachment_id=");
    writer.write_text(attachment_id, true);
    writer.write_static("]");
}

fn bounded_access_requests(
    requests: &[ToolAccessRequestPayload],
    limits: ToolPreviewLimits,
) -> (Vec<ToolAccessPreview>, bool) {
    let mut budget = CollectionBudget::new(limits);
    let mut previews = Vec::new();
    let mut truncated = false;

    for request in requests {
        if !budget.consume_node() {
            truncated = true;
            break;
        }
        let target = budget.take_string(&request.target);
        let normalized_path = request
            .normalized_path
            .as_deref()
            .map(|path| budget.take_string(path));
        let reason_key = request
            .reason_key
            .as_deref()
            .map(|reason| budget.take_string(reason));
        truncated |= target.truncated
            || normalized_path
                .as_ref()
                .is_some_and(|value| value.truncated)
            || reason_key.as_ref().is_some_and(|value| value.truncated);
        previews.push(ToolAccessPreview {
            kind: request.kind,
            target,
            normalized_path,
            within_project: request.within_project,
            reason_key,
        });
        if budget.exhausted() {
            truncated |= previews.len() < requests.len();
            break;
        }
    }

    let collection_truncated = truncated || previews.len() < requests.len();
    (previews, collection_truncated)
}

struct BoundedWriter {
    limits: ToolPreviewLimits,
    text: String,
    input_bytes: usize,
    nodes: usize,
    lines: usize,
    truncated: bool,
}

impl BoundedWriter {
    fn new(limits: ToolPreviewLimits) -> Self {
        Self {
            limits,
            text: String::with_capacity(limits.max_output_bytes.min(4_096)),
            input_bytes: 0,
            nodes: 0,
            lines: usize::from(limits.max_lines > 0),
            truncated: false,
        }
    }

    fn consume_node(&mut self) -> bool {
        if self.nodes >= self.limits.max_nodes {
            return false;
        }
        self.nodes += 1;
        true
    }

    fn stopped(&self) -> bool {
        self.truncated
    }

    fn truncate(&mut self) {
        self.truncated = true;
    }

    fn write_static(&mut self, value: &str) {
        for character in value.chars() {
            if !self.push_char(character) {
                break;
            }
        }
    }

    fn write_newline_and_indent(&mut self, depth: usize) {
        self.write_static("\n");
        for _ in 0..depth.saturating_mul(2) {
            if self.stopped() {
                return;
            }
            self.write_static(" ");
        }
    }

    fn write_text(&mut self, value: &str, enforce_string_limit: bool) {
        let mut string_bytes = 0usize;
        for character in value.chars() {
            let bytes = character.len_utf8();
            if (enforce_string_limit
                && string_bytes.saturating_add(bytes) > self.limits.max_string_bytes)
                || self.input_bytes.saturating_add(bytes) > self.limits.max_input_bytes
            {
                self.truncate();
                return;
            }
            string_bytes += bytes;
            self.input_bytes += bytes;
            if !self.push_char(character) {
                return;
            }
        }
    }

    fn write_json_string(&mut self, value: &str) {
        self.write_static("\"");
        let mut string_bytes = 0usize;
        for character in value.chars() {
            let bytes = character.len_utf8();
            if string_bytes.saturating_add(bytes) > self.limits.max_string_bytes
                || self.input_bytes.saturating_add(bytes) > self.limits.max_input_bytes
            {
                self.truncate();
                return;
            }
            string_bytes += bytes;
            self.input_bytes += bytes;
            match character {
                '"' => self.write_static("\\\""),
                '\\' => self.write_static("\\\\"),
                '\u{08}' => self.write_static("\\b"),
                '\u{0c}' => self.write_static("\\f"),
                '\n' => self.write_static("\\n"),
                '\r' => self.write_static("\\r"),
                '\t' => self.write_static("\\t"),
                character if character <= '\u{1f}' => {
                    self.write_static(&format!("\\u{:04x}", character as u32));
                }
                character => {
                    self.push_char(character);
                }
            }
            if self.stopped() {
                return;
            }
        }
        self.write_static("\"");
    }

    fn push_char(&mut self, character: char) -> bool {
        if self.truncated {
            return false;
        }
        if self.limits.max_lines == 0 {
            self.truncate();
            return false;
        }
        if character == '\n' && self.lines >= self.limits.max_lines {
            self.truncate();
            return false;
        }
        let reserved = TRUNCATION_MARKER.len();
        if self
            .text
            .len()
            .saturating_add(character.len_utf8())
            .saturating_add(reserved)
            > self.limits.max_output_bytes
        {
            self.truncate();
            return false;
        }
        if character == '\n' {
            self.lines += 1;
        }
        self.text.push(character);
        true
    }

    fn finish(mut self) -> BoundedPreview {
        if self.truncated
            && self.text.len().saturating_add(TRUNCATION_MARKER.len())
                <= self.limits.max_output_bytes
        {
            self.text.push_str(TRUNCATION_MARKER);
        }
        BoundedPreview {
            text: self.text,
            truncated: self.truncated,
        }
    }
}

struct CollectionBudget {
    limits: ToolPreviewLimits,
    input_bytes: usize,
    output_bytes: usize,
    nodes: usize,
    lines: usize,
}

impl CollectionBudget {
    fn new(limits: ToolPreviewLimits) -> Self {
        Self {
            limits,
            input_bytes: 0,
            output_bytes: 0,
            nodes: 0,
            lines: 0,
        }
    }

    fn consume_node(&mut self) -> bool {
        if self.nodes >= self.limits.max_nodes {
            return false;
        }
        self.nodes += 1;
        true
    }

    fn take_string(&mut self, value: &str) -> BoundedPreview {
        let mut text = String::new();
        let mut string_bytes = 0usize;
        let mut value_lines = 0usize;
        let mut truncated = false;
        for character in value.chars() {
            let bytes = character.len_utf8();
            let added_lines = usize::from(text.is_empty()) + usize::from(character == '\n');
            if string_bytes.saturating_add(bytes) > self.limits.max_string_bytes
                || self.input_bytes.saturating_add(bytes) > self.limits.max_input_bytes
                || self
                    .output_bytes
                    .saturating_add(text.len())
                    .saturating_add(bytes)
                    .saturating_add(TRUNCATION_MARKER.len())
                    > self.limits.max_output_bytes
                || self
                    .lines
                    .saturating_add(value_lines)
                    .saturating_add(added_lines)
                    > self.limits.max_lines
            {
                truncated = true;
                break;
            }
            string_bytes += bytes;
            self.input_bytes += bytes;
            value_lines += added_lines;
            text.push(character);
        }
        if truncated
            && self
                .output_bytes
                .saturating_add(text.len())
                .saturating_add(TRUNCATION_MARKER.len())
                <= self.limits.max_output_bytes
        {
            text.push_str(TRUNCATION_MARKER);
        }
        self.output_bytes += text.len();
        self.lines += value_lines;
        BoundedPreview { text, truncated }
    }

    fn exhausted(&self) -> bool {
        self.nodes >= self.limits.max_nodes
            || self.input_bytes >= self.limits.max_input_bytes
            || self
                .limits
                .max_output_bytes
                .saturating_sub(self.output_bytes)
                <= TRUNCATION_MARKER.len()
            || self.lines >= self.limits.max_lines
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use gpui::{Context, Entity, EntityId, IntoElement, Render, TestAppContext, View, Window, div};
    use jaco_core::{
        ApprovalDecisionPayload, ApprovalRequestPayload, ConversationEntryStatus,
        ProviderRawPayload, RunErrorPayload, StructuredOutput, ToolApprovalPolicy, ToolArguments,
        ToolExecutionPolicy, ToolInvocationApproval, ToolInvocationInput, ToolInvocationOutput,
    };
    use serde_json::json;

    struct CopyStateSnapshot {
        invocation: Option<EntityId>,
        sibling: Option<EntityId>,
    }

    struct CopyStateRoot {
        revision: usize,
        snapshots: Rc<RefCell<[Option<CopyStateSnapshot>; 2]>>,
    }

    impl Render for CopyStateRoot {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            CopyStateView {
                state: cx.entity().clone(),
            }
        }
    }

    #[derive(IntoElement)]
    struct CopyStateView {
        state: Entity<CopyStateRoot>,
    }

    impl View for CopyStateView {
        fn entity_id(&self) -> Option<EntityId> {
            Some(self.state.entity_id())
        }

        fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
            let (revision, snapshots) = {
                let state = self.state.read(cx);
                (state.revision, state.snapshots.clone())
            };
            let on_copy: OnCopy = Rc::new(|_, _, _| true);
            let invocation_id = ToolInvocationId::from("invocation-a");
            let sibling_id = ToolInvocationId::from("invocation-b");
            let invocation = CopyButton::new(
                tool_invocation_copy_button_id(&invocation_id),
                "invocation-a".to_string(),
                on_copy.clone(),
                "copy".to_string(),
                "copied".to_string(),
                window,
                cx,
            );
            let sibling = CopyButton::new(
                tool_invocation_copy_button_id(&sibling_id),
                "invocation-b".to_string(),
                on_copy,
                "copy".to_string(),
                "copied".to_string(),
                window,
                cx,
            );
            snapshots.borrow_mut()[revision] = Some(CopyStateSnapshot {
                invocation: invocation.entity_id(),
                sibling: sibling.entity_id(),
            });
            div().child(invocation).child(sibling)
        }
    }

    #[gpui::test]
    fn invocation_id_keyed_copy_state_survives_rebuild_and_isolated_from_sibling(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_component::init);
        let snapshots = Rc::new(RefCell::new([None, None]));
        let (root, cx) = cx.add_window_view(|_, _| CopyStateRoot {
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
        let initial = snapshots[0].as_ref().expect("initial copy buttons");
        let rebuilt = snapshots[1].as_ref().expect("rebuilt copy buttons");
        assert_eq!(initial.invocation, rebuilt.invocation);
        assert_eq!(initial.sibling, rebuilt.sibling);
        assert_ne!(initial.invocation, initial.sibling);
    }

    #[test]
    fn project_agent_details_groups_strictly_by_outer_id() {
        let invocation_a = invocation("invocation-a", "same-name", ToolInvocationStatus::Running);
        let mut invocation_b =
            invocation("invocation-b", "same-name", ToolInvocationStatus::Succeeded);
        invocation_b.created_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);
        let entries = vec![
            lifecycle_entry("call-a", 1, Some("invocation-a"), "inner-mismatch", false),
            lifecycle_entry("result-a", 2, Some("invocation-a"), "inner-other", true),
            lifecycle_entry("call-missing", 3, Some("missing"), "invocation-a", false),
            lifecycle_entry("result-missing", 4, Some("missing"), "invocation-b", true),
            lifecycle_entry("missing-outer", 5, None, "invocation-a", false),
        ];

        let items = project_agent_details(
            &entries,
            &[invocation_b, invocation_a],
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(items.len(), 4);
        assert!(matches!(
            &items[0],
            AgentDetailItem::ToolInvocation(detail) if detail.id == "invocation-a"
        ));
        assert!(matches!(
            &items[1],
            AgentDetailItem::UnresolvedToolLifecycle(unresolved)
                if unresolved.outer_invocation_id.as_deref() == Some("missing")
                    && unresolved.entry_count == 2
                    && unresolved.entry_kinds.tool_call
                    && unresolved.entry_kinds.tool_result
        ));
        assert!(matches!(
            &items[2],
            AgentDetailItem::UnresolvedToolLifecycle(unresolved)
                if unresolved.outer_invocation_id.is_none() && unresolved.entry_count == 1
        ));
        assert!(matches!(
            &items[3],
            AgentDetailItem::ToolInvocation(detail) if detail.id == "invocation-b"
        ));
    }

    #[test]
    fn unresolved_outer_id_and_kinds_are_bounded() {
        let outer_id = "x".repeat(TOOL_PREVIEW_LIMITS.max_string_bytes * 2);
        let entries = (0..10_000)
            .map(|index| {
                lifecycle_entry(
                    &format!("entry-{index}"),
                    index,
                    Some(&outer_id),
                    "ignored-inner-id",
                    index % 2 == 0,
                )
            })
            .collect::<Vec<_>>();
        let items = project_agent_details(
            &entries,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(items.len(), 1);
        let AgentDetailItem::UnresolvedToolLifecycle(unresolved) = &items[0] else {
            panic!("missing record must produce an unresolved lifecycle item");
        };
        assert_eq!(unresolved.entry_count, 10_000);
        assert!(unresolved.entry_kinds.tool_call);
        assert!(unresolved.entry_kinds.tool_result);
        let label = unresolved.outer_id_label.as_ref().unwrap();
        assert!(label.truncated);
        assert!(label.text.len() <= TOOL_PREVIEW_LIMITS.max_output_bytes);
        assert!(std::str::from_utf8(label.text.as_bytes()).is_ok());
    }

    #[test]
    fn unresolved_title_supplies_id_in_both_runtime_locales() {
        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            let title = label_with_id(&i18n, "conversation-tool-unresolved", "invocation-1");
            assert!(title.contains("invocation-1"), "locale {locale}: {title}");
            assert_ne!(title, "conversation-tool-unresolved");
        }
    }

    #[test]
    fn projection_covers_sources_statuses_duration_and_approval_authority() {
        let statuses = [
            ToolInvocationStatus::Requested,
            ToolInvocationStatus::AwaitingApproval,
            ToolInvocationStatus::Running,
            ToolInvocationStatus::Succeeded,
            ToolInvocationStatus::Failed,
            ToolInvocationStatus::Denied,
            ToolInvocationStatus::Canceled,
        ];
        for status in statuses {
            let mut invocation = invocation("invocation", "tool", status);
            invocation.started_at = Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(1));
            invocation.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(4);
            if matches!(
                status,
                ToolInvocationStatus::Succeeded
                    | ToolInvocationStatus::Failed
                    | ToolInvocationStatus::Denied
                    | ToolInvocationStatus::Canceled
            ) {
                invocation.completed_at = Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(3));
            }
            if status == ToolInvocationStatus::AwaitingApproval {
                invocation.approval = Some(approval(ApprovalStatus::Pending));
            }
            let detail = project_tool_invocation_detail(&invocation, false, None, true);
            assert_eq!(
                detail.approval_decidable,
                status == ToolInvocationStatus::AwaitingApproval
            );
            assert_eq!(
                detail.persisted_duration(),
                Some(if detail.completed_at.is_some() {
                    Duration::seconds(2)
                } else {
                    Duration::seconds(3)
                })
            );
        }

        let mut mcp = invocation("mcp", "tool", ToolInvocationStatus::Running);
        mcp.source = ToolSource::Mcp {
            server_id: "source-server".to_string(),
        };
        mcp.server_id = Some("record-server".to_string());
        let detail = project_tool_invocation_detail(&mcp, false, None, false);
        assert_eq!(detail.source_kind, ToolSourceKind::Mcp);
        assert_eq!(
            source_summary_label(&detail, &I18n::english_for_test()),
            "MCP · record-server"
        );
        assert_eq!(detail.server_or_provider_id.unwrap().text, "record-server");

        let mut hosted = invocation("hosted", "tool", ToolInvocationStatus::Running);
        hosted.source = ToolSource::ProviderHosted {
            provider_id: "provider-1".to_string(),
        };
        let detail = project_tool_invocation_detail(&hosted, false, None, false);
        assert_eq!(detail.source_kind, ToolSourceKind::ProviderHosted);
        assert_eq!(
            source_summary_label(&detail, &I18n::english_for_test()),
            "Provider hosted · provider-1"
        );
        assert_eq!(detail.server_or_provider_id.unwrap().text, "provider-1");

        let local = invocation("local", "tool", ToolInvocationStatus::Running);
        let detail = project_tool_invocation_detail(&local, false, None, false);
        assert_eq!(
            source_summary_label(&detail, &I18n::english_for_test()),
            "Local"
        );
    }

    #[test]
    fn duration_summary_is_explicit_accurate_and_stable() {
        let i18n = I18n::english_for_test();
        let mut invocation = invocation("duration", "tool", ToolInvocationStatus::Running);
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(detail.persisted_duration(), None);
        assert_eq!(
            duration_summary_label(&detail, &i18n),
            "Updated after Unavailable"
        );

        invocation.started_at = Some(OffsetDateTime::UNIX_EPOCH);
        invocation.updated_at = OffsetDateTime::UNIX_EPOCH;
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(duration_summary_label(&detail, &i18n), "Updated after 0s");

        invocation.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(500);
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(
            duration_summary_label(&detail, &i18n),
            "Updated after 500ms"
        );

        invocation.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(1_500);
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(duration_summary_label(&detail, &i18n), "Updated after 1.5s");

        invocation.started_at = Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(2));
        invocation.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(detail.persisted_duration(), None);
        assert_eq!(
            duration_summary_label(&detail, &i18n),
            "Updated after Unavailable"
        );

        invocation.status = ToolInvocationStatus::Succeeded;
        invocation.started_at = Some(OffsetDateTime::UNIX_EPOCH);
        invocation.completed_at = None;
        let detail = project_tool_invocation_detail(&invocation, false, None, false);
        assert_eq!(detail.persisted_duration(), None);
        assert_eq!(
            duration_summary_label(&detail, &i18n),
            "Completed in Unavailable"
        );
    }

    #[test]
    fn canonical_credential_shaped_strings_are_preserved_within_budget() {
        let canonical =
            "Authorization: Bearer synthetic-token https://user:pass@example.test /tmp/key sk-test";
        let preview = bounded_text_preview(canonical, TOOL_PREVIEW_LIMITS);
        assert_eq!(preview.text, canonical);
        assert!(!preview.truncated);

        let json = json!({
            "api_key": canonical,
            canonical: "secret-shaped-value",
        });
        let preview = bounded_json_preview(&json, TOOL_PREVIEW_LIMITS);
        assert!(preview.text.contains(canonical));
        assert!(preview.text.contains("secret-shaped-value"));
        assert!(!preview.text.contains("[REDACTED]"));
    }

    #[test]
    fn json_text_and_access_previews_obey_all_hard_limits() {
        let deep = (0..64).fold(
            json!("终".repeat(20_000)),
            |value, index| json!({ format!("key-{index}-{}", "宽".repeat(1_000)): value }),
        );
        let preview = bounded_json_preview(&deep, TOOL_PREVIEW_LIMITS);
        assert_preview_is_bounded(&preview, TOOL_PREVIEW_LIMITS);
        assert!(preview.truncated);

        let wide = Value::Array(
            (0..TOOL_PREVIEW_LIMITS.max_nodes)
                .map(|_| Value::String("值".repeat(TOOL_PREVIEW_LIMITS.max_string_bytes / 3)))
                .collect(),
        );
        let preview = bounded_json_preview(&wide, TOOL_PREVIEW_LIMITS);
        assert_preview_is_bounded(&preview, TOOL_PREVIEW_LIMITS);
        assert!(preview.truncated);

        let many_lines = (0..10_000)
            .map(|index| format!("第{index}行"))
            .collect::<Vec<_>>()
            .join("\n");
        let preview = bounded_text_preview(&many_lines, TOOL_PREVIEW_LIMITS);
        assert_preview_is_bounded(&preview, TOOL_PREVIEW_LIMITS);
        assert!(preview.truncated);

        let requests = (0..10_000)
            .map(|index| ToolAccessRequestPayload {
                kind: ToolAccessKind::Read,
                target: format!("/synthetic/{index}/{}", "路".repeat(1_000)),
                normalized_path: Some(format!("/normalized/{index}")),
                within_project: index % 2 == 0,
                reason_key: Some(format!("reason-{index}")),
            })
            .collect::<Vec<_>>();
        let (previews, truncated) = bounded_access_requests(&requests, TOOL_PREVIEW_LIMITS);
        assert!(truncated);
        assert!(previews.len() <= TOOL_PREVIEW_LIMITS.max_nodes);
        assert!(
            previews
                .iter()
                .flat_map(|preview| [
                    Some(&preview.target),
                    preview.normalized_path.as_ref(),
                    preview.reason_key.as_ref(),
                ])
                .flatten()
                .map(|preview| preview.text.len())
                .sum::<usize>()
                <= TOOL_PREVIEW_LIMITS.max_output_bytes
        );

        let input_limited = ToolPreviewLimits {
            max_input_bytes: 5,
            max_string_bytes: 100,
            max_output_bytes: 100,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_text_preview("ab界cd", input_limited);
        assert_eq!(preview.text, "ab界[TRUNCATED]");
        assert!(preview.truncated);

        let line_limited = ToolPreviewLimits {
            max_lines: 2,
            max_string_bytes: 100,
            max_output_bytes: 100,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_text_preview("first\nsecond\nthird", line_limited);
        assert_eq!(preview.text, "first\nsecond[TRUNCATED]");
        assert!(preview.truncated);

        let node_limited = ToolPreviewLimits {
            max_nodes: 3,
            max_string_bytes: 100,
            max_output_bytes: 100,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_json_preview(&json!([1, 2, 3, 4]), node_limited);
        assert!(preview.truncated);
        assert_preview_is_bounded(&preview, node_limited);

        let depth_limited = ToolPreviewLimits {
            max_depth: 1,
            max_string_bytes: 100,
            max_output_bytes: 100,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_json_preview(&json!({ "a": { "b": 1 } }), depth_limited);
        assert!(preview.truncated);
        assert_preview_is_bounded(&preview, depth_limited);
    }

    #[test]
    fn content_parts_are_streamed_and_do_not_activate_markup() {
        let literal = "![image](file:///tmp/synthetic) <img src='https://example.test'> [link](https://example.test) ```nested```";
        let parts = (0..10_000)
            .map(|_| ContentPart::Text {
                text: literal.to_string(),
            })
            .collect::<Vec<_>>();
        let preview = bounded_content_parts(&parts, TOOL_PREVIEW_LIMITS).unwrap();
        assert_preview_is_bounded(&preview, TOOL_PREVIEW_LIMITS);
        assert!(preview.text.starts_with(literal));
        assert!(preview.truncated);
        assert!(preview.text.ends_with(TRUNCATION_MARKER));

        let mixed = vec![
            ContentPart::Text {
                text: "plain text".to_string(),
            },
            ContentPart::Image {
                attachment_id: "image-1".to_string(),
            },
            ContentPart::File {
                attachment_id: "file-1".to_string(),
            },
        ];
        let preview = bounded_content_parts(&mixed, TOOL_PREVIEW_LIMITS).unwrap();
        assert_eq!(
            preview.text,
            "plain text\n[image attachment_id=image-1]\n[file attachment_id=file-1]"
        );
        assert!(!preview.truncated);

        let non_text = vec![
            ContentPart::Audio {
                attachment_id: "audio-1".to_string(),
            },
            ContentPart::Attachment {
                attachment_id: "attachment-1".to_string(),
            },
        ];
        let preview = bounded_content_parts(&non_text, TOOL_PREVIEW_LIMITS).unwrap();
        assert_eq!(
            preview.text,
            "[audio attachment_id=audio-1]\n[attachment attachment_id=attachment-1]"
        );
        assert!(!preview.truncated);

        let node_limited = ToolPreviewLimits {
            max_nodes: 1,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_content_parts(&mixed, node_limited).unwrap();
        assert_eq!(preview.text, "plain text[TRUNCATED]");
        assert!(preview.truncated);

        let no_nodes = ToolPreviewLimits {
            max_nodes: 0,
            ..TOOL_PREVIEW_LIMITS
        };
        let preview = bounded_content_parts(&non_text, no_nodes).unwrap();
        assert_eq!(preview.text, TRUNCATION_MARKER);
        assert!(preview.truncated);
    }

    #[test]
    fn literal_preview_keeps_markdown_links_images_and_uris_as_plain_lines() {
        let literal = "![image](file:///tmp/synthetic)\n[link](https://example.test)\n<img src=\"https://example.test/image.png\">";
        let lines = literal_preview_lines(literal)
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            lines,
            literal.split('\n').map(str::to_string).collect::<Vec<_>>()
        );
        assert!(lines.iter().all(|line| {
            line.contains("file://") || line.contains("https://") || line.contains("<img")
        }));
    }

    #[test]
    fn builder_omits_provider_raw_and_duplicate_approval_arguments() {
        let mut invocation = invocation("invocation", "tool", ToolInvocationStatus::Failed);
        invocation.approval = Some(approval(ApprovalStatus::Denied));
        invocation.output = Some(ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: "visible output".to_string(),
            }],
            structured_output: Some(StructuredOutput {
                value: json!({ "visible": true }),
            }),
            raw_output: Some(ProviderRawPayload {
                provider_kind: "synthetic".to_string(),
                value: json!({ "provider_secret": "raw-output-must-not-appear" }),
            }),
            is_error: true,
        });
        invocation.error = Some(RunErrorPayload {
            code: "synthetic_error".to_string(),
            message: "visible error".to_string(),
            retryable: false,
            provider: Some("provider".to_string()),
            raw: Some(ProviderRawPayload {
                provider_kind: "synthetic".to_string(),
                value: json!({ "provider_secret": "raw-error-must-not-appear" }),
            }),
        });

        let preview = build_tool_invocation_preview(&invocation);
        let debug_preview = format!("{preview:?}");
        assert!(preview.provider_raw_hidden);
        assert!(!debug_preview.contains("raw-output-must-not-appear"));
        assert!(!debug_preview.contains("raw-error-must-not-appear"));
        assert!(!debug_preview.contains("duplicate arguments must be omitted"));
        assert_eq!(preview.text_output.unwrap().text, "visible output");
        assert!(
            preview
                .structured_output
                .unwrap()
                .text
                .contains("\"visible\": true")
        );
        assert_eq!(preview.error.unwrap().message.text, "visible error");
        assert_eq!(
            preview.approval.unwrap().request_reason.text,
            "approval reason"
        );

        let preview = build_tool_invocation_preview(&invocation);
        let detail = project_tool_invocation_detail(
            &invocation,
            true,
            Some(Arc::new(preview.clone())),
            false,
        );
        let copy = tool_invocation_copy_text(&detail, &preview, &I18n::english_for_test());
        assert!(copy.contains("visible output"));
        assert!(copy.contains("visible error"));
        assert!(copy.contains("Provider raw payload is hidden"));
        assert!(!copy.contains("raw-output-must-not-appear"));
        assert!(!copy.contains("raw-error-must-not-appear"));
        assert!(!copy.contains("duplicate arguments must be omitted"));
    }

    #[test]
    fn missing_and_present_fields_keep_visible_copy_parity() {
        let i18n = I18n::english_for_test();
        let mut pending = invocation("pending", "tool", ToolInvocationStatus::AwaitingApproval);
        let mut pending_approval = approval(ApprovalStatus::Pending);
        pending_approval.decision = None;
        pending_approval.decided_at = None;
        pending_approval.expires_at = None;
        pending.approval = Some(pending_approval);
        let preview = build_tool_invocation_preview(&pending);
        let detail = project_tool_invocation_detail(&pending, true, None, false);
        let copy = tool_invocation_copy_text(&detail, &preview, &i18n);
        assert_copy_contains_fields(&copy, &metadata_display_fields(&detail, &i18n));
        let approval_fields = approval_display_fields(preview.approval.as_ref().unwrap(), &i18n);
        assert_copy_contains_fields(&copy, &approval_fields);
        for key in [
            "conversation-tool-approval-decision",
            "conversation-tool-approval-decided-by",
            "conversation-tool-approval-decision-reason",
            "conversation-tool-approval-decided-at",
            "conversation-tool-approval-expires-at",
        ] {
            assert!(copy.contains(&format!("{}: Unavailable", i18n.t(key))));
        }
        assert!(copy.contains("Server or provider: Unavailable"));
        assert!(copy.contains("Namespace: Unavailable"));
        assert!(copy.contains("Started: Unavailable"));
        assert!(copy.contains("Completed: Unavailable"));

        for status in [
            ToolInvocationStatus::Failed,
            ToolInvocationStatus::Denied,
            ToolInvocationStatus::Canceled,
        ] {
            let invocation = invocation("missing-error", "tool", status);
            let preview = build_tool_invocation_preview(&invocation);
            let detail = project_tool_invocation_detail(&invocation, true, None, false);
            let display = error_display(&detail, &preview, &i18n);
            assert_eq!(
                display,
                Some(ErrorDisplay::Missing(DisplayField {
                    label: "Error".to_string(),
                    value: "Unavailable".to_string(),
                }))
            );
            assert!(
                tool_invocation_copy_text(&detail, &preview, &i18n).contains("Error: Unavailable")
            );
        }

        let succeeded = invocation("success", "tool", ToolInvocationStatus::Succeeded);
        let preview = build_tool_invocation_preview(&succeeded);
        let detail = project_tool_invocation_detail(&succeeded, true, None, false);
        assert_eq!(error_display(&detail, &preview, &i18n), None);

        let mut failed = invocation("failed", "tool", ToolInvocationStatus::Failed);
        failed.error = Some(RunErrorPayload {
            code: "synthetic_error".to_string(),
            message: "visible message".to_string(),
            retryable: true,
            provider: None,
            raw: None,
        });
        let preview = build_tool_invocation_preview(&failed);
        let detail = project_tool_invocation_detail(&failed, true, None, false);
        let ErrorDisplay::Present(error_fields) = error_display(&detail, &preview, &i18n).unwrap()
        else {
            panic!("persisted error must project labeled fields");
        };
        let copy = tool_invocation_copy_text(&detail, &preview, &i18n);
        assert_copy_contains_fields(&copy, &error_fields);
        assert!(copy.contains("Error code: synthetic_error"));
        assert!(copy.contains("Error message: visible message"));
        assert!(copy.contains("Retryable: Yes"));
        assert!(copy.contains("Provider: Unavailable"));
    }

    #[test]
    fn cache_is_only_attached_when_expanded_and_revision_matches() {
        let invocation = invocation("invocation", "tool", ToolInvocationStatus::Running);
        let preview = Arc::new(build_tool_invocation_preview(&invocation));
        let mut expanded = HashMap::new();
        expanded.insert(invocation.id.clone(), true);
        let mut previews = HashMap::new();
        previews.insert(
            invocation.id.clone(),
            ToolInvocationPreviewCacheEntry {
                revision: invocation.updated_at,
                preview: preview.clone(),
            },
        );

        let detail =
            detail_from_projection_state(&invocation, &expanded, &previews, &HashSet::new());
        assert!(Arc::ptr_eq(detail.preview.as_ref().unwrap(), &preview));

        previews.get_mut(&invocation.id).unwrap().revision += Duration::seconds(1);
        let detail =
            detail_from_projection_state(&invocation, &expanded, &previews, &HashSet::new());
        assert!(detail.preview.is_none());
    }

    fn invocation(id: &str, tool_name: &str, status: ToolInvocationStatus) -> ToolInvocation {
        ToolInvocation {
            id: id.to_string(),
            agent_run_id: "run-1".to_string(),
            provider_step_id: Some("step-1".to_string()),
            call_id: format!("call-{id}"),
            source: ToolSource::Local,
            namespace: None,
            server_id: None,
            tool_name: tool_name.to_string(),
            runtime_tool_name: tool_name.to_string(),
            status,
            input: ToolInvocationInput {
                source: ToolSource::Local,
                namespace: None,
                tool_name: tool_name.to_string(),
                runtime_tool_name: tool_name.to_string(),
                call_id: format!("call-{id}"),
                arguments: ToolArguments {
                    value: json!({ "input": "value" }),
                },
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
            },
            output: None,
            error: None,
            approval: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            started_at: None,
            completed_at: None,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn approval(status: ApprovalStatus) -> ToolInvocationApproval {
        ToolInvocationApproval {
            status,
            request: ApprovalRequestPayload {
                reason: "approval reason".to_string(),
                tool_source: ToolSource::Local,
                tool_name: "tool".to_string(),
                arguments_preview: "duplicate arguments must be omitted".to_string(),
                access_requests: vec![ToolAccessRequestPayload {
                    kind: ToolAccessKind::Write,
                    target: "/synthetic/project/file".to_string(),
                    normalized_path: Some("/synthetic/project/file".to_string()),
                    within_project: true,
                    reason_key: Some("write".to_string()),
                }],
            },
            decision: Some(ApprovalDecisionPayload {
                approved: status == ApprovalStatus::Approved,
                decided_by: "synthetic-user".to_string(),
                reason: Some("decision reason".to_string()),
            }),
            requested_at: OffsetDateTime::UNIX_EPOCH,
            decided_at: Some(OffsetDateTime::UNIX_EPOCH),
            expires_at: None,
        }
    }

    fn lifecycle_entry(
        id: &str,
        seq: i32,
        outer_id: Option<&str>,
        inner_id: &str,
        result: bool,
    ) -> ConversationEntry {
        let payload = if result {
            ConversationEntryPayload::ToolResult(jaco_core::ToolResultEntry {
                tool_invocation_id: Some(inner_id.to_string()),
                call_id: format!("call-{inner_id}"),
                content: Vec::new(),
                is_error: false,
                structured_output: None,
                raw_output: None,
            })
        } else {
            ConversationEntryPayload::ToolCall(jaco_core::ToolCallEntry {
                tool_invocation_id: Some(inner_id.to_string()),
                call_id: format!("call-{inner_id}"),
                source: ToolSource::Local,
                name: "same-name".to_string(),
                runtime_tool_name: "same-name".to_string(),
                arguments: ToolArguments { value: json!({}) },
            })
        };
        ConversationEntry {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            seq,
            kind: payload.kind(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some("run-1".to_string()),
            provider_step_id: None,
            tool_invocation_id: outer_id.map(str::to_string),
            provider_item_id: None,
            payload,
            search_text: String::new(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn assert_preview_is_bounded(preview: &BoundedPreview, limits: ToolPreviewLimits) {
        assert!(preview.text.len() <= limits.max_output_bytes);
        assert!(preview.text.lines().count() <= limits.max_lines);
        assert!(std::str::from_utf8(preview.text.as_bytes()).is_ok());
        if preview.truncated {
            assert!(preview.text.ends_with(TRUNCATION_MARKER));
        }
    }

    fn assert_copy_contains_fields(copy: &str, fields: &[DisplayField]) {
        for field in fields {
            assert!(
                copy.contains(&format!("{}: {}", field.label, field.value)),
                "copy missing field {:?}: {copy}",
                field
            );
        }
    }
}
