use std::{collections::BTreeMap, rc::Rc};

use crate::{
    components::hotkey_input::format_hotkey_label,
    components::run_settings::reasoning_selection_is_valid,
    foundation::{
        I18n,
        assets::{IconName, provider_visual_for_kind, provider_visual_icon},
        search::field_matches_query,
    },
    state::hotkey::ShortcutRuntimeDiagnostics,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    switch::Switch,
    tag::Tag,
    v_flex,
};
use jaco_core::{ShortcutId, ShortcutInputSource};
use jaco_db::{PromptRecord, ProviderModelRecord, ProviderRecord, ShortcutRecord};

use super::validation::canonical_hotkey;

type ShortcutActionHandler = Rc<dyn Fn(ShortcutId, &mut Window, &mut App) + 'static>;
type ShortcutToggleHandler = Rc<dyn Fn(ShortcutId, bool, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShortcutStatus {
    Enabled,
    Disabled,
    InvalidHotkey,
    HotkeyConflict,
    PromptUnavailable,
    ModelUnavailable,
    CapabilityMismatch,
    RegistrationFailed,
}

impl ShortcutStatus {
    fn label(&self, i18n: &I18n) -> SharedString {
        i18n.t(match self {
            Self::Enabled => "shortcut-status-enabled",
            Self::Disabled => "shortcut-status-disabled",
            Self::InvalidHotkey => "shortcut-status-hotkey-invalid",
            Self::HotkeyConflict => "shortcut-status-hotkey-conflict",
            Self::PromptUnavailable => "shortcut-status-prompt-unavailable",
            Self::ModelUnavailable => "shortcut-status-model-unavailable",
            Self::CapabilityMismatch => "shortcut-status-capability-mismatch",
            Self::RegistrationFailed => "shortcut-status-registration-failed",
        })
        .into()
    }

    fn tag(&self) -> Tag {
        match self {
            Self::Enabled => Tag::success(),
            Self::Disabled => Tag::secondary(),
            Self::InvalidHotkey
            | Self::HotkeyConflict
            | Self::PromptUnavailable
            | Self::ModelUnavailable
            | Self::CapabilityMismatch
            | Self::RegistrationFailed => Tag::warning(),
        }
        .small()
        .outline()
    }

    fn should_render_row_tag(&self) -> bool {
        !matches!(self, Self::Enabled | Self::Disabled)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShortcutManagementRow {
    pub(super) id: ShortcutId,
    pub(super) hotkey: String,
    pub(super) hotkey_label: SharedString,
    pub(super) prompt_label: SharedString,
    pub(super) provider_kind: Option<String>,
    pub(super) provider_label: SharedString,
    pub(super) model_label: SharedString,
    pub(super) input_source_label: SharedString,
    pub(super) action_label: SharedString,
    pub(super) status: ShortcutStatus,
    pub(super) status_label: SharedString,
    pub(super) updated_label: SharedString,
    pub(super) enabled: bool,
    search_text: String,
}

#[derive(IntoElement, Clone)]
pub(super) struct ShortcutManagementEntry {
    row: ShortcutManagementRow,
    on_view: ShortcutActionHandler,
    on_edit: ShortcutActionHandler,
    on_reregister: ShortcutActionHandler,
    on_delete: ShortcutActionHandler,
    on_toggle: ShortcutToggleHandler,
}

impl ShortcutManagementEntry {
    pub(super) fn new(row: ShortcutManagementRow) -> Self {
        Self {
            row,
            on_view: Rc::new(|_, _, _| {}),
            on_edit: Rc::new(|_, _, _| {}),
            on_reregister: Rc::new(|_, _, _| {}),
            on_delete: Rc::new(|_, _, _| {}),
            on_toggle: Rc::new(|_, _, _, _| {}),
        }
    }

    pub(super) fn on_view(
        mut self,
        handler: impl Fn(ShortcutId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_view = Rc::new(handler);
        self
    }

    pub(super) fn on_edit(
        mut self,
        handler: impl Fn(ShortcutId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Rc::new(handler);
        self
    }

    pub(super) fn on_reregister(
        mut self,
        handler: impl Fn(ShortcutId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reregister = Rc::new(handler);
        self
    }

    pub(super) fn on_delete(
        mut self,
        handler: impl Fn(ShortcutId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Rc::new(handler);
        self
    }

    pub(super) fn on_toggle(
        mut self,
        handler: impl Fn(ShortcutId, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Rc::new(handler);
        self
    }
}

impl RenderOnce for ShortcutManagementEntry {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let view_label = i18n.t("button-view");
        let edit_label = i18n.t("button-edit");
        let reregister_label = i18n.t("shortcut-action-reregister");
        let delete_label = i18n.t("button-delete");

        let row_id = self.row.id.clone();
        let view_id = self.row.id.clone();
        let edit_id = self.row.id.clone();
        let reregister_id = self.row.id.clone();
        let delete_id = self.row.id.clone();
        let toggle_id = self.row.id.clone();
        let on_row_view = self.on_view.clone();
        let on_view = self.on_view.clone();
        let on_edit = self.on_edit.clone();
        let on_reregister = self.on_reregister.clone();
        let on_delete = self.on_delete.clone();
        let on_toggle = self.on_toggle.clone();
        let hotkey_label = self.row.hotkey_label.clone();
        let status = self.row.status.clone();
        let status_label = self.row.status_label.clone();
        let updated_label = self.row.updated_label.clone();
        let show_status_tag = status.should_render_row_tag();
        let model_label = shortcut_model_label(&self.row);
        let detail_label = shortcut_detail_label(&self.row);
        let provider_icon = provider_visual_icon(provider_visual_for_kind(
            self.row.provider_kind.as_deref().unwrap_or_default(),
        ));

        h_flex()
            .id(format!("shortcut-settings-row-{}", self.row.id))
            .w_full()
            .min_w_0()
            .items_center()
            .gap_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.background.background)
            .px_3()
            .py_2()
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().tokens.accent.background.opacity(0.45)))
            .on_click(move |_, window, cx| on_row_view(row_id.clone(), window, cx))
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().tokens.border.background.opacity(0.35))
                    .child(
                        provider_icon
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new(hotkey_label)
                                    .flex_none()
                                    .max_w(px(240.))
                                    .text_sm()
                                    .font_medium()
                                    .truncate(),
                            )
                            .when(show_status_tag, |this| {
                                this.child(status.tag().child(status_label))
                            })
                            .child(
                                Label::new(updated_label)
                                    .flex_none()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                Label::new(model_label)
                                    .flex_none()
                                    .max_w(px(360.))
                                    .truncate(),
                            )
                            .child(Label::new("->").flex_none())
                            .child(Label::new(detail_label).flex_1().min_w_0().truncate()),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .child(
                        Switch::new(format!("shortcut-settings-enabled-{toggle_id}"))
                            .small()
                            .checked(self.row.enabled)
                            .on_click(move |checked, window, cx| {
                                cx.stop_propagation();
                                on_toggle(toggle_id.clone(), *checked, window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("shortcut-settings-view-{view_id}"))
                            .icon(IconName::Eye)
                            .ghost()
                            .tooltip(view_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_view(view_id.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("shortcut-settings-edit-{edit_id}"))
                            .icon(IconName::Pencil)
                            .ghost()
                            .tooltip(edit_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_edit(edit_id.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("shortcut-settings-reregister-{reregister_id}"))
                            .icon(IconName::RefreshCcw)
                            .ghost()
                            .tooltip(reregister_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_reregister(reregister_id.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("shortcut-settings-delete-{delete_id}"))
                            .icon(IconName::Trash)
                            .danger()
                            .tooltip(delete_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_delete(delete_id.clone(), window, cx);
                            }),
                    ),
            )
    }
}

pub(super) fn shortcut_management_rows(
    shortcuts: &[ShortcutRecord],
    prompts: &[PromptRecord],
    providers: &[(ProviderRecord, Vec<ProviderModelRecord>)],
    diagnostics: &ShortcutRuntimeDiagnostics,
    i18n: &I18n,
) -> Vec<ShortcutManagementRow> {
    let hotkey_counts = canonical_hotkey_counts(shortcuts);
    let temporary_hotkey = diagnostics
        .temporary_hotkey
        .as_deref()
        .and_then(|hotkey| canonical_hotkey(hotkey).ok());

    shortcuts
        .iter()
        .map(|shortcut| {
            let prompt = shortcut
                .prompt_id
                .as_ref()
                .and_then(|prompt_id| prompts.iter().find(|prompt| &prompt.id == prompt_id));
            let (provider, model) = shortcut_provider_model(shortcut, providers);
            let status = shortcut_status(
                shortcut,
                prompt,
                provider,
                model,
                diagnostics,
                &hotkey_counts,
                temporary_hotkey.as_deref(),
            );
            let prompt_label = prompt
                .map(|prompt| prompt.name.clone())
                .unwrap_or_else(|| i18n.t("shortcut-prompt-none").to_string());
            let provider_label = provider
                .map(|provider| provider.display_name.clone())
                .unwrap_or_else(|| i18n.t("shortcut-model-unavailable").to_string());
            let provider_kind = provider.map(|provider| provider.kind.clone());
            let model_label = model
                .and_then(|model| model.display_name.clone())
                .or_else(|| shortcut.model_id.clone())
                .unwrap_or_else(|| i18n.t("shortcut-model-unavailable").to_string());
            let input_source_label = input_source_label(shortcut.input_source, i18n);
            let action_label = i18n.t("shortcut-action-temporary-conversation");
            let status_label = status.label(i18n);
            let updated_label = shortcut_updated_label(shortcut.updated_at);
            let hotkey_label = format_hotkey_label(&shortcut.hotkey);
            let search_text = format!(
                "{} {} {} {} {} {} {} shortcuts shortcut hotkey prompt provider model selection clipboard screenshot ocr 快捷键 全局快捷键 提示词 模型 提供商 选中文字 剪贴板 截图",
                shortcut.hotkey,
                hotkey_label,
                prompt_label,
                provider_label,
                model_label,
                input_source_label,
                action_label,
            )
            .to_lowercase();

            ShortcutManagementRow {
                id: shortcut.id.clone(),
                hotkey: shortcut.hotkey.clone(),
                hotkey_label: hotkey_label.into(),
                prompt_label: prompt_label.into(),
                provider_kind,
                provider_label: provider_label.into(),
                model_label: model_label.into(),
                input_source_label: input_source_label.into(),
                action_label: action_label.into(),
                status,
                status_label,
                updated_label: updated_label.into(),
                enabled: shortcut.enabled,
                search_text,
            }
        })
        .collect()
}

pub(super) fn filter_shortcut_rows(
    rows: &[ShortcutManagementRow],
    query: &str,
) -> Vec<ShortcutManagementRow> {
    let query = query.trim();
    if query.is_empty() {
        return rows.to_vec();
    }

    rows.iter()
        .filter(|row| field_matches_query(&row.search_text, query))
        .cloned()
        .collect()
}

fn shortcut_model_label(row: &ShortcutManagementRow) -> SharedString {
    row.model_label.clone()
}

fn shortcut_detail_label(row: &ShortcutManagementRow) -> SharedString {
    format!(
        "{} -> {} -> {}",
        row.prompt_label.as_ref(),
        row.input_source_label.as_ref(),
        row.action_label.as_ref()
    )
    .into()
}

pub(super) fn input_source_label(source: ShortcutInputSource, i18n: &I18n) -> String {
    match source {
        ShortcutInputSource::SelectionOrClipboard => {
            i18n.t("shortcut-input-selection-or-clipboard").to_string()
        }
        ShortcutInputSource::Screenshot => i18n.t("shortcut-input-screenshot").to_string(),
    }
}

pub(super) fn shortcut_updated_label(updated_at: time::OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        updated_at.year(),
        u8::from(updated_at.month()),
        updated_at.day(),
        updated_at.hour(),
        updated_at.minute()
    )
}

fn shortcut_status(
    shortcut: &ShortcutRecord,
    prompt: Option<&PromptRecord>,
    provider: Option<&ProviderRecord>,
    model: Option<&ProviderModelRecord>,
    diagnostics: &ShortcutRuntimeDiagnostics,
    hotkey_counts: &BTreeMap<String, usize>,
    temporary_hotkey: Option<&str>,
) -> ShortcutStatus {
    let Ok(canonical) = canonical_hotkey(&shortcut.hotkey) else {
        return ShortcutStatus::InvalidHotkey;
    };
    if temporary_hotkey == Some(canonical.as_str())
        || hotkey_counts.get(&canonical).copied().unwrap_or_default() > 1
    {
        return ShortcutStatus::HotkeyConflict;
    }
    if shortcut
        .prompt_id
        .as_ref()
        .is_some_and(|_| prompt.is_none_or(|prompt| !prompt.enabled))
    {
        return ShortcutStatus::PromptUnavailable;
    }
    if provider.is_none_or(|provider| !provider.enabled) || model.is_none_or(|model| !model.enabled)
    {
        return ShortcutStatus::ModelUnavailable;
    }
    if shortcut
        .settings_snapshot
        .reasoning_selection
        .as_ref()
        .is_some_and(|selection| {
            !reasoning_selection_is_valid(
                model.and_then(|model| model.capabilities.reasoning.as_ref()),
                selection,
            )
        })
    {
        return ShortcutStatus::CapabilityMismatch;
    }
    if diagnostics.registration_errors.contains_key(&shortcut.id) {
        return ShortcutStatus::RegistrationFailed;
    }
    if shortcut.enabled {
        ShortcutStatus::Enabled
    } else {
        ShortcutStatus::Disabled
    }
}

fn shortcut_provider_model<'a>(
    shortcut: &ShortcutRecord,
    providers: &'a [(ProviderRecord, Vec<ProviderModelRecord>)],
) -> (Option<&'a ProviderRecord>, Option<&'a ProviderModelRecord>) {
    let Some(provider_id) = shortcut.provider_id.as_ref() else {
        return (None, None);
    };
    let Some(model_id) = shortcut.model_id.as_ref() else {
        return (
            providers
                .iter()
                .find(|(provider, _)| &provider.id == provider_id)
                .map(|(provider, _)| provider),
            None,
        );
    };
    let Some((provider, models)) = providers
        .iter()
        .find(|(provider, _)| &provider.id == provider_id)
    else {
        return (None, None);
    };
    (
        Some(provider),
        models.iter().find(|model| &model.model_id == model_id),
    )
}

fn canonical_hotkey_counts(shortcuts: &[ShortcutRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for shortcut in shortcuts {
        if let Ok(hotkey) = canonical_hotkey(&shortcut.hotkey) {
            *counts.entry(hotkey).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::{
        ShortcutStatus, filter_shortcut_rows, shortcut_detail_label, shortcut_management_rows,
        shortcut_model_label,
    };
    use crate::state::hotkey::ShortcutRuntimeDiagnostics;
    use jaco_core::{
        ProviderModelMetadata, ProviderSettingsPayload, ReasoningSelectionSnapshot,
        RunSettingsSnapshot, ShortcutAction, ShortcutInputSource, ToolApprovalMode,
        ToolApprovalPolicy, ToolPolicySnapshot, conservative_model_capabilities,
    };
    use jaco_db::{PromptRecord, ProviderModelRecord, ProviderRecord, ShortcutRecord};
    use time::OffsetDateTime;

    #[test]
    fn rows_do_not_need_title_to_filter_by_hotkey_or_model() {
        let i18n = crate::foundation::I18n::english_for_test();
        let rows = shortcut_management_rows(
            &[shortcut("shortcut-1", "super+shift+j")],
            &[],
            &[(provider(), vec![model(true)])],
            &ShortcutRuntimeDiagnostics::default(),
            &i18n,
        );

        assert_eq!(filter_shortcut_rows(&rows, "shift").len(), 1);
        assert_eq!(filter_shortcut_rows(&rows, "gpt").len(), 1);
        assert_eq!(rows[0].status, ShortcutStatus::Enabled);
    }

    #[test]
    fn shortcut_row_labels_prioritize_model_metadata() {
        let i18n = crate::foundation::I18n::english_for_test();
        let rows = shortcut_management_rows(
            &[shortcut("shortcut-1", "super+shift+j")],
            &[],
            &[(provider(), vec![model(true)])],
            &ShortcutRuntimeDiagnostics::default(),
            &i18n,
        );

        assert_eq!(shortcut_model_label(&rows[0]).as_ref(), "gpt-5");
        assert_eq!(
            shortcut_detail_label(&rows[0]).as_ref(),
            "No prompt -> Selection or Clipboard -> Temporary Conversation"
        );
    }

    #[test]
    fn row_status_reports_duplicate_hotkeys() {
        let i18n = crate::foundation::I18n::english_for_test();
        let rows = shortcut_management_rows(
            &[
                shortcut("shortcut-1", "super+shift+j"),
                shortcut("shortcut-2", "cmd+shift+j"),
            ],
            &[],
            &[(provider(), vec![model(true)])],
            &ShortcutRuntimeDiagnostics::default(),
            &i18n,
        );

        assert_eq!(rows[0].status, ShortcutStatus::HotkeyConflict);
        assert_eq!(rows[1].status, ShortcutStatus::HotkeyConflict);
    }

    #[test]
    fn row_status_reports_reasoning_capability_mismatch() {
        let i18n = crate::foundation::I18n::english_for_test();
        let mut shortcut = shortcut("shortcut-1", "super+shift+j");
        shortcut.settings_snapshot.reasoning_selection = Some(ReasoningSelectionSnapshot::AlwaysOn);

        let rows = shortcut_management_rows(
            &[shortcut],
            &[],
            &[(provider(), vec![model(true)])],
            &ShortcutRuntimeDiagnostics::default(),
            &i18n,
        );

        assert_eq!(rows[0].status, ShortcutStatus::CapabilityMismatch);
    }

    fn shortcut(id: &str, hotkey: &str) -> ShortcutRecord {
        ShortcutRecord {
            id: id.to_string(),
            hotkey: hotkey.to_string(),
            enabled: true,
            prompt_id: None,
            provider_id: Some("provider".to_string()),
            model_id: Some("gpt-5".to_string()),
            input_source: ShortcutInputSource::SelectionOrClipboard,
            action: ShortcutAction::OpenTemporaryConversation,
            settings_snapshot: RunSettingsSnapshot {
                prompt: None,
                provider_id: "provider".to_string(),
                model_id: "gpt-5".to_string(),
                model_capabilities: conservative_model_capabilities("openai"),
                provider_settings: ProviderSettingsPayload {
                    provider_kind: "openai".to_string(),
                    fields: Vec::new(),
                },
                reasoning_selection: None,
                tool_policy: ToolPolicySnapshot {
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    enabled_sources: Vec::new(),
                    max_steps: 32,
                    approval_mode: ToolApprovalMode::RequestApproval,
                    permission_scope: None,
                },
            },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn provider() -> ProviderRecord {
        ProviderRecord {
            id: "provider".to_string(),
            kind: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: Vec::new(),
            },
            secret_refs: jaco_core::ProviderSecretRefs { refs: Vec::new() },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn model(enabled: bool) -> ProviderModelRecord {
        ProviderModelRecord {
            id: "model-row".to_string(),
            provider_id: "provider".to_string(),
            model_id: "gpt-5".to_string(),
            display_name: None,
            enabled,
            capabilities: conservative_model_capabilities("openai"),
            metadata: ProviderModelMetadata {
                display_name: None,
                family: None,
                raw: None,
            },
            fetched_at: OffsetDateTime::UNIX_EPOCH,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[allow(dead_code)]
    fn prompt(enabled: bool) -> PromptRecord {
        PromptRecord {
            id: "prompt".to_string(),
            name: "Prompt".to_string(),
            content: jaco_core::PromptContent {
                text: "Prompt text".to_string(),
            },
            enabled,
            sort_order: 10,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
