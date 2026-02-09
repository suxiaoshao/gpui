use crate::{
    adapter::render_template_detail_by_adapter,
    database::{ConversationTemplate, Db, Mode},
    errors::AiChatResult,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, h_flex, label::Label, scroll::ScrollableElement, tag::Tag, v_flex,
};

actions!(template_detail_view, [RefreshTemplateDetail]);

pub(crate) struct TemplateDetailView {
    template_id: i32,
    template: AiChatResult<ConversationTemplate>,
}

impl TemplateDetailView {
    pub fn new(template_id: i32, cx: &mut Context<Self>) -> Self {
        Self {
            template_id,
            template: Self::get_template(template_id, cx),
        }
    }

    fn get_template(template_id: i32, cx: &mut Context<Self>) -> AiChatResult<ConversationTemplate> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::find(template_id, conn)
    }

    fn refresh(&mut self, _: &RefreshTemplateDetail, _window: &mut Window, cx: &mut Context<Self>) {
        self.template = Self::get_template(self.template_id, cx);
        cx.notify();
    }
}

impl Render for TemplateDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .key_context("template-detail-view")
            .on_action(cx.listener(Self::refresh))
            .child(match &self.template {
                Ok(template) => {
                    render_template_detail_by_adapter(template, cx)
                        .unwrap_or_else(|_| fallback_template_detail(template, cx))
                }
                Err(err) => v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(Label::new(format!("Load template failed: {err}")).text_sm())
                    .into_any_element(),
            })
    }
}

fn fallback_template_detail(template: &ConversationTemplate, cx: &App) -> AnyElement {
    let template_json = serde_json::to_string_pretty(&template.template).unwrap_or_default();
    v_flex()
        .size_full()
        .gap_3()
        .p_4()
        .overflow_y_scrollbar()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Label::new(&template.icon))
                .child(Label::new(&template.name).text_xl())
                .child(
                    match template.mode {
                        Mode::Contextual => Tag::primary(),
                        Mode::Single => Tag::info(),
                        Mode::AssistantOnly => Tag::success(),
                    }
                    .outline()
                    .child(template.mode.to_string()),
                ),
        )
        .map(|this| match template.description.as_ref() {
            Some(description) => this.child(Label::new(description).text_sm()),
            None => this,
        })
        .child(Label::new(format!("Adapter: {}", template.adapter)).text_sm())
        .child(Label::new("Template JSON").text_sm())
        .child(
            div()
                .w_full()
                .p_3()
                .rounded_md()
                .bg(cx.theme().secondary)
                .child(Label::new(template_json).text_xs()),
        )
        .into_any_element()
}
