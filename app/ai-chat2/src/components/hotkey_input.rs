use crate::foundation::{I18n, assets::IconName};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
};

actions!(
    ai_chat2_hotkey_input,
    [
        /// Starts recording a hotkey.
        StartRecording,
        /// Stops recording a hotkey.
        StopRecording,
        /// Clears the recorded hotkey.
        ClearHotkey,
    ]
);

const KEY_CONTEXT: &str = "AiChat2HotkeyInput";

pub(crate) struct HotkeyInput {
    id: ElementId,
    style: StyleRefinement,
    size: Size,
    value: Option<Keystroke>,
    outer_focus_handle: FocusHandle,
    capture_focus_handle: FocusHandle,
    intercept_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HotkeyInputEvent {
    Change,
}

impl EventEmitter<HotkeyInputEvent> for HotkeyInput {}

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

pub(crate) fn format_hotkey_label(hotkey: &str) -> String {
    string_to_keystroke(hotkey)
        .map(|keystroke| format_keystroke_label(&keystroke))
        .unwrap_or_else(|| hotkey.to_string())
}

pub(crate) fn string_to_keystroke(string: &str) -> Option<Keystroke> {
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

impl HotkeyInput {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let outer_focus_handle = cx.focus_handle();
        let capture_focus_handle = cx.focus_handle();
        let _subscriptions = vec![
            cx.on_focus_in(&capture_focus_handle, window, Self::on_capture_focus_in),
            cx.on_focus_out(&capture_focus_handle, window, Self::on_capture_focus_out),
        ];
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            size: Size::default(),
            outer_focus_handle,
            capture_focus_handle,
            intercept_subscription: None,
            value: None,
            _subscriptions,
        }
    }

    pub(crate) fn default_value(mut self, default_value: Option<Keystroke>) -> Self {
        self.value = default_value;
        self
    }

    pub(crate) fn current_hotkey_string(&self) -> Option<String> {
        self.value.as_ref().map(keystroke_to_string)
    }

    pub(crate) fn set_hotkey(&mut self, value: Option<Keystroke>, cx: &mut Context<Self>) {
        if self.value == value {
            return;
        }

        self.value = value;
        cx.notify();
    }

    pub(crate) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.outer_focus_handle.focus(window, cx);
    }

    fn on_capture_focus_in(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.intercept_subscription.is_some() {
            return;
        }

        let listener = cx.listener(|this, event: &KeystrokeEvent, window, cx| {
            this.handle_keystroke(event, window, cx);
        });
        self.intercept_subscription = Some(cx.intercept_keystrokes(listener));
        cx.notify();
    }

    fn on_capture_focus_out(
        &mut self,
        _event: FocusOutEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.intercept_subscription.take();
        cx.notify();
    }

    fn handle_keystroke(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !event.keystroke.modifiers.modified() {
            return;
        }

        self.value = Some(event.keystroke.clone());
        self.stop_recording(&StopRecording, window, cx);
        cx.emit(HotkeyInputEvent::Change);
        cx.notify();
    }

    fn start_recording(&mut self, _: &StartRecording, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.capture_focus_handle, cx);
        self.clear_hotkey(&ClearHotkey, window, cx);
        cx.stop_propagation();
    }

    fn stop_recording(&mut self, _: &StopRecording, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_recording(window) {
            window.focus(&self.outer_focus_handle, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn clear_hotkey(&mut self, _: &ClearHotkey, _window: &mut Window, cx: &mut Context<Self>) {
        if self.value.take().is_some() {
            cx.emit(HotkeyInputEvent::Change);
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn is_recording(&self, window: &Window) -> bool {
        self.capture_focus_handle.is_focused(window)
    }
}

impl Focusable for HotkeyInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.outer_focus_handle.clone()
    }
}

impl Render for HotkeyInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_recording = self.is_recording(window);
        let (record_label, stop_label, clear_label, not_set_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("hotkey-action-record"),
                i18n.t("hotkey-action-stop-recording"),
                i18n.t("hotkey-action-clear"),
                i18n.t("hotkey-not-set"),
            )
        };

        h_flex()
            .w_64()
            .items_center()
            .gap_2()
            .px_2()
            .id(self.id.clone())
            .refine_style(&self.style)
            .track_focus(&self.outer_focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::start_recording))
            .on_action(cx.listener(Self::stop_recording))
            .on_action(cx.listener(Self::clear_hotkey))
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
            .when(is_recording, |this| this.border_color(cx.theme().primary))
            .child(
                div()
                    .track_focus(&self.capture_focus_handle)
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|this| match self.size {
                        Size::Small => this.text_sm(),
                        Size::XSmall => this.text_xs(),
                        Size::Large => this.text_base(),
                        Size::Size(size) => this.text_size(size * 0.875),
                        Size::Medium => this.text_sm(),
                    })
                    .text_color(if self.value.is_some() {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .text_center()
                    .overflow_hidden()
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(
                                self.value
                                    .as_ref()
                                    .map(format_keystroke_label)
                                    .unwrap_or_else(|| {
                                        if is_recording {
                                            "REC".to_string()
                                        } else {
                                            not_set_label.to_string()
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .when(is_recording, |this| {
                        this.child(
                            Button::new((self.id.clone(), "stop"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::X)
                                .tooltip(stop_label.clone())
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.stop_recording(&StopRecording, window, cx);
                                })),
                        )
                    })
                    .when(!is_recording, |this| {
                        this.child(
                            Button::new((self.id.clone(), "record"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::Keyboard)
                                .tooltip(record_label.clone())
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.start_recording(&StartRecording, window, cx);
                                })),
                        )
                    })
                    .when(self.value.is_some(), |this| {
                        this.child(
                            Button::new((self.id.clone(), "clear"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::Trash)
                                .tooltip(clear_label.clone())
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.clear_hotkey(&ClearHotkey, window, cx);
                                })),
                        )
                    }),
            )
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
    use super::{
        ClearHotkey, HotkeyInput, StartRecording, format_hotkey_label, string_to_keystroke,
    };
    use gpui::{
        AppContext as _, KeyContext, Keystroke, KeystrokeEvent, Modifiers, TestAppContext,
        VisualTestContext, WindowHandle,
    };

    #[test]
    fn string_to_keystroke_accepts_plus_format_only() {
        assert!(string_to_keystroke("super+shift+k").is_some());
        assert!(string_to_keystroke("cmd+shift+k").is_some());
        assert!(string_to_keystroke("cmd-shift-k").is_none());
    }

    #[test]
    fn format_hotkey_label_falls_back_to_raw_text() {
        assert_eq!(format_hotkey_label("cmd-shift-k"), "cmd-shift-k");
    }

    #[gpui::test]
    fn recorder_captures_modified_hotkey_and_clear_is_local(cx: &mut TestAppContext) {
        let window = open_hotkey_input_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let input = window.root(&mut cx).expect("hotkey input");

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.start_recording(&StartRecording, window, cx);
                input.handle_keystroke(
                    &keystroke_event(Keystroke {
                        modifiers: Modifiers {
                            shift: true,
                            platform: true,
                            ..Default::default()
                        },
                        key: "j".to_string(),
                        key_char: Some("j".to_string()),
                    }),
                    window,
                    cx,
                );
            });
        });

        assert_eq!(
            input.read_with(&cx, |input, _| input.current_hotkey_string()),
            Some("shift+super+j".to_string())
        );

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.clear_hotkey(&ClearHotkey, window, cx);
            });
        });

        assert_eq!(
            input.read_with(&cx, |input, _| input.current_hotkey_string()),
            None
        );
    }

    #[gpui::test]
    fn recorder_ignores_plain_keys(cx: &mut TestAppContext) {
        let window = open_hotkey_input_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let input = window.root(&mut cx).expect("hotkey input");

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.start_recording(&StartRecording, window, cx);
                input.handle_keystroke(
                    &keystroke_event(Keystroke {
                        modifiers: Modifiers::default(),
                        key: "j".to_string(),
                        key_char: Some("j".to_string()),
                    }),
                    window,
                    cx,
                );
            });
        });

        assert_eq!(
            input.read_with(&cx, |input, _| input.current_hotkey_string()),
            None
        );
    }

    fn open_hotkey_input_window(cx: &mut TestAppContext) -> WindowHandle<HotkeyInput> {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::foundation::init_i18n(cx);
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| HotkeyInput::new("test-hotkey-input", window, cx))
            })
            .expect("open window")
        })
    }

    fn keystroke_event(keystroke: Keystroke) -> KeystrokeEvent {
        KeystrokeEvent {
            keystroke,
            action: None,
            context_stack: vec![KeyContext::default()],
        }
    }
}
