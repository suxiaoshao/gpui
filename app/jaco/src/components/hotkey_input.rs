use crate::foundation::{I18n, assets::IconName};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Sizable, Size, StyledExt, ThemeStyled,
    button::{Button, ButtonVariants},
    h_flex,
};

actions!(
    jaco_hotkey_input,
    [
        /// Starts recording a hotkey.
        StartRecording,
        /// Stops recording a hotkey.
        StopRecording,
        /// Clears the recorded hotkey.
        ClearHotkey,
    ]
);

const KEY_CONTEXT: &str = "JacoHotkeyInput";

pub(crate) struct HotkeyInputState {
    outer_focus_handle: FocusHandle,
    capture_focus_handle: FocusHandle,
    intercept_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HotkeyInputEvent {
    Change(Option<String>),
}

impl EventEmitter<HotkeyInputEvent> for HotkeyInputState {}

#[derive(IntoElement)]
pub(crate) struct HotkeyInput {
    state: Entity<HotkeyInputState>,
    id: ElementId,
    style: StyleRefinement,
    size: Size,
    value: Option<Keystroke>,
}

pub(crate) fn keystroke_to_string(keystroke: &Keystroke) -> String {
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

impl HotkeyInputState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let outer_focus_handle = cx.focus_handle();
        let capture_focus_handle = cx.focus_handle();
        let _subscriptions = vec![
            cx.on_focus_in(&capture_focus_handle, window, Self::on_capture_focus_in),
            cx.on_focus_out(&capture_focus_handle, window, Self::on_capture_focus_out),
        ];
        Self {
            outer_focus_handle,
            capture_focus_handle,
            intercept_subscription: None,
            _subscriptions,
        }
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

        self.stop_recording(&StopRecording, window, cx);
        cx.emit(HotkeyInputEvent::Change(Some(keystroke_to_string(
            &event.keystroke,
        ))));
        cx.notify();
    }

    fn start_recording(
        &mut self,
        had_value: bool,
        _: &StartRecording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.capture_focus_handle, cx);
        self.clear_hotkey(had_value, &ClearHotkey, window, cx);
        cx.stop_propagation();
    }

    fn stop_recording(&mut self, _: &StopRecording, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_recording(window) {
            window.focus(&self.outer_focus_handle, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn clear_hotkey(
        &mut self,
        had_value: bool,
        _: &ClearHotkey,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if had_value {
            cx.emit(HotkeyInputEvent::Change(None));
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn is_recording(&self, window: &Window) -> bool {
        self.capture_focus_handle.is_focused(window)
    }
}

impl HotkeyInput {
    pub(crate) fn new(id: impl Into<ElementId>, state: &Entity<HotkeyInputState>) -> Self {
        Self {
            state: state.clone(),
            id: id.into(),
            style: StyleRefinement::default(),
            size: Size::default(),
            value: None,
        }
    }

    pub(crate) fn value(mut self, value: Option<Keystroke>) -> Self {
        self.value = value;
        self
    }
}

impl Focusable for HotkeyInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).outer_focus_handle.clone()
    }
}

impl View for HotkeyInput {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_recording = self.state.read(cx).is_recording(window);
        let (record_label, stop_label, clear_label, not_set_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("hotkey-action-record"),
                i18n.t("hotkey-action-stop-recording"),
                i18n.t("hotkey-action-clear"),
                i18n.t("hotkey-not-set"),
            )
        };
        let outer_focus_handle = self.state.read(cx).outer_focus_handle.clone();
        let capture_focus_handle = self.state.read(cx).capture_focus_handle.clone();
        let is_focused =
            outer_focus_handle.is_focused(window) || capture_focus_handle.is_focused(window);
        let had_value = self.value.is_some();
        let start_action_state = self.state.clone();
        let stop_action_state = self.state.clone();
        let clear_action_state = self.state.clone();
        let start_button_state = self.state.clone();
        let stop_button_state = self.state.clone();
        let clear_button_state = self.state.clone();

        h_flex()
            .w_64()
            .items_center()
            .gap_2()
            .px_2()
            .id(self.id.clone())
            .refine_style(&self.style)
            .track_focus(&outer_focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(move |action: &StartRecording, window, cx| {
                start_action_state.update(cx, |state, cx| {
                    state.start_recording(had_value, action, window, cx);
                });
            })
            .on_action(move |action: &StopRecording, window, cx| {
                stop_action_state.update(cx, |state, cx| {
                    state.stop_recording(action, window, cx);
                });
            })
            .on_action(move |action: &ClearHotkey, window, cx| {
                clear_action_state.update(cx, |state, cx| {
                    state.clear_hotkey(had_value, action, window, cx);
                });
            })
            .map(|this| match self.size {
                Size::Large => this.h_11(),
                Size::Medium => this.h_8(),
                Size::Small => this.h_6(),
                Size::XSmall => this.h_5(),
                Size::Size(size) => this.h(size),
            })
            .line_height(relative(1.))
            .bg(cx.theme().tokens.background.background)
            .rounded(cx.theme().radius)
            .border_color(cx.theme().input)
            .border_1()
            .when(cx.theme().shadow, |this| this.shadow_xs())
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .when(is_recording, |this| this.border_color(cx.theme().primary))
            .child(
                div()
                    .track_focus(&capture_focus_handle)
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
                                .on_click(move |_event, window, cx| {
                                    stop_button_state.update(cx, |state, cx| {
                                        state.stop_recording(&StopRecording, window, cx);
                                    });
                                }),
                        )
                    })
                    .when(!is_recording, |this| {
                        this.child(
                            Button::new((self.id.clone(), "record"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::Keyboard)
                                .tooltip(record_label.clone())
                                .on_click(move |_event, window, cx| {
                                    start_button_state.update(cx, |state, cx| {
                                        state.start_recording(
                                            had_value,
                                            &StartRecording,
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        )
                    })
                    .when(self.value.is_some(), |this| {
                        this.child(
                            Button::new((self.id.clone(), "clear"))
                                .xsmall()
                                .ghost()
                                .icon(IconName::Trash)
                                .tooltip(clear_label.clone())
                                .on_click(move |_event, window, cx| {
                                    clear_button_state.update(cx, |state, cx| {
                                        state.clear_hotkey(had_value, &ClearHotkey, window, cx);
                                    });
                                }),
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
        ClearHotkey, HotkeyInput, HotkeyInputEvent, HotkeyInputState, StartRecording,
        format_hotkey_label, keystroke_to_string, string_to_keystroke,
    };
    use gpui::{
        AppContext as _, Context, Entity, IntoElement, KeyContext, Keystroke, KeystrokeEvent,
        Modifiers, Render, Subscription, TestAppContext, View, VisualTestContext, Window,
        WindowHandle,
    };

    struct HotkeyInputHarness {
        input: Entity<HotkeyInputState>,
        value: Option<Keystroke>,
        _subscription: Subscription,
    }

    impl HotkeyInputHarness {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let input = cx.new(|cx| HotkeyInputState::new(window, cx));
            let _subscription =
                cx.subscribe(&input, |this, _input, event: &HotkeyInputEvent, cx| {
                    let HotkeyInputEvent::Change(value) = event;
                    this.value = value.as_deref().and_then(string_to_keystroke);
                    cx.notify();
                });
            Self {
                input,
                value: None,
                _subscription,
            }
        }

        fn view(&self) -> HotkeyInput {
            HotkeyInput::new("test-hotkey-input", &self.input).value(self.value.clone())
        }

        fn current_hotkey_string(&self) -> Option<String> {
            self.value.as_ref().map(keystroke_to_string)
        }
    }

    impl Render for HotkeyInputHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.view()
        }
    }

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
    fn recorder_captures_modified_hotkey_and_clear_emits_value_changes(cx: &mut TestAppContext) {
        let window = open_hotkey_input_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let harness = window.root(&mut cx).expect("hotkey input harness");
        let input = harness.read_with(&cx, |harness, _| harness.input.clone());

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.start_recording(false, &StartRecording, window, cx);
            });
        });
        cx.update(|window, cx| {
            assert!(input.read(cx).is_recording(window));
            input.update(cx, |input, cx| {
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
        cx.update(|window, cx| {
            assert!(!input.read(cx).is_recording(window));
        });

        assert_eq!(
            harness.read_with(&cx, |harness, _| harness.current_hotkey_string()),
            Some("shift+super+j".to_string())
        );

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.clear_hotkey(true, &ClearHotkey, window, cx);
            });
        });

        assert_eq!(
            harness.read_with(&cx, |harness, _| harness.current_hotkey_string()),
            None
        );
    }

    #[gpui::test]
    fn recorder_ignores_plain_keys(cx: &mut TestAppContext) {
        let window = open_hotkey_input_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let harness = window.root(&mut cx).expect("hotkey input harness");
        let input = harness.read_with(&cx, |harness, _| harness.input.clone());

        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.start_recording(false, &StartRecording, window, cx);
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
            harness.read_with(&cx, |harness, _| harness.current_hotkey_string()),
            None
        );
    }

    #[gpui::test]
    fn view_uses_backing_identity_across_rebuilds(cx: &mut TestAppContext) {
        let window = open_hotkey_input_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let harness = window.root(&mut cx).expect("hotkey input harness");
        let state = harness.read_with(&cx, |harness, _| harness.input.clone());

        let first = HotkeyInput::new("first-hotkey-input", &state);
        let rebuilt = HotkeyInput::new("rebuilt-hotkey-input", &state).value(Some(Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Default::default()
            },
            key: "k".to_string(),
            key_char: Some("k".to_string()),
        }));

        assert_eq!(View::entity_id(&first), Some(state.entity_id()));
        assert_eq!(View::entity_id(&rebuilt), Some(state.entity_id()));
    }

    fn open_hotkey_input_window(cx: &mut TestAppContext) -> WindowHandle<HotkeyInputHarness> {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::foundation::init_i18n(cx);
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| HotkeyInputHarness::new(window, cx))
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
