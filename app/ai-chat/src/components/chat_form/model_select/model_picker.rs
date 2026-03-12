use crate::{
    llm::ProviderModel,
    components::chat_form::picker::PickerSection,
};
use gpui::{App, IntoElement, ParentElement as _, SharedString, Styled as _, Window};
use gpui_component::{h_flex, label::Label, select::SelectItem};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelOption {
    model: ProviderModel,
}

impl ModelOption {
    pub(crate) fn new(model: ProviderModel) -> Self {
        Self { model }
    }

    pub(crate) fn into_model(self) -> ProviderModel {
        self.model
    }
}

impl SelectItem for ModelOption {
    type Value = ProviderModel;

    fn title(&self) -> SharedString {
        self.model.id.clone().into()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .child(
                Label::new(self.model.id.clone())
                    .text_sm()
                    .truncate(),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.model
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.model.id.to_lowercase().contains(&query)
            || self.model.provider_name.to_lowercase().contains(&query)
    }
}

pub(crate) fn model_sections(models: &[ProviderModel]) -> Vec<PickerSection<ModelOption>> {
    let mut provider_groups = BTreeMap::<String, Vec<ModelOption>>::new();
    for model in models.iter().cloned() {
        provider_groups
            .entry(model.provider_name.clone())
            .or_default()
            .push(ModelOption::new(model));
    }

    provider_groups
        .into_iter()
        .map(|(provider, mut items)| {
            items.sort_by_key(|left| left.title());
            PickerSection::section(provider, items)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::model_sections;
    use crate::{
        components::chat_form::picker::PickerListDelegate,
        llm::{ProviderModel, ProviderModelCapability},
    };

    #[test]
    fn selected_index_for_returns_none_when_model_is_missing() {
        let sections = model_sections(&[ProviderModel::new(
            "OpenAI",
            "gpt-4o",
            ProviderModelCapability::Streaming,
        )]);

        let missing_model = ProviderModel::new(
            "OpenAI",
            "gpt-5",
            ProviderModelCapability::Streaming,
        );
        assert_eq!(
            PickerListDelegate::selected_index_for(&sections, Some(&missing_model)),
            None
        );
    }
}
