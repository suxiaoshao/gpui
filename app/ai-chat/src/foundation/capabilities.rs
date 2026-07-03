use crate::{foundation::i18n::I18n, llm::CapabilityRequirement};
use gpui::App;

pub(crate) fn capability_label(requirement: CapabilityRequirement, cx: &App) -> String {
    cx.global::<I18n>().t(requirement.label_key())
}

pub(crate) fn capability_labels(requirements: &[CapabilityRequirement], cx: &App) -> Vec<String> {
    requirements
        .iter()
        .copied()
        .map(|requirement| capability_label(requirement, cx))
        .collect()
}

pub(crate) fn capability_labels_text(requirements: &[CapabilityRequirement], cx: &App) -> String {
    capability_labels(requirements, cx).join(", ")
}
