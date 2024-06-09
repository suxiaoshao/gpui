use std::time::Duration;

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
            .child(InputElements::new(self, cx))
    }
}

#[derive(Debug, Clone, IntoElement)]
pub struct InputElements {
    input: String,
    index: usize,
    focus: bool,
}

impl InputElements {
    fn new(input: &Input, cx: &WindowContext) -> Self {
        InputElements {
            input: input.text.clone(),
            index: input.index,
            focus: input.focus_handle.is_focused(cx),
        }
    }
}

impl RenderOnce for InputElements {
    fn render(self, cx: &mut WindowContext) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let text = self.input.chars().collect::<Vec<_>>();
        let index = self.index;
        let gap = px(1.0);
        let font_size = cx.text_style().font_size;
        let rem_size = cx.rem_size();
        let font_size = font_size.to_pixels(rem_size);
        let cursor_color = theme.input_cursor_color();

        div()
            .flex()
            .flex_row()
            .items_center()
            .when(text.is_empty(), |x| x.child(""))
            .child(
                div().flex().flex_row().children(
                    text.iter()
                        .take(index)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>(),
                ),
            )
            .when(self.focus, |x| {
                x.child(
                    div().bg(cursor_color).w(gap).h(font_size).with_animation(
                        "input_cursor",
                        Animation::new(Duration::from_secs(1))
                            .repeat()
                            .with_easing(bounce(ease_in_out)),
                        move |cursor, delate| {
                            let mut color = cursor_color;
                            let delate = match delate {
                                0.75..=1.0 => 1.0,
                                _ => delate * 4.0 / 3.0,
                            };
                            color.a = delate;
                            cursor.bg(color)
                        },
                    ),
                )
            })
            .child(
                div().flex().flex_row().children(
                    text.iter()
                        .skip(index)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>(),
                ),
            )
    }
}
