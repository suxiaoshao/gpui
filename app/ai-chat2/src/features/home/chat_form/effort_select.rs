use super::{
    ChatForm,
    picker::{PickerPopoverConfig, PickerSection, picker_popover, picker_trigger},
    thinking_effort::{ThinkingEffort, reasoning_efforts},
};
use crate::{foundation, foundation::assets::IconName};
use ai_chat_core::ModelCapabilitiesSnapshot;
use gpui::*;
use gpui_component::{Disableable, label::Label, select::SelectItem};

#[derive(Clone, Debug)]
pub(super) struct EffortOption {
    pub(super) effort: ThinkingEffort,
}

impl EffortOption {
    fn new(effort: ThinkingEffort) -> Self {
        Self { effort }
    }
}

impl SelectItem for EffortOption {
    type Value = ThinkingEffort;

    fn title(&self) -> SharedString {
        match self.effort {
            ThinkingEffort::None => "None",
            ThinkingEffort::Low => "Low",
            ThinkingEffort::Medium => "Medium",
            ThinkingEffort::High => "High",
            ThinkingEffort::XHigh => "Extra High",
        }
        .into()
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        Label::new(self.effort.label(cx.global::<foundation::I18n>()))
            .text_sm()
            .truncate()
    }

    fn value(&self) -> &Self::Value {
        &self.effort
    }
}

pub(super) fn effort_sections(
    capabilities: Option<&ModelCapabilitiesSnapshot>,
    i18n: &foundation::I18n,
) -> Vec<PickerSection<EffortOption>> {
    let efforts =
        reasoning_efforts(capabilities.and_then(|capabilities| capabilities.reasoning.as_ref()));
    vec![PickerSection::section(
        i18n.t("chat-form-thinking-header"),
        efforts.into_iter().map(EffortOption::new),
    )]
}

impl ChatForm {
    pub(super) fn render_effort_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self
            .selected_effort
            .map(|effort| effort.label(cx.global::<foundation::I18n>()))
            .unwrap_or_else(|| cx.global::<foundation::I18n>().t("chat-form-effort-select"));

        let has_effort_options = self.has_effort_options();

        picker_popover(
            cx,
            PickerPopoverConfig {
                id: "chat-form-effort-popover",
                open: self.effort_picker_open,
                trigger: picker_trigger(
                    "chat-form-effort-trigger",
                    IconName::Lightbulb,
                    label,
                    self.effort_picker_open,
                )
                .disabled(!has_effort_options),
                list: self.effort_picker.clone(),
                width: px(180.),
                max_height: rems(16.).into(),
                search_placeholder: None,
                footer: None,
                on_open_change: cx.listener(|form, open: &bool, window, cx| {
                    form.set_effort_picker_open(*open, window, cx);
                }),
            },
        )
    }
}
