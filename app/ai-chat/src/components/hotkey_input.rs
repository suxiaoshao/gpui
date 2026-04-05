use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
};

pub(crate) struct HotkeyInput {
    id: ElementId,
    style: StyleRefinement,
    size: Size,
    value: Option<Keystroke>,
    default_value: Option<Keystroke>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
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

fn format_keystroke_label(keystroke: &Keystroke) -> String {
    #[cfg(target_os = "macos")]
    const DIVIDER: &str = "";
    #[cfg(not(target_os = "macos"))]
    const DIVIDER: &str = "+";

    let mut parts = vec![];

    if keystroke.modifiers.control {
        #[cfg(target_os = "macos")]
        parts.push("⌃".to_string());
        #[cfg(not(target_os = "macos"))]
        parts.push("Ctrl".to_string());
    }

    if keystroke.modifiers.alt {
        #[cfg(target_os = "macos")]
        parts.push("⌥".to_string());
        #[cfg(not(target_os = "macos"))]
        parts.push("Alt".to_string());
    }

    if keystroke.modifiers.shift {
        #[cfg(target_os = "macos")]
        parts.push("⇧".to_string());
        #[cfg(not(target_os = "macos"))]
        parts.push("Shift".to_string());
    }

    if keystroke.modifiers.platform {
        #[cfg(target_os = "macos")]
        parts.push("⌘".to_string());
        #[cfg(not(target_os = "macos"))]
        parts.push("Win".to_string());
    }

    let key = match keystroke.key.as_str() {
        #[cfg(target_os = "macos")]
        "ctrl" => "⌃".to_string(),
        #[cfg(not(target_os = "macos"))]
        "ctrl" => "Ctrl".to_string(),
        #[cfg(target_os = "macos")]
        "alt" => "⌥".to_string(),
        #[cfg(not(target_os = "macos"))]
        "alt" => "Alt".to_string(),
        #[cfg(target_os = "macos")]
        "shift" => "⇧".to_string(),
        #[cfg(not(target_os = "macos"))]
        "shift" => "Shift".to_string(),
        #[cfg(target_os = "macos")]
        "cmd" => "⌘".to_string(),
        #[cfg(not(target_os = "macos"))]
        "cmd" => "Win".to_string(),
        "space" => "Space".to_string(),
        #[cfg(target_os = "macos")]
        "backspace" | "delete" => "⌫".to_string(),
        #[cfg(not(target_os = "macos"))]
        "backspace" => "Backspace".to_string(),
        #[cfg(not(target_os = "macos"))]
        "delete" => "Delete".to_string(),
        #[cfg(target_os = "macos")]
        "escape" => "⎋".to_string(),
        #[cfg(not(target_os = "macos"))]
        "escape" => "Esc".to_string(),
        #[cfg(target_os = "macos")]
        "enter" => "⏎".to_string(),
        #[cfg(not(target_os = "macos"))]
        "enter" => "Enter".to_string(),
        "pagedown" => "Page Down".to_string(),
        "pageup" => "Page Up".to_string(),
        #[cfg(target_os = "macos")]
        "left" => "←".to_string(),
        #[cfg(not(target_os = "macos"))]
        "left" => "Left".to_string(),
        #[cfg(target_os = "macos")]
        "right" => "→".to_string(),
        #[cfg(not(target_os = "macos"))]
        "right" => "Right".to_string(),
        #[cfg(target_os = "macos")]
        "up" => "↑".to_string(),
        #[cfg(not(target_os = "macos"))]
        "up" => "Up".to_string(),
        #[cfg(target_os = "macos")]
        "down" => "↓".to_string(),
        #[cfg(not(target_os = "macos"))]
        "down" => "Down".to_string(),
        key if key.len() == 1 => key.to_uppercase(),
        key => {
            let mut chars = key.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.collect::<String>()),
                None => String::new(),
            }
        }
    };

    parts.push(key);
    parts.join(DIVIDER)
}

pub fn string_to_keystroke(string: &str) -> Option<Keystroke> {
    if string.contains('-') && !string.contains('+') {
        return None;
    }

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
    pub(crate) fn new(
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let _subscriptions = vec![cx.on_blur(&focus_handle, window, Self::on_blur)];
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            size: Size::default(),
            focus_handle,
            value: None,
            _subscriptions,
            default_value: None,
        }
    }
    pub(crate) fn default_value(self, default_value: Option<Keystroke>) -> Self {
        Self {
            id: self.id,
            style: self.style,
            size: self.size,
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
            .w_64()
            .items_center()
            .relative()
            .id(self.id.clone())
            .refine_style(&self.style)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.modifiers.modified() {
                    this.value = Some(event.keystroke.clone());
                    cx.notify();
                }
            }))
            .map(|this| match self.size {
                Size::Large => this.h_11(),
                Size::Medium => this.h_8(),
                Size::Small => this.h_6(),
                Size::XSmall => this.h_5(),
                Size::Size(size) => this.h(size),
            })
            .line_height(relative(1.))
            .bg(cx.theme().background)
            .rounded(cx.theme().radius)
            .border_color(cx.theme().input)
            .border_1()
            .when(cx.theme().shadow, |this| this.shadow_xs())
            .focus(|this| this.focused_border(cx))
            .when_some(self.value.as_ref(), |this, value| {
                this.child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|this| match self.size {
                            Size::Small => this.px_2().text_sm(),
                            Size::XSmall => this.px_2().text_xs(),
                            Size::Large => this.px_3().text_base(),
                            Size::Size(size) => this.px(size * 0.25).text_size(size * 0.875),
                            Size::Medium => this.px_3().text_sm(),
                        })
                        .text_color(cx.theme().foreground)
                        .text_center()
                        .overflow_hidden()
                        .child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(format_keystroke_label(value)),
                        ),
                )
            })
            .when(self.focus_handle.is_focused(window), |this| {
                this.child(
                    h_flex()
                        .h_full()
                        .items_center()
                        .gap_1()
                        .absolute()
                        .right_2()
                        .child(
                            Button::new((self.id.clone(), "cancel"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|_this, _event, window, cx| {
                                    cx.emit(HotkeyEvent::Cancel);
                                    window.focus_next();
                                })),
                        )
                        .child(
                            Button::new((self.id.clone(), "confirm"))
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

impl Styled for HotkeyInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for HotkeyInput {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::string_to_keystroke;

    #[test]
    fn string_to_keystroke_accepts_plus_format_only() {
        assert!(string_to_keystroke("super+shift+k").is_some());
        assert!(string_to_keystroke("cmd+shift+k").is_some());
        assert!(string_to_keystroke("cmd-shift-k").is_none());
    }
}
