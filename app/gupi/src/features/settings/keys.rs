use super::*;
use crate::{
    features::home::actions::Kind,
    state::keybindings::{self, COMMANDS},
};
use gpui_kit::component::{
    Selectable, Sizable, WindowExt,
    input::Escape,
    setting::{RenderOptions, SettingField, SettingGroup, SettingItem},
};
use gpui_kit::prelude::FluentBuilder;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Application,
    Conversation,
    Files,
    Temporary,
}
impl Group {
    pub fn of(kind: Kind) -> Self {
        if kind.temporary_only() {
            return Self::Temporary;
        }
        match kind {
            Kind::Palette
            | Kind::QuickOpen
            | Kind::New
            | Kind::Settings
            | Kind::Sidebar
            | Kind::Scan
            | Kind::ShowMain
            | Kind::Quit => Self::Application,
            Kind::Export | Kind::Reveal | Kind::CopyPath | Kind::Delete => Self::Files,
            _ => Self::Conversation,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Application => "settings-key-group-app",
            Self::Conversation => "settings-key-group-conversation",
            Self::Files => "settings-key-group-files",
            Self::Temporary => "temporary-title",
        }
    }
}

pub(super) struct KeysView {
    controller: Entity<ConfigController>,
    inputs: Vec<Entity<InputState>>,
    editing: Option<usize>,
    error: Option<String>,
    capture: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl KeysView {
    pub fn actions(owner: &Entity<Self>, global: &Entity<global_keys::GlobalKeys>) -> SettingGroup {
        let global = global.clone();
        let owner = owner.clone();
        SettingGroup::new().border_0().p_0().item(
            SettingItem::render(move |_, _, cx| Self::render_actions(&owner, &global, cx))
                .keywords(["快捷键 keys keyboard shortcut 恢复默认 reset defaults global template task 全局快捷任务 模板"]),
        )
    }

    fn render_actions(
        owner: &Entity<Self>,
        global: &Entity<global_keys::GlobalKeys>,
        cx: &App,
    ) -> AnyElement {
        let disabled = owner.read(cx).controller.read(cx).busy(cx)
            || owner
                .read(cx)
                .controller
                .read(cx)
                .preferences(cx)
                .keybindings
                .is_empty()
                && !owner
                    .read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .shortcuts
                    .has_bindings();
        let owner = owner.downgrade();
        let actions = h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(global_keys::GlobalKeys::add_button(global, cx))
            .child(
                Button::new("key-reset-all")
                    .small()
                    .label(t(cx, "settings-key-reset-all"))
                    .disabled(disabled)
                    .debug_selector(|| "key-reset-all".into())
                    .on_click(move |_, window, cx| {
                        let owner = owner.clone();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            let owner = owner.clone();
                            dialog
                                .title(t(cx, "settings-key-reset-all"))
                                .child(t(cx, "settings-key-reset-all-confirm"))
                                .footer(dialog_buttons("settings-key-reset-all", false, false, cx))
                                .on_ok(move |_, window, cx| {
                                    owner
                                        .update(cx, |this, cx| this.reset_all(window, cx))
                                        .unwrap_or(false)
                                })
                        });
                    }),
            );
        v_flex()
            .gap_2()
            .child(actions)
            .children(
                cx.try_global::<crate::app::shortcuts::ShortcutsRuntime>()
                    .and_then(|rt| rt.error.clone())
                    .map(|error| div().text_color(cx.theme().danger).child(error)),
            )
            .into_any_element()
    }

    fn reset_all(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.controller.read(cx).busy(cx) {
            return false;
        }
        self.cancel(window, cx);
        self.controller.update(cx, |controller, cx| {
            controller.set_preference(PreferenceChange::ResetKeybindings, cx);
        });
        self.sync(window, cx);
        true
    }

    pub fn item(owner: &Entity<Self>, index: usize, cx: &App) -> SettingItem {
        let command = &COMMANDS[index];
        let owner = owner.downgrade();
        SettingItem::new(
            t(cx, command.label),
            SettingField::render(move |options, _, cx| {
                owner
                    .update(cx, |this, cx| this.render_binding(index, options, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            }),
        )
        .keywords([command.id, "快捷键 keys keyboard shortcut"])
    }

    pub fn new(
        controller: Entity<ConfigController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = controller.read(cx).preferences(cx);
        let inputs: Vec<_> = COMMANDS
            .iter()
            .map(|command| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(command.value(&config.keybindings))
                        .placeholder(t(cx, "settings-key-unbound"))
                })
            })
            .collect();
        let mut subscriptions = Vec::new();
        for (index, input) in inputs.iter().enumerate() {
            let focus = input.focus_handle(cx);
            subscriptions.push(cx.on_focus_in(&focus, window, move |this, window, cx| {
                this.activate(index, window, cx);
            }));
            subscriptions.push(cx.on_focus_out(&focus, window, move |this, _, _, cx| {
                if this.editing == Some(index) {
                    this.capture = None;
                    cx.notify();
                }
            }));
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event, window, cx| match event {
                    InputEvent::Focus | InputEvent::Blur => {}
                    InputEvent::Change => {
                        let config = this.controller.read(cx).preferences(cx);
                        if input.read(cx).value().as_ref()
                            != COMMANDS[index].value(&config.keybindings)
                        {
                            this.activate(index, window, cx);
                        }
                        if this.editing == Some(index) {
                            this.error = None;
                        }
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        this.confirm(index, window, cx);
                    }
                },
            ));
        }
        let store = controller.read(cx).store.clone();
        subscriptions.push(store.observe_in(cx, window, |this, _, window, cx| {
            this.sync(window, cx);
        }));
        subscriptions.push(cx.observe_global_in::<crate::foundation::i18n::I18n>(
            window,
            |this, window, cx| {
                let placeholder = t(cx, "settings-key-unbound");
                for input in &this.inputs {
                    input.update(cx, |input, cx| {
                        input.set_placeholder(placeholder.clone(), window, cx)
                    });
                }
                cx.notify();
            },
        ));
        Self {
            controller,
            inputs,
            editing: None,
            error: None,
            capture: None,
            _subscriptions: subscriptions,
        }
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) {
            return;
        }
        let config = self.controller.read(cx).preferences(cx);
        for (index, input) in self.inputs.iter().enumerate() {
            if self.editing != Some(index) {
                let value = COMMANDS[index].value(&config.keybindings);
                if input.read(cx).value().as_ref() != value {
                    input.update(cx, |input, cx| input.set_value(value, window, cx));
                }
            }
        }
        cx.notify();
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing != Some(index) {
            self.cancel(window, cx);
            self.editing = Some(index);
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.capture = None;
        self.error = None;
        if let Some(index) = self.editing.take() {
            let config = self.controller.read(cx).preferences(cx);
            self.inputs[index].update(cx, |input, cx| {
                input.set_value(COMMANDS[index].value(&config.keybindings), window, cx);
            });
        }
        cx.notify();
    }

    fn clear(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) {
            return;
        }
        self.activate(index, window, cx);
        self.capture = None;
        self.error = None;
        self.inputs[index].update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn save(
        &mut self,
        index: usize,
        value: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.controller.read(cx).busy(cx) || self.capture.is_some() {
            return false;
        }
        self.activate(index, window, cx);
        let command = &COMMANDS[index];
        let config = self.controller.read(cx).preferences(cx);
        match keybindings::validate(
            command.id,
            value.as_deref().unwrap_or(command.default),
            &config.keybindings,
            cx,
        ) {
            Ok(()) => {
                self.editing = None;
                self.error = None;
                self.controller.update(cx, |owner, cx| {
                    owner.set_preference(PreferenceChange::Keybinding(command.id.into(), value), cx)
                });
                self.sync(window, cx);
                true
            }
            Err(error) => {
                self.error = Some(format!(
                    "{}: {}",
                    t(cx, "settings-key-conflict"),
                    t(cx, &error)
                ));
                cx.notify();
                false
            }
        }
    }

    fn confirm(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.capture.is_some() || self.controller.read(cx).busy(cx) {
            return false;
        }
        let value = self.inputs[index].read(cx).value().trim().to_owned();
        let config = self.controller.read(cx).preferences(cx);
        if value == COMMANDS[index].value(&config.keybindings) {
            self.cancel(window, cx);
            return true;
        }
        self.save(index, Some(value), window, cx)
    }

    fn start_recording(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) {
            return;
        }
        self.activate(index, window, cx);
        self.inputs[index].focus_handle(cx).focus(window, cx);
        self.error = None;
        let listener = cx.listener(move |this, event: &KeystrokeEvent, window, cx| {
            if !this.inputs[index].focus_handle(cx).is_focused(window)
                || this.controller.read(cx).busy(cx)
            {
                this.capture = None;
                cx.notify();
                return;
            }
            cx.stop_propagation();
            if event.keystroke.key != "escape" {
                this.inputs[index].update(cx, |input, cx| {
                    input.set_value(event.keystroke.unparse(), window, cx)
                });
            }
            this.capture = None;
            cx.notify();
        });
        self.capture = Some(cx.intercept_keystrokes(listener));
        cx.notify();
    }

    fn render_binding(
        &self,
        index: usize,
        options: &RenderOptions,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let command = &COMMANDS[index];
        let config = self.controller.read(cx).preferences(cx);
        let input = &self.inputs[index];
        let busy = self.controller.read(cx).busy(cx);
        let editing = self.editing == Some(index);
        let recording = editing && self.capture.is_some();
        let dirty = input.read(cx).value().as_ref() != command.value(&config.keybindings);
        let suffix = h_flex()
            .when(!input.read(cx).value().is_empty(), |row| {
                row.child(
                    Button::new(("key-clear", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Eraser)
                        .tooltip(t(cx, "settings-key-clear"))
                        .accessibility_label(t(cx, "settings-key-clear"))
                        .disabled(busy)
                        .debug_selector(move || format!("key-clear-{}", command.id))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.clear(index, window, cx)),
                        ),
                )
            })
            .child(
                Button::new(("key-record", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Keyboard)
                    .selected(recording)
                    .tooltip(t(
                        cx,
                        if recording {
                            "settings-key-recording"
                        } else {
                            "settings-key-record"
                        },
                    ))
                    .accessibility_label(t(cx, "settings-key-record"))
                    .disabled(busy)
                    .debug_selector(move || format!("key-record-{}", command.id))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_recording(index, window, cx)
                    })),
            );
        let row = h_flex()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .debug_selector(move || format!("key-binding-{}", command.id))
                    .child(
                        Input::new(input)
                            .with_size(options.size())
                            .disabled(busy)
                            .readonly(true)
                            .suffix(suffix),
                    ),
            )
            .when(config.keybindings.contains_key(command.id), |row| {
                row.child(
                    Button::new(("key-reset", index))
                        .ghost()
                        .with_size(options.size())
                        .icon(IconName::Undo2)
                        .tooltip(t(cx, "settings-key-reset"))
                        .accessibility_label(t(cx, "settings-key-reset"))
                        .disabled(busy || recording)
                        .debug_selector(move || format!("key-reset-{}", command.id))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.save(index, None, window, cx);
                        })),
                )
            })
            .when(editing && (dirty || recording), |row| {
                row.child(
                    Button::new(("key-confirm", index))
                        .ghost()
                        .with_size(options.size())
                        .icon(IconName::Check)
                        .tooltip(t(cx, "action-confirm"))
                        .accessibility_label(t(cx, "action-confirm"))
                        .disabled(busy || recording)
                        .debug_selector(move || format!("key-confirm-{}", command.id))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm(index, window, cx);
                        })),
                )
                .child(
                    Button::new(("key-cancel", index))
                        .ghost()
                        .with_size(options.size())
                        .icon(IconName::X)
                        .tooltip(t(cx, "settings-key-cancel"))
                        .accessibility_label(t(cx, "settings-key-cancel"))
                        .disabled(busy)
                        .debug_selector(move || format!("key-cancel-{}", command.id))
                        .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))),
                )
            });
        v_flex()
            .id(("key-field", index))
            .gap_1()
            .map(|field| {
                if options.layout() == Axis::Horizontal {
                    field.w(px(360.))
                } else {
                    field.w_full()
                }
            })
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                this.cancel(window, cx);
            }))
            .child(row)
            .when(recording, |field| {
                field.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "settings-key-recording")),
                )
            })
            .when(editing, |field| {
                field.children(self.error.as_ref().map(|error| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone())
                }))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, AppLanguage, COMMANDS, ConfigController, KeysView};
    use crate::features::settings::global_keys::GlobalKeys;
    use crate::state::config::{ConfigContents, ConfigData};
    use gpui_form::Form;
    use gpui_kit::component::{
        Root, WindowExt,
        group_box::GroupBoxVariant,
        setting::{SettingGroup, SettingPage, Settings},
    };
    use gpui_kit::{
        AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyBinding,
        Modifiers, ParentElement, Render, Styled, Task, TestAppContext, VisualTestContext, Window,
        div, point, px,
    };
    use gpui_operation::{Complete, Load, Transition};
    use std::{cell::Cell, rc::Rc};

    gpui_kit::actions!(keys_test, [UnrelatedAction]);

    fn command_index(id: &str) -> usize {
        COMMANDS
            .iter()
            .position(|command| command.id == id)
            .unwrap()
    }

    struct Fixture {
        global: Entity<GlobalKeys>,
        keys: Entity<KeysView>,
        _form: Entity<Form<AppConfig>>,
        dispatched: Rc<Cell<bool>>,
    }
    impl Render for Fixture {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let keys = self.keys.clone();
            div()
                .id("keys-fixture-root")
                .size_full()
                .on_action(cx.listener(|this, _: &UnrelatedAction, _, _| this.dispatched.set(true)))
                .child(
                    Settings::new("keys-fixture")
                        .with_group_variant(GroupBoxVariant::Normal)
                        .page(
                            SettingPage::new("快捷键")
                                .group(KeysView::actions(&keys, &self.global))
                                .group(
                                    SettingGroup::new()
                                        .title("应用")
                                        .item(KeysView::item(
                                            &self.keys,
                                            command_index("palette"),
                                            cx,
                                        ))
                                        .item(KeysView::item(
                                            &self.keys,
                                            command_index("quick_open"),
                                            cx,
                                        )),
                                ),
                        ),
                )
                .children(Root::render_dialog_layer(window, cx))
        }
    }
    fn setup(
        cx: &mut TestAppContext,
    ) -> (Entity<KeysView>, Rc<Cell<bool>>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            app_theme::init(cx);
            crate::state::theme::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
            cx.bind_keys([KeyBinding::new("ctrl-alt-9", UnrelatedAction, None)]);
        });
        let mut keys = None;
        let dispatched = Rc::new(Cell::new(false));
        let flag = dispatched.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            let form = cx.new(|_| Form::new(AppConfig::default()));
            let controller = cx.new(|cx| ConfigController::new(&form, cx));
            // Use the onboarding form as an in-memory preference store. No user files are written.
            controller.read(cx).store.clone().update(cx, |op| {
                op.transition(Load(Task::ready(())));
                op.transition(Complete(Ok(ConfigData {
                    path: "/tmp/keys-fixture/config.toml".into(),
                    contents: ConfigContents::Missing,
                    backup: None,
                })));
            });
            let global = cx.new(|_| GlobalKeys::new(controller.clone()));
            let owner = cx.new(|cx| KeysView::new(controller, window, cx));
            keys = Some(owner.clone());
            let fixture = cx.new(|_| Fixture {
                global,
                keys: owner,
                _form: form,
                dispatched: flag,
            });
            Root::new(fixture, window, cx)
        });
        cx.simulate_resize(gpui_kit::size(px(1000.), px(640.)));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (keys.unwrap(), dispatched, cx)
    }
    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn record(cx: &mut VisualTestContext, keys: &Entity<KeysView>, index: usize, value: &str) {
        cx.update(|window, cx| {
            keys.update(cx, |keys, cx| keys.start_recording(index, window, cx));
        });
        cx.simulate_keystrokes(value);
    }

    #[gpui_kit::test]
    fn reset_all_requires_confirmation_and_restores_every_input(cx: &mut TestAppContext) {
        let (keys, _, cx) = setup(cx);
        let button = cx.debug_bounds("key-reset-all").unwrap();
        assert!(
            button.left() > px(800.) && button.right() <= px(1000.),
            "{button:?}"
        );
        assert!(button.bottom() < px(120.));
        click(cx, "key-reset-all");
        cx.update(|window, cx| assert!(!window.has_active_dialog(cx)));
        cx.update(|window, cx| {
            keys.update(cx, |this, cx| {
                assert!(this.save(
                    command_index("palette"),
                    Some("ctrl-alt-7".into()),
                    window,
                    cx
                ));
                assert!(this.save(command_index("quick_open"), Some(String::new()), window, cx));
            });
        });
        record(cx, &keys, command_index("palette"), "ctrl-alt-8");
        click(cx, "key-reset-all");
        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            assert_eq!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .len(),
                2
            );
        });
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| {
            assert!(!window.has_active_dialog(cx));
            assert_eq!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .len(),
                2
            );
            assert_eq!(
                keys.read(cx).inputs[command_index("palette")]
                    .read(cx)
                    .value(),
                "ctrl-alt-8"
            );
        });
        click(cx, "key-reset-all");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(!window.has_active_dialog(cx));
            let keys = keys.read(cx);
            assert!(
                keys.controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
            for (input, command) in keys.inputs.iter().zip(super::COMMANDS) {
                assert_eq!(input.read(cx).value(), command.default);
            }
        });
        click(cx, "key-reset-all");
        cx.update(|window, cx| assert!(!window.has_active_dialog(cx)));
    }

    #[gpui_kit::test]
    fn clearing_requires_confirmation_and_cancel_restores_binding(cx: &mut TestAppContext) {
        let (keys, _, cx) = setup(cx);
        cx.update(|window, cx| {
            keys.read(cx).inputs[command_index("palette")]
                .focus_handle(cx)
                .focus(window, cx)
        });
        cx.simulate_keystrokes("enter");
        cx.update(|_, cx| {
            assert!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
        });
        click(cx, "key-clear-palette");
        cx.update(|window, cx| {
            assert!(!window.has_active_dialog(cx));
            let keys = keys.read(cx);
            assert!(
                keys.inputs[command_index("palette")]
                    .read(cx)
                    .value()
                    .is_empty()
            );
            assert!(
                keys.controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
        });
        click(cx, "key-cancel-palette");
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx).inputs[command_index("palette")]
                    .read(cx)
                    .value(),
                "secondary-shift-p"
            );
        });
        click(cx, "key-clear-palette");
        click(cx, "key-confirm-palette");
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .get("palette")
                    .map(String::as_str),
                Some("")
            );
        });
        click(cx, "key-reset-palette");
        cx.update(|_, cx| {
            assert!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
            assert_eq!(
                keys.read(cx).inputs[command_index("palette")]
                    .read(cx)
                    .value(),
                "secondary-shift-p"
            );
        });
        record(cx, &keys, command_index("palette"), "secondary-p");
        click(cx, "key-confirm-palette");
        cx.update(|_, cx| {
            assert!(
                keys.read(cx).error.is_some(),
                "conflicting shortcut stays in the item"
            );
            assert!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
        });
        click(cx, "key-cancel-palette");
        record(cx, &keys, command_index("palette"), "ctrl-alt-7");
        click(cx, "key-confirm-palette");
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .get("palette")
                    .map(String::as_str),
                Some("ctrl-alt-7")
            );
        });
    }

    #[gpui_kit::test]
    fn shortcut_field_rejects_typing_pasting_and_a_second_stroke(cx: &mut TestAppContext) {
        let (keys, _, cx) = setup(cx);
        let index = command_index("palette");
        cx.update(|window, cx| {
            keys.read(cx).inputs[index]
                .focus_handle(cx)
                .focus(window, cx)
        });
        cx.simulate_input("ctrl-alt-x y");
        cx.update(|_, cx| {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("ctrl-alt-x y".into()))
        });
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a cmd-v backspace"
        } else {
            "ctrl-a ctrl-v backspace"
        });
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx).inputs[index].read(cx).value(),
                "secondary-shift-p"
            )
        });
        record(cx, &keys, index, "ctrl-alt-7");
        cx.simulate_keystrokes("y");
        cx.update(|_, cx| assert_eq!(keys.read(cx).inputs[index].read(cx).value(), "ctrl-alt-7"));
        click(cx, "key-confirm-palette");
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx)
                    .controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings["palette"],
                "ctrl-alt-7"
            )
        });
    }

    #[gpui_kit::test]
    fn inline_recording_intercepts_keys_and_search_filters_each_item(cx: &mut TestAppContext) {
        let (keys, dispatched, cx) = setup(cx);
        click(cx, "key-record-palette");
        cx.simulate_keystrokes("ctrl-alt-9");
        assert!(
            !dispatched.get(),
            "recording must not execute application shortcuts"
        );
        cx.update(|window, cx| {
            assert!(!window.has_active_dialog(cx));
            let keys = keys.read(cx);
            assert!(keys.capture.is_none());
            assert_eq!(
                keys.inputs[command_index("palette")].read(cx).value(),
                "ctrl-alt-9"
            );
            assert!(
                keys.controller
                    .read(cx)
                    .preferences(cx)
                    .keybindings
                    .is_empty()
            );
        });
        click(cx, "key-record-palette");
        cx.simulate_keystrokes("escape");
        cx.update(|_, cx| {
            assert!(keys.read(cx).capture.is_none());
            assert_eq!(
                keys.read(cx).inputs[command_index("palette")]
                    .read(cx)
                    .value(),
                "ctrl-alt-9"
            );
        });
        cx.simulate_keystrokes("escape");
        cx.update(|_, cx| {
            assert_eq!(
                keys.read(cx).inputs[command_index("palette")]
                    .read(cx)
                    .value(),
                "secondary-shift-p"
            );
        });
        click(cx, "key-record-palette");
        // Moving focus to another field must release the recorder before the next keystroke.
        cx.update(|window, cx| {
            keys.read(cx).inputs[command_index("quick_open")]
                .focus_handle(cx)
                .focus(window, cx)
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        cx.update(|window, cx| {
            let keys = keys.read(cx);
            assert!(
                keys.capture.is_none(),
                "editing={:?}, first={}, second={}",
                keys.editing,
                keys.inputs[command_index("palette")]
                    .focus_handle(cx)
                    .is_focused(window),
                keys.inputs[command_index("quick_open")]
                    .focus_handle(cx)
                    .is_focused(window)
            );
        });
        cx.simulate_click(point(px(100.), px(24.)), Modifiers::default());
        cx.simulate_input("palette");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("key-binding-palette").is_some());
        assert!(cx.debug_bounds("key-binding-quick_open").is_none());
    }
}
