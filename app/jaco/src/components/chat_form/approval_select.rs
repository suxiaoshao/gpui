use super::ChatForm;
use crate::{
    components::picker::{PickerPopoverConfig, PickerSection, picker_popover, picker_trigger},
    foundation,
    foundation::assets::IconName,
};
use gpui::*;
use gpui_component::{label::Label, select::SelectItem};
use jaco_core::ToolApprovalMode;

#[derive(Clone, Debug)]
pub(super) struct ApprovalModeOption {
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

    pub(super) fn mode(&self) -> ToolApprovalMode {
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

pub(super) fn approval_mode_sections(
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

pub(super) fn approval_mode_label(mode: ToolApprovalMode, i18n: &foundation::I18n) -> String {
    i18n.t(match mode {
        ToolApprovalMode::AutoApprove => "chat-form-approval-auto",
        ToolApprovalMode::RequestApproval => "chat-form-approval-request",
        ToolApprovalMode::FullAccess => "chat-form-approval-full",
    })
}

impl ChatForm {
    pub(super) fn render_approval_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        picker_popover(
            cx,
            PickerPopoverConfig {
                id: "chat-form-approval-popover",
                open: self.approval_picker_open,
                trigger: picker_trigger(
                    "chat-form-approval-trigger",
                    IconName::Shield,
                    approval_mode_label(
                        self.selected_approval_mode,
                        cx.global::<foundation::I18n>(),
                    ),
                    self.approval_picker_open,
                ),
                list: self.approval_picker.clone(),
                width: px(180.),
                max_height: rems(12.).into(),
                search_placeholder: None,
                footer: None,
                on_open_change: cx.listener(|form, open: &bool, window, cx| {
                    form.set_approval_picker_open(*open, window, cx);
                }),
            },
        )
    }
}
