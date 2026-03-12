use super::picker::PickerSection;
use crate::database::ConversationTemplate;
use gpui::{App, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div};
use gpui_component::{h_flex, label::Label, select::SelectItem};

#[derive(Clone)]
pub(crate) struct TemplateOption {
    template: ConversationTemplate,
}

impl TemplateOption {
    pub(crate) fn new(template: ConversationTemplate) -> Self {
        Self { template }
    }

    pub(crate) fn into_template(self) -> ConversationTemplate {
        self.template
    }
}

impl SelectItem for TemplateOption {
    type Value = i32;

    fn title(&self) -> SharedString {
        self.template.name.clone().into()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let label = if let Some(description) = self.template.description.clone() {
            Label::new(self.template.name.clone())
                .text_sm()
                .secondary(description)
        } else {
            Label::new(self.template.name.clone()).text_sm()
        };

        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .child(Label::new(self.template.icon.clone()))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .truncate()
                    .child(label),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.template.id
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.template.name.to_lowercase().contains(&query)
            || self
                .template
                .description
                .as_ref()
                .is_some_and(|description| description.to_lowercase().contains(&query))
    }
}

pub(crate) fn template_sections(
    templates: impl IntoIterator<Item = ConversationTemplate>,
) -> Vec<PickerSection<TemplateOption>> {
    PickerSection::flat(templates.into_iter().map(TemplateOption::new))
}
