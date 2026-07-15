use crate::{components::picker::PickerSection, foundation};
use gpui::*;
use gpui_component::{label::Label, select::SelectItem};
use jaco_core::ToolApprovalMode;

#[derive(Clone, Debug)]
pub(crate) struct ApprovalModeOption {
    mode: ToolApprovalMode,
    label: SharedString,
}

impl ApprovalModeOption {
    fn new(mode: ToolApprovalMode, i18n: &foundation::I18n) -> Self {
        Self {
            label: approval_mode_label(mode, i18n).into(),
            mode,
        }
    }

    pub(crate) fn mode(&self) -> ToolApprovalMode {
        self.mode
    }
}

impl SelectItem for ApprovalModeOption {
    type Value = ToolApprovalMode;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn render(&self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Label::new(self.label.clone()).text_sm().truncate()
    }

    fn value(&self) -> &Self::Value {
        &self.mode
    }
}

pub(crate) fn approval_mode_sections(
    i18n: &foundation::I18n,
) -> Vec<PickerSection<ApprovalModeOption>> {
    vec![PickerSection::section(
        i18n.t("chat-form-approval-header"),
        [
            ToolApprovalMode::AutoApprove,
            ToolApprovalMode::RequestApproval,
            ToolApprovalMode::FullAccess,
        ]
        .into_iter()
        .map(|mode| ApprovalModeOption::new(mode, i18n)),
    )]
}

pub(crate) fn approval_mode_label(mode: ToolApprovalMode, i18n: &foundation::I18n) -> String {
    i18n.t(match mode {
        ToolApprovalMode::AutoApprove => "chat-form-approval-auto",
        ToolApprovalMode::RequestApproval => "chat-form-approval-request",
        ToolApprovalMode::FullAccess => "chat-form-approval-full",
    })
}
