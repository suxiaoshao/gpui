use crate::i18n::I18n;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputState},
    select::{SearchableVec, Select, SelectState},
};

const CONTEXT: &str = "chat-form";

actions!([Send, Pause]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubmitButtonMode {
    Send,
    Pause,
}

fn submit_button_mode(text_is_empty: bool, running: bool) -> (SubmitButtonMode, bool) {
    if running {
        (SubmitButtonMode::Pause, false)
    } else {
        (SubmitButtonMode::Send, text_is_empty)
    }
}

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("shift-enter", Send, Some(CONTEXT))]);
}

#[derive(IntoElement)]
pub(crate) struct ChatInput {
    base: Div,
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<SearchableVec<String>>>,
    running: bool,
}

pub(crate) fn input_state(window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).multi_line(true).auto_grow(3, 8))
}

impl ChatInput {
    pub(crate) fn new(
        input_state: &Entity<InputState>,
        extension_state: &Entity<SelectState<SearchableVec<String>>>,
    ) -> Self {
        Self {
            input_state: input_state.clone(),
            extension_state: extension_state.clone(),
            base: div().key_context(CONTEXT),
            running: false,
        }
    }
    pub(crate) fn running(self, running: bool) -> Self {
        Self {
            base: self.base,
            input_state: self.input_state,
            extension_state: self.extension_state,
            running,
        }
    }
}

impl RenderOnce for ChatInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text = self.input_state.read(cx).value();
        let (button_mode, button_disabled) = submit_button_mode(text.is_empty(), self.running);
        let button_tooltip = {
            let i18n = cx.global::<I18n>();
            match button_mode {
                SubmitButtonMode::Send => i18n.t("tooltip-send-message"),
                SubmitButtonMode::Pause => i18n.t("tooltip-pause-message"),
            }
        };
        self.base
            .bg(cx.theme().input)
            .rounded(cx.theme().radius)
            .child(
                Input::new(&self.input_state)
                    .bordered(false)
                    .bg(cx.theme().transparent),
            )
            .child(
                h_flex()
                    .w_full()
                    .p_1()
                    .child(div().flex_1())
                    .child(
                        Select::new(&self.extension_state)
                            .cleanable(true)
                            .w(px(150.)),
                    )
                    .child(match button_mode {
                        SubmitButtonMode::Send => Button::new("send")
                            .disabled(button_disabled)
                            .icon(IconName::ArrowUp)
                            .small()
                            .tooltip(button_tooltip)
                            .on_click(|_event, window, cx| {
                                window.dispatch_action(Send.boxed_clone(), cx);
                            })
                            .into_any_element(),
                        SubmitButtonMode::Pause => Button::new("pause")
                            .disabled(button_disabled)
                            .icon(IconName::Close)
                            .small()
                            .tooltip(button_tooltip)
                            .on_click(|_event, window, cx| {
                                window.dispatch_action(Pause.boxed_clone(), cx);
                            })
                            .into_any_element(),
                    }),
            )
    }
}

impl InteractiveElement for ChatInput {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

#[cfg(test)]
mod tests {
    use super::{SubmitButtonMode, submit_button_mode};

    #[test]
    fn submit_button_mode_uses_pause_when_running() {
        assert_eq!(
            submit_button_mode(true, true),
            (SubmitButtonMode::Pause, false)
        );
        assert_eq!(
            submit_button_mode(false, true),
            (SubmitButtonMode::Pause, false)
        );
    }

    #[test]
    fn submit_button_mode_disables_send_without_text() {
        assert_eq!(
            submit_button_mode(true, false),
            (SubmitButtonMode::Send, true)
        );
        assert_eq!(
            submit_button_mode(false, false),
            (SubmitButtonMode::Send, false)
        );
    }
}
