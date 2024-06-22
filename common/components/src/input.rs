use std::time::Duration;

use gpui::{prelude::*, *};
use unicode_segmentation::UnicodeSegmentation;

use theme::Theme;

type OnChange = Box<dyn Fn(&String, &mut WindowContext) + 'static>;

pub struct Input {
    text: String,
    focus_handle: FocusHandle,
    index: usize,
    on_change: Option<OnChange>,
}

impl Input {
    pub fn new(text: String, cx: &mut ViewContext<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let index = text.graphemes(true).count();
        Input {
            text,
            focus_handle,
            index,
            on_change: None,
        }
    }
    pub fn on_change(mut self, on_change: impl Fn(&String, &mut WindowContext) + 'static) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }
    pub fn get_value(&self) -> &str {
        &self.text
    }
}

impl FocusableView for Input {
    fn focus_handle(&self, _cx: &AppContext) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Input {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .px_2()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, cx| {
                match &event.keystroke.ime_key {
                    Some(key) => {
                        this.text.insert_str(this.index, key);
                        this.index += key.graphemes(true).count();
                        if let Some(on_change) = this.on_change.as_ref() {
                            on_change(&this.text, cx)
                        }
                        cx.notify();
                    }
                    None => match event.keystroke.key.as_str() {
                        "backspace" => {
                            if this.index > 0 {
                                this.index -= 1;
                                let mut text = this.text.graphemes(true).collect::<Vec<_>>();
                                text.remove(this.index);
                                this.text = text.into_iter().collect();
                                if let Some(on_change) = this.on_change.as_ref() {
                                    on_change(&this.text, cx)
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
                            if this.index < this.text.graphemes(true).count() {
                                this.index += 1;
                                cx.notify();
                            }
                        }
                        "enter" => {}
                        "q" | "w" | "e" | "r" | "t" | "y" | "u" | "i" | "o" | "p" | "a" | "s"
                        | "d" | "f" | "g" | "h" | "j" | "k" | "l" | "z" | "x" | "c" | "v" | "b"
                        | "n" | "m"
                            if !(event.keystroke.modifiers.alt
                                || event.keystroke.modifiers.control
                                || event.keystroke.modifiers.function
                                || event.keystroke.modifiers.platform
                                || event.keystroke.modifiers.shift) =>
                        {
                            dbg!(event.keystroke.key.as_str());
                        }
                        "v" if event.keystroke.modifiers.platform
                            || event.keystroke.modifiers.control =>
                        {
                            let text = cx.read_from_clipboard();
                            if let Some(text) = text {
                                let text = text.text();
                                this.text.insert_str(this.index, text);
                                this.index += text.graphemes(true).count();
                                if let Some(on_change) = this.on_change.as_ref() {
                                    on_change(&this.text, cx)
                                }
                                cx.notify();
                            }
                        }
                        _ => {}
                    },
                };
            }))
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
        let text = self.input.graphemes(true).collect::<Vec<_>>();
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
            .cursor_text()
            .when(text.is_empty(), |x| x.child(""))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(String::from_iter(text.iter().take(index).copied())),
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
                div()
                    .flex()
                    .flex_row()
                    .child(String::from_iter(text.iter().skip(index).copied())),
            )
    }
}
