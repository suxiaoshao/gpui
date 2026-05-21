use super::picker::PickerSection;
use crate::{
    database::ConversationTemplate,
    foundation::{capability_labels_text, i18n::I18n},
    llm::{CapabilityRequirement, ProviderModel},
};
use gpui::{
    App, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use gpui_component::{h_flex, label::Label, select::SelectItem, tag::Tag, v_flex};

#[derive(Clone)]
pub(crate) struct TemplateOption {
    template: ConversationTemplate,
    missing_requirements: Option<Vec<CapabilityRequirement>>,
}

impl TemplateOption {
    pub(crate) fn new(
        template: ConversationTemplate,
        selected_model: Option<&ProviderModel>,
    ) -> Self {
        let missing_requirements = selected_model.map(|model| {
            model
                .capabilities
                .missing_requirements(&template.required_capabilities)
        });
        Self {
            template,
            missing_requirements,
        }
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

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = if let Some(description) = self.template.description.clone() {
            Label::new(self.template.name.clone())
                .text_sm()
                .secondary(description)
        } else {
            Label::new(self.template.name.clone()).text_sm()
        };

        let compatibility = self.missing_requirements.as_ref().map(|requirements| {
            let compatibility_label = if requirements.is_empty() {
                cx.global::<I18n>().t("template-compatibility-compatible")
            } else {
                cx.global::<I18n>().t("template-compatibility-incompatible")
            };
            if requirements.is_empty() {
                Tag::success()
                    .outline()
                    .child(compatibility_label)
                    .into_any_element()
            } else {
                Tag::warning()
                    .outline()
                    .child(compatibility_label)
                    .into_any_element()
            }
        });

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
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(label)
                            .when_some(compatibility, |this, compatibility| {
                                this.child(compatibility)
                            })
                            .when_some(self.missing_requirements.as_ref(), |this, requirements| {
                                this.when(!requirements.is_empty(), |this| {
                                    this.child(
                                        Label::new(capability_labels_text(requirements, cx))
                                            .text_xs()
                                            .truncate(),
                                    )
                                })
                            }),
                    ),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.template.id
    }

    fn matches(&self, query: &str) -> bool {
        self.template.matches_search_query(query)
    }
}

pub(crate) fn template_sections(
    templates: impl IntoIterator<Item = ConversationTemplate>,
    selected_model: Option<&ProviderModel>,
) -> Vec<PickerSection<TemplateOption>> {
    PickerSection::flat(
        templates
            .into_iter()
            .map(|template| TemplateOption::new(template, selected_model)),
    )
}

#[cfg(test)]
mod tests {
    use super::template_sections;
    use crate::{
        database::ConversationTemplate,
        llm::{CapabilityRequirement, ModelCapabilities, ProviderModel},
    };
    use time::OffsetDateTime;

    fn template(required_capabilities: Vec<CapabilityRequirement>) -> ConversationTemplate {
        let now = OffsetDateTime::now_utc();
        ConversationTemplate {
            id: 1,
            name: "Vision".to_string(),
            icon: "eye".to_string(),
            description: None,
            prompts: Vec::new(),
            required_capabilities,
            created_time: now,
            updated_time: now,
        }
    }

    fn model(id: &str, capabilities: ModelCapabilities) -> ProviderModel {
        ProviderModel::new("Test", id, capabilities)
    }

    #[test]
    fn template_sections_recompute_compatibility_for_selected_model() {
        let template = template(vec![CapabilityRequirement::StructuredOutput]);
        let text_model = model("text", ModelCapabilities::text_streaming());
        let mut structured_capabilities = ModelCapabilities::text_streaming();
        structured_capabilities.structured_output = true;
        let structured_model = model("structured", structured_capabilities);

        let text_sections = template_sections(vec![template.clone()], Some(&text_model));
        assert_eq!(
            text_sections[0].items[0]
                .as_ref()
                .missing_requirements
                .clone(),
            Some(vec![CapabilityRequirement::StructuredOutput])
        );

        let structured_sections =
            template_sections(vec![template.clone()], Some(&structured_model));
        assert_eq!(
            structured_sections[0].items[0]
                .as_ref()
                .missing_requirements
                .clone(),
            Some(Vec::new())
        );

        let unknown_sections = template_sections(vec![template], None);
        assert_eq!(
            unknown_sections[0].items[0].as_ref().missing_requirements,
            None
        );
    }
}
