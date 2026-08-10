use std::ops::Deref;

use gpui::{Context, Entity, Render, Window};
use gpui_component::input::{Input, InputContentType, InputState};
use gpui_form::Form;
use gpui_form_gpui_component::FormInput;

use super::draft::RequestDraft;
use crate::foundation::I18n;

pub(super) struct UrlInput {
    input: FormInput,
}

impl UrlInput {
    pub(super) fn new<Owner>(
        form: &Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Owner: 'static,
    {
        let placeholder = cx.global::<I18n>().t("field-url");
        let input = FormInput::new(
            form,
            RequestDraft::URL,
            move |window, cx| InputState::new(window, cx).placeholder(placeholder),
            window,
            cx,
        );
        Self { input }
    }

    pub(super) fn element(&self) -> Input {
        Input::new(&self.input).content_type(InputContentType::Url)
    }
}

impl Deref for UrlInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl Render for UrlInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.element()
    }
}
