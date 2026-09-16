use super::*;
use crate::state::keybindings::{self, COMMANDS};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::prelude::FluentBuilder;

pub(super) struct KeysView {
    controller: Entity<ConfigController>,
    input: Entity<InputState>,
    editing: Option<usize>,
    error: Option<String>,
    capture: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl KeysView {
    pub fn new(
        controller: Entity<ConfigController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let focus = input.focus_handle(cx);
        let blur = cx.on_focus_out(&focus, window, |this, _, _, cx| {
            this.capture = None;
            cx.notify();
        });
        Self {
            controller,
            input,
            editing: None,
            error: None,
            capture: None,
            _subscriptions: vec![blur],
        }
    }
    fn save(&mut self, index: usize, value: Option<String>, cx: &mut Context<Self>) {
        let command = &COMMANDS[index];
        let config = self.controller.read(cx).preferences(cx);
        match keybindings::validate(
            command.id,
            value.as_deref().unwrap_or(command.default),
            &config.keybindings,
            cx,
        ) {
            Ok(()) => {
                self.controller.update(cx, |owner, cx| {
                    owner.set_preference(PreferenceChange::Keybinding(command.id.into(), value), cx)
                });
                self.editing = None;
                self.error = None;
                self.capture = None;
            }
            Err(error) => {
                self.error = Some(format!(
                    "{}: {}",
                    t(cx, "settings-key-conflict"),
                    t(cx, &error)
                ))
            }
        }
        cx.notify();
    }
    pub fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let command = &COMMANDS[index];
        let config = self.controller.read(cx).preferences(cx);
        let busy = self.controller.read(cx).busy(cx);
        let value = command.value(&config.keybindings);
        let mut row = h_flex()
            .gap_2()
            .child(div().flex_1().child(t(cx, command.label)));
        if self.editing == Some(index) {
            row = row
                .child(Input::new(&self.input).w(px(210.)).disabled(busy))
                .child(
                    Button::new(("key-record", index))
                        .label(t(
                            cx,
                            if self.capture.is_some() {
                                "settings-key-recording"
                            } else {
                                "settings-key-record"
                            },
                        ))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.input.focus_handle(cx).focus(window, cx);
                            let listener =
                                cx.listener(|this, event: &KeystrokeEvent, window, cx| {
                                    cx.stop_propagation();
                                    if event.keystroke.key == "escape" {
                                        this.capture = None;
                                        cx.notify();
                                        return;
                                    }
                                    this.input.update(cx, |input, cx| {
                                        input.set_value(event.keystroke.unparse(), window, cx)
                                    });
                                    this.capture = None;
                                    cx.notify();
                                });
                            this.capture = Some(cx.intercept_keystrokes(listener));
                            cx.notify();
                        })),
                )
                .child(
                    Button::new(("key-save", index))
                        .label(t(cx, "action-confirm"))
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save(index, Some(this.input.read(cx).value().trim().into()), cx)
                        })),
                )
                .child(
                    Button::new(("key-cancel", index))
                        .label(t(cx, "action-cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editing = None;
                            this.error = None;
                            this.capture = None;
                            cx.notify();
                        })),
                );
        } else {
            for stroke in value
                .split_whitespace()
                .filter_map(|s| Keystroke::parse(s).ok())
            {
                row = row.child(Kbd::new(stroke));
            }
            row = row.child(
                Button::new(("key-edit", index))
                    .label(t(cx, "settings-key-edit"))
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let config = this.controller.read(cx).preferences(cx);
                        let value = COMMANDS[index].value(&config.keybindings).to_owned();
                        this.input
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        this.editing = Some(index);
                        this.error = None;
                        cx.notify();
                    })),
            );
        }
        if config.keybindings.contains_key(command.id) {
            row = row.child(
                Button::new(("key-reset", index))
                    .label(t(cx, "settings-key-reset"))
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| this.save(index, None, cx))),
            );
        }
        v_flex()
            .gap_2()
            .child(row)
            .when(self.editing == Some(index), |view| {
                view.children(
                    self.error
                        .as_ref()
                        .map(|error| div().text_color(cx.theme().danger).child(error.clone())),
                )
            })
            .into_any_element()
    }
}
