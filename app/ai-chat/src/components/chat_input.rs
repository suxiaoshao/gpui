use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputState},
};

const CONTEXT: &str = "chat-form";

actions!([Send]);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("shift-enter", Send, Some(CONTEXT))]);
}

#[derive(IntoElement)]
pub(crate) struct ChatInput {
    base: Div,
    input_state: Entity<InputState>,
    disabled: bool,
}

pub(crate) fn input_state(window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).multi_line(true).auto_grow(3, 8))
}

impl ChatInput {
    pub(crate) fn new(input_state: &Entity<InputState>) -> Self {
        Self {
            input_state: input_state.clone(),
            base: div().key_context(CONTEXT),
            disabled: false,
        }
    }
    pub(crate) fn disabled(self, disabled: bool) -> Self {
        Self {
            base: self.base,
            input_state: self.input_state,
            disabled,
        }
    }
}

impl RenderOnce for ChatInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text = self.input_state.read(cx).value();
        let disable = text.is_empty() || self.disabled;
        self.base
            .bg(cx.theme().input)
            .rounded(cx.theme().radius)
            .child(
                Input::new(&self.input_state)
                    .bordered(false)
                    .bg(cx.theme().transparent),
            )
            .child(
                h_flex().w_full().p_1().child(div().flex_1()).child(
                    Button::new("send")
                        .disabled(disable)
                        .icon(IconName::ArrowUp)
                        .small(),
                ),
            )
    }
}

impl InteractiveElement for ChatInput {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}
