use gpui::*;
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent, InputState},
    select::SelectItem,
};

use crate::http_body::{HttpBodyEvent, HttpBodyForm};

#[derive(Default, Clone, Copy)]
pub enum TextType {
    #[default]
    Plaintext,
    Json,
    Html,
    Xml,
    Javascript,
    Css,
}

impl TextType {
    fn language(&self) -> &'static str {
        match self {
            TextType::Plaintext => "plaintext",
            TextType::Json => "json",
            TextType::Html => "html",
            TextType::Xml => "xml",
            TextType::Javascript => "javascript",
            TextType::Css => "css",
        }
    }
}

impl SelectItem for TextType {
    type Value = TextType;

    fn title(&self) -> SharedString {
        match self {
            TextType::Plaintext => "Plain Text".into(),
            TextType::Json => "JSON".into(),
            TextType::Html => "HTML".into(),
            TextType::Xml => "XML".into(),
            TextType::Javascript => "JavaScript".into(),
            TextType::Css => "CSS".into(),
        }
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

#[derive(Default, Clone)]
pub struct HttpText {
    pub(crate) text: String,
    pub(crate) text_type: TextType,
}

pub struct HttpTextView {
    form: Entity<HttpBodyForm>,
    input_state: Entity<InputState>,
    _subscription: Vec<Subscription>,
}

impl HttpTextView {
    pub(crate) fn new(
        form: Entity<HttpBodyForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = form.read(cx).text.text.to_string();
        let text_type = form.read(cx).text.text_type;
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .code_editor(text_type.language()) // Language for syntax highlighting
                .line_number(true) // Show line numbers
                .searchable(true) // Enable search functionality
                .default_value(text)
        });
        let _subscription = vec![
            cx.subscribe_in(&form, window, Self::subcription_in),
            cx.subscribe_in(&input_state, window, |this, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    let text = state.read(cx).value().to_string();
                    this.form.update(cx, |_form, cx| {
                        cx.emit(HttpBodyEvent::SetText(text));
                    });
                }
            }),
        ];
        Self {
            form,
            input_state,
            _subscription,
        }
    }
    fn subcription_in(
        &mut self,
        _subscriber: &Entity<HttpBodyForm>,
        emitter: &HttpBodyEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let HttpBodyEvent::SetTextType(text_type) = emitter {
            self.input_state.update(cx, |state, cx| {
                state.set_highlighter(text_type.language(), cx);
            });
        }
    }
}

impl Render for HttpTextView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().p_2().flex_1().child(
            Input::new(&self.input_state)
                .h_full()
                .font_family(cx.theme().mono_font_family.clone()),
        )
    }
}
