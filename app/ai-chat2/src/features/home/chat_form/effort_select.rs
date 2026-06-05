use super::{
    ChatForm,
    picker::{PickerPopoverConfig, PickerSection, picker_popover, picker_trigger},
    thinking_effort::{reasoning_selection_label, reasoning_selections},
};
use crate::{foundation, foundation::assets::IconName};
use ai_chat_core::{ModelCapabilitiesSnapshot, ReasoningSelectionSnapshot};
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Sizable, h_flex, input::NumberInput, label::Label, select::SelectItem,
};

#[derive(Clone, Debug)]
pub(super) struct EffortOption {
    selection: ReasoningSelectionSnapshot,
    label: SharedString,
}

impl EffortOption {
    fn new(selection: ReasoningSelectionSnapshot, i18n: &foundation::I18n) -> Self {
        Self {
            label: reasoning_selection_label(&selection, i18n).into(),
            selection,
        }
    }

    pub(super) fn selection(&self) -> &ReasoningSelectionSnapshot {
        &self.selection
    }
}

impl SelectItem for EffortOption {
    type Value = ReasoningSelectionSnapshot;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        Label::new(reasoning_selection_label(
            &self.selection,
            cx.global::<foundation::I18n>(),
        ))
        .text_sm()
        .truncate()
    }

    fn value(&self) -> &Self::Value {
        &self.selection
    }
}

pub(super) fn effort_sections(
    capabilities: Option<&ModelCapabilitiesSnapshot>,
    i18n: &foundation::I18n,
) -> Vec<PickerSection<EffortOption>> {
    let selections =
        reasoning_selections(capabilities.and_then(|capabilities| capabilities.reasoning.as_ref()));
    vec![PickerSection::section(
        i18n.t("chat-form-thinking-header"),
        selections
            .into_iter()
            .map(|selection| EffortOption::new(selection, i18n)),
    )]
}

impl ChatForm {
    pub(super) fn render_effort_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self
            .selected_reasoning_selection
            .as_ref()
            .map(|selection| reasoning_selection_label(selection, cx.global::<foundation::I18n>()))
            .unwrap_or_else(|| cx.global::<foundation::I18n>().t("chat-form-effort-select"));

        let has_effort_options = self.has_effort_options();
        let footer = self.render_token_budget_footer(cx);

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
                footer,
                on_open_change: cx.listener(|form, open: &bool, window, cx| {
                    form.set_effort_picker_open(*open, window, cx);
                }),
            },
        )
    }

    fn render_token_budget_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.has_token_budget_options().then(|| {
            div()
                .border_t_1()
                .border_color(cx.theme().border)
                .p_1()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .px_1()
                        .py_1()
                        .child(
                            Label::new(
                                cx.global::<foundation::I18n>()
                                    .t("chat-form-effort-token-budget"),
                            )
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                        )
                        .child(
                            NumberInput::new(&self.token_budget_input)
                                .small()
                                .w(px(112.)),
                        ),
                )
                .into_any_element()
        })
    }
}
