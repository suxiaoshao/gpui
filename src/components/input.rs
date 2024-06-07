use gpui::*;
use ui::FluentBuilder;

use crate::theme::Theme;

type OnChange = Box<dyn Fn(String, &mut WindowContext) + 'static>;

pub struct Input {
    text: String,
    focus_handle: FocusHandle,
    id: ElementId,
    index: usize,
    on_change: Option<OnChange>,
}

impl Input {
    pub fn new(text: String, id: impl Into<ElementId>, cx: &mut ViewContext<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let index = text.chars().count();
        Input {
            text,
            focus_handle,
            id: id.into(),
            index,
            on_change: None,
        }
    }
    pub fn on_change(mut self, on_change: impl Fn(String, &mut WindowContext) + 'static) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }
}

impl FocusableView for Input {
    fn focus_handle(&self, _cx: &AppContext) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Input {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .id(self.id.clone())
            .border_1()
            .track_focus(&self.focus_handle)
            .bg(theme.input_bg_color())
            .text_color(theme.input_text_color())
            .p_1()
            .on_click(cx.listener(|this, _, cx| {
                this.focus_handle.focus(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, cx| {
                match &event.keystroke.ime_key {
                    Some(key) => {
                        this.text.insert_str(this.index, key);
                        this.index += key.chars().count();
                        if let Some(on_change) = this.on_change.as_ref() {
                            on_change(this.text.clone(), cx)
                        }
                        cx.notify();
                    }
                    None => match event.keystroke.key.as_str() {
                        "backspace" => {
                            if this.index > 0 {
                                this.index -= 1;
                                let mut text = this.text.chars().collect::<Vec<_>>();
                                text.remove(this.index);
                                this.text = text.into_iter().collect();
                                if let Some(on_change) = this.on_change.as_ref() {
                                    on_change(this.text.clone(), cx)
                                }
                                cx.notify();
                            }
                        }
                        "left" => {
                            if this.index > 0 {
                                this.index -= 1;
                                cx.notify();
                            }
                        }
                        "right" => {
                            if this.index < this.text.len() {
                                this.index += 1;
                                cx.notify();
                            }
                        }
                        "enter" => {}
                        _ => {}
                    },
                };
            }))
            .focus(|x| x.border_color(theme.input_focus_border_color()))
            .in_focus(|x| x.border_color(theme.input_border_color()))
            .child(InputElements::new(self))
    }
}

#[derive(Debug, Clone, IntoElement)]
pub struct InputElements {
    input: String,
    index: usize,
}

impl InputElements {
    fn new(input: &Input) -> Self {
        InputElements {
            input: input.text.clone(),
            index: input.index,
        }
    }
}

impl RenderOnce for InputElements {
    fn render(self, cx: &mut WindowContext) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let text = self.input.chars().collect::<Vec<_>>();
        let index = self.index;
        let gap = px(1.0);
        div()
            .flex()
            .flex_row()
            .child(
                div().flex().flex_row().children(
                    text.iter()
                        .take(index)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>(),
                ),
            )
            .child(div().bg(theme.input_cursor_color()).w(gap).relative())
            .child(
                div().flex().flex_row().children(
                    text.iter()
                        .skip(index)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>(),
                ),
            )
            .when(text.is_empty(), |x| x.child(""))
    }
}
