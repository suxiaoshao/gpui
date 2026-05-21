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
    missing_requirements: Vec<CapabilityRequirement>,
}

impl TemplateOption {
    pub(crate) fn new(
        template: ConversationTemplate,
        selected_model: Option<&ProviderModel>,
    ) -> Self {
        let missing_requirements = selected_model
            .map(|model| {
                model
                    .capabilities
                    .missing_requirements(&template.required_capabilities)
            })
            .unwrap_or_default();
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

        let compatibility_label = if self.missing_requirements.is_empty() {
            cx.global::<I18n>().t("template-compatibility-compatible")
        } else {
            cx.global::<I18n>().t("template-compatibility-incompatible")
        };
        let compatibility = if self.missing_requirements.is_empty() {
            Tag::success().outline().child(compatibility_label)
        } else {
            Tag::warning().outline().child(compatibility_label)
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
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(label)
                            .child(compatibility)
                            .when(!self.missing_requirements.is_empty(), |this| {
                                this.child(
                                    Label::new(capability_labels_text(
                                        &self.missing_requirements,
                                        cx,
                                    ))
                                    .text_xs()
                                    .truncate(),
                                )
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
