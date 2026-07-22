use crate::{
    components::picker::PickerSection,
    components::run_settings::{reasoning_selection_label, reasoning_selections},
    foundation,
};
use gpui::*;
use gpui_component::{label::Label, select::SelectItem};
use jaco_core::{ModelCapabilitiesSnapshot, ReasoningSelectionSnapshot};

#[derive(Clone, Debug)]
pub(crate) struct EffortOption {
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

    pub(crate) fn selection(&self) -> &ReasoningSelectionSnapshot {
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

pub(crate) fn effort_sections(
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
