use crate::{
    components::provider_template_form::ProviderTemplateFormState, i18n::I18n, llm::ChatFormLayout,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    group_box::{GroupBox, GroupBoxVariant, GroupBoxVariants},
    h_flex,
    label::Label,
    popover::Popover,
    scroll::ScrollableElement,
    v_flex,
};

pub(crate) struct ProviderChatFormView {
    form: Entity<ProviderTemplateFormState>,
    base_template: serde_json::Value,
    layout: ChatFormLayout,
}

impl ProviderChatFormView {
    pub(crate) fn new(
        form: Entity<ProviderTemplateFormState>,
        base_template: serde_json::Value,
        layout: ChatFormLayout,
    ) -> Self {
        Self {
            form,
            base_template,
            layout,
        }
    }

    pub(crate) fn effective_template(&self, cx: &App) -> Result<serde_json::Value, String> {
        self.form
            .read(cx)
            .collect_merged_template(&self.base_template, cx)
    }

    fn has_inline_fields(&self) -> bool {
        !self.layout.inline_field_ids.is_empty()
    }

    fn has_popover_fields(&self) -> bool {
        self.layout
            .popover_groups
            .iter()
            .any(|group| !group.field_ids.is_empty())
    }
}

impl Render for ProviderChatFormView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.has_inline_fields() && !self.has_popover_fields() {
            return div();
        }

        let inline_fields = {
            let form = self.form.read(cx);
            self.layout
                .inline_field_ids
                .iter()
                .filter_map(|id| form.render_inline_field(id))
                .collect::<Vec<_>>()
        };
        let show_popover = self.has_popover_fields();
        let settings_tooltip = cx.global::<I18n>().t("tooltip-chat-form-settings");

        h_flex()
            .items_center()
            .gap_1()
            .children(inline_fields)
            .when(show_popover, |this| {
                let form = self.form.clone();
                let layout = self.layout.clone();
                this.child(
                    Popover::new("provider-chat-form-popover")
                        .anchor(Corner::TopRight)
                        .trigger(
                            Button::new("provider-chat-form-settings")
                                .icon(IconName::Settings)
                                .ghost()
                                .small()
                                .tooltip(settings_tooltip),
                        )
                        .content(move |_state, _window, cx| {
                            let groups = layout
                                .popover_groups
                                .iter()
                                .enumerate()
                                .filter_map(|(ix, group)| {
                                    if group.field_ids.is_empty() {
                                        return None;
                                    }
                                    let title =
                                        group.title_key.map(|key| cx.global::<I18n>().t(key));
                                    let description =
                                        group.description_key.map(|key| cx.global::<I18n>().t(key));
                                    let fields = group
                                        .field_ids
                                        .iter()
                                        .filter_map(|field_id| form.read(cx).render_popover_field(field_id))
                                        .collect::<Vec<_>>();
                                    if fields.is_empty() {
                                        return None;
                                    }
                                    Some(
                                        GroupBox::new()
                                            .id(("provider-chat-form-group", ix))
                                            .with_variant(GroupBoxVariant::Outline)
                                            .when(title.is_some() || description.is_some(), |this| {
                                                this.title(
                                                    v_flex()
                                                        .gap_1()
                                                        .when_some(title.clone(), |this, title| {
                                                            this.child(Label::new(title))
                                                        })
                                                        .when_some(description.clone(), |this, description| {
                                                            this.child(
                                                                Label::new(description)
                                                                    .text_sm()
                                                                    .text_color(cx.theme().muted_foreground),
                                                            )
                                                        }),
                                                )
                                            })
                                            .children(fields)
                                            .into_any_element(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            div()
                                .w(px(380.))
                                .max_h(px(360.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w_full()
                                        .pr_1()
                                        .child(v_flex().gap_3().children(groups))
                                        .overflow_y_scrollbar(),
                                )
                        }),
                )
            })
    }
}
