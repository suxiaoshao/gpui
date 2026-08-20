use gpui::{
    App, Context, Entity, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme as _,
    input::{Editor, EditorState},
    select::{SelectItem, SelectState},
    v_flex,
};
use gpui_form::{ControlBinding, ControlProjection, DynamicPath, Form};
use gpui_form_gpui_component::FormEditor;

use crate::{
    features::request::{
        controls::FormScalarSelect,
        draft::{RequestDraft, TextBodyDraft, TextBodyFormat},
    },
    foundation::I18n,
};

#[derive(Clone)]
struct TextFormatOption {
    value: TextBodyFormat,
    title: SharedString,
}

impl SelectItem for TextFormatOption {
    type Value = TextBodyFormat;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

type TextFormatOptions = Vec<TextFormatOption>;

const TEXT_BODY_FORMATS: [TextBodyFormat; 6] = [
    TextBodyFormat::PlainText,
    TextBodyFormat::Json,
    TextBodyFormat::JavaScript,
    TextBodyFormat::Html,
    TextBodyFormat::Xml,
    TextBodyFormat::Css,
];

pub(super) struct HttpTextView {
    format_preset: FormScalarSelect<RequestDraft, TextFormatOptions, TextBodyFormat>,
    content: FormEditor,
    _syntax_binding: ControlBinding,
}

impl HttpTextView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        text: DynamicPath<RequestDraft, TextBodyDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let format_path = text.clone().then(TextBodyDraft::FORMAT);
        let content_path = text.then(TextBodyDraft::CONTENT);
        let initial_format = format_path.try_get(&form, cx).unwrap_or_default();
        let content = FormEditor::try_new(
            &form,
            content_path,
            move |window, cx| {
                EditorState::new(window, cx)
                    .language(initial_format.editor_language())
                    .line_number(true)
                    .searchable(true)
            },
            window,
            cx,
        )
        .expect("the Text case was resolved immediately before its controls were built");
        let options = text_format_options(cx);
        let format_preset = FormScalarSelect::try_new(
            &form,
            format_path.clone(),
            move |window, cx| SelectState::new(options, None, window, cx),
            window,
            cx,
        )
        .expect("the Text format path is active while its view is being built");
        let content_state = (*content).clone();
        let (syntax_binding, _writer) = format_path
            .try_bind_control_in(
                &form,
                &content_state,
                |state, projection, _window, cx| match projection {
                    ControlProjection::Value(format) => {
                        state.set_highlighter(format.editor_language(), cx);
                    }
                    ControlProjection::Retired => {}
                },
                window,
                cx,
            )
            .expect("the Text format path is active while its view is being built");
        Self {
            format_preset,
            content,
            _syntax_binding: syntax_binding,
        }
    }
}

impl Render for HttpTextView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .p_2()
            .gap_2()
            .flex_1()
            .min_h(px(0.))
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(180.))
                    .child(self.format_preset.element()),
            )
            .child(
                div().flex_1().min_h(px(0.)).overflow_hidden().child(
                    Editor::new(&self.content)
                        .size_full()
                        .font_family(cx.theme().mono_font_family.clone()),
                ),
            )
    }
}

fn text_format_options(cx: &App) -> TextFormatOptions {
    let i18n = cx.global::<I18n>();
    TEXT_BODY_FORMATS
        .into_iter()
        .map(|value| TextFormatOption {
            value,
            title: i18n.t(text_format_i18n_key(value)).into(),
        })
        .collect()
}

const fn text_format_i18n_key(format: TextBodyFormat) -> &'static str {
    match format {
        TextBodyFormat::PlainText => "text-format-plain",
        TextBodyFormat::Json => "text-format-json",
        TextBodyFormat::JavaScript => "text-format-javascript",
        TextBodyFormat::Html => "text-format-html",
        TextBodyFormat::Xml => "text-format-xml",
        TextBodyFormat::Css => "text-format-css",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_format_selector_has_only_supported_fixed_formats() {
        assert_eq!(
            TEXT_BODY_FORMATS,
            [
                TextBodyFormat::PlainText,
                TextBodyFormat::Json,
                TextBodyFormat::JavaScript,
                TextBodyFormat::Html,
                TextBodyFormat::Xml,
                TextBodyFormat::Css,
            ]
        );
        assert_eq!(
            TEXT_BODY_FORMATS.map(TextBodyFormat::media_type),
            [
                "text/plain",
                "application/json",
                "application/javascript",
                "text/html",
                "application/xml",
                "text/css",
            ]
        );
    }
}
