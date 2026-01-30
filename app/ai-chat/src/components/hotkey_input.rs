use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    kbd::Kbd,
};

pub(crate) struct HotkeyInput {
    value: Option<Keystroke>,
    default_value: Option<Keystroke>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

pub(crate) enum HotkeyEvent {
    Confirm(SharedString),
    Cancel,
}

fn keystroke_to_string(keystroke: &Keystroke) -> String {
    let mut result = String::new();
    if keystroke.modifiers.control {
        result.push_str("ctrl+");
    }
    if keystroke.modifiers.alt {
        result.push_str("alt+");
    }
    if keystroke.modifiers.shift {
        result.push_str("shift+");
    }
    if keystroke.modifiers.platform {
        result.push_str("super+");
    }
    result.push_str(&keystroke.key.to_string());
    result
}

pub fn string_to_keystroke(string: &str) -> Option<Keystroke> {
    let mut modifiers = Modifiers::none();
    let mut key = None;

    for part in string.split('+') {
        match part {
            "ctrl" | "control" => {
                modifiers.control = true;
            }
            "alt" | "option" => {
                modifiers.alt = true;
            }
            "shift" => {
                modifiers.shift = true;
            }
            "super" | "cmd" | "command" => {
                modifiers.platform = true;
            }
            _ => key = Some(part.parse().ok()?),
        }
    }

    key.map(|key: String| Keystroke {
        modifiers,
        key_char: Some(key.clone()),
        key,
    })
}

impl EventEmitter<HotkeyEvent> for HotkeyInput {}

impl HotkeyInput {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let _subscriptions = vec![cx.on_blur(&focus_handle, window, Self::on_blur)];
        Self {
            focus_handle,
            value: None,
            _subscriptions,
            default_value: None,
        }
    }
    pub(crate) fn default_value(self, default_value: Option<Keystroke>) -> Self {
        Self {
            focus_handle: self.focus_handle,
            value: default_value.clone(),
            _subscriptions: self._subscriptions,
            default_value,
        }
    }
    fn on_blur(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        match self.value.as_ref() {
            Some(value) => cx.emit(HotkeyEvent::Confirm(keystroke_to_string(value).into())),
            None => cx.emit(HotkeyEvent::Cancel),
        };
    }
    pub(crate) fn set_default_value(&mut self, default_value: Option<Keystroke>) {
        self.default_value = default_value.clone();
        self.value = default_value;
    }
}

impl Render for HotkeyInput {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        h_flex()
            .justify_center()
            .relative()
            .id("hotkey-input")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.modifiers.modified() {
                    this.value = Some(event.keystroke.clone());
                    cx.notify();
                }
            }))
            .h_8()
            .line_height(rems(1.25))
            .w_64()
            .rounded(cx.theme().radius)
            .border_color(cx.theme().input)
            .border_1()
            .when(cx.theme().shadow, |this| this.shadow_xs())
            .focus(|this| this.focused_border(cx))
            .when_some(self.value.as_ref(), |this, value| {
                this.child(Kbd::new(value.clone()))
            })
            .when(self.focus_handle.is_focused(window), |this| {
                this.child(
                    h_flex()
                        .gap_1()
                        .absolute()
                        .right_2()
                        .child(
                            Button::new("cancel")
                                .xsmall()
                                .ghost()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|_this, _event, window, cx| {
                                    cx.emit(HotkeyEvent::Cancel);
                                    window.focus_next();
                                })),
                        )
                        .child(
                            Button::new("confirm")
                                .xsmall()
                                .ghost()
                                .icon(IconName::Check)
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.on_blur(window, cx);
                                    window.focus_next();
                                })),
                        ),
                )
            })
    }
}
