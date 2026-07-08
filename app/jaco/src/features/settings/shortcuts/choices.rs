use crate::foundation::search::field_matches_query;
use gpui::*;
use gpui_component::select::SelectItem;
use jaco_core::{PromptId, ShortcutInputSource};
use jaco_db::PromptRecord;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PromptChoice {
    value: Option<PromptId>,
    label: SharedString,
    search_text: String,
}

impl PromptChoice {
    pub(super) fn none(label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            value: None,
            search_text: label.to_string().to_lowercase(),
            label,
        }
    }

    pub(super) fn from_prompt(prompt: &PromptRecord) -> Self {
        Self {
            value: Some(prompt.id.clone()),
            label: prompt.name.clone().into(),
            search_text: format!(
                "{} {} prompt prompts 提示词",
                prompt.name, prompt.content.text
            )
            .to_lowercase(),
        }
    }
}

impl SelectItem for PromptChoice {
    type Value = Option<PromptId>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(self.label.clone().into_any_element())
    }

    fn matches(&self, query: &str) -> bool {
        field_matches_query(&self.search_text, query)
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InputSourceChoice {
    value: ShortcutInputSource,
    label: SharedString,
}

impl InputSourceChoice {
    pub(super) fn new(value: ShortcutInputSource, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }

    pub(super) fn value(&self) -> ShortcutInputSource {
        self.value
    }

    pub(super) fn label(&self) -> SharedString {
        self.label.clone()
    }
}
