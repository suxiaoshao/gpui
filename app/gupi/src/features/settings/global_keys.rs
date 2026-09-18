use super::*;
use crate::state::shortcuts::{ShortcutTask, Shortcuts};
use gpui_kit::component::{
    Sizable, WindowExt,
    setting::{SettingField, SettingGroup, SettingItem},
};
use gpui_kit::prelude::FluentBuilder;
mod editor;

pub(super) struct GlobalKeys {
    controller: Entity<ConfigController>,
    inputs: std::collections::BTreeMap<String, Entity<BindingInput>>,
}
impl GlobalKeys {
    pub fn new(controller: Entity<ConfigController>) -> Self {
        Self {
            controller,
            inputs: Default::default(),
        }
    }
    pub fn groups(owner: &Entity<Self>, cx: &App) -> Vec<SettingGroup> {
        let mut group = SettingGroup::new().title(t(cx, "shortcut-global"));
        let config = owner.read(cx).controller.read(cx).preferences(cx).shortcuts;
        let mut groups = Vec::new();
        for (id, label) in std::iter::once((String::new(), t(cx, "shortcut-launcher"))).chain(
            config
                .tasks
                .iter()
                .map(|task| (task.id.clone(), task.name.clone())),
        ) {
            if !id.is_empty() && groups.is_empty() {
                groups.push(group);
                group = SettingGroup::new().title(t(cx, "shortcut-tasks"));
            }
            let owner = owner.clone();
            let keywords = label.clone();
            let description = config.tasks.iter().find(|t| t.id == id).map(|t| {
                format!(
                    "/{}",
                    t.template.file_stem().unwrap_or_default().to_string_lossy()
                )
            });
            group = group.item(
                SettingItem::new(
                    label,
                    SettingField::render(move |_, window, cx| {
                        owner.update(cx, |this, cx| this.render_binding(&id, window, cx))
                    }),
                )
                .description(description.unwrap_or_default())
                .keywords(vec![keywords, "global shortcut 全局快捷键".into()]),
            );
        }
        if groups.is_empty() {
            groups.push(group);
            group = SettingGroup::new().title(t(cx, "shortcut-tasks"));
        }
        groups.push(group);
        groups
    }
    pub(super) fn add_button(owner: &Entity<Self>, cx: &App) -> Button {
        let disabled = owner.read(cx).controller.read(cx).busy(cx);
        let owner = owner.clone();
        Button::new("add-shortcut-task")
            .small()
            .icon(IconName::Plus)
            .label(t(cx, "shortcut-add"))
            .disabled(disabled)
            .on_click(move |_, window, cx| owner.update(cx, |this, cx| this.edit(None, window, cx)))
    }
    fn render_binding(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self
            .inputs
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| BindingInput::new(id.to_owned(), self.controller.clone(), window, cx))
            })
            .clone();
        let row = h_flex().gap_1().child(input);
        if id.is_empty() {
            return row.into_any_element();
        }
        let edit = id.to_owned();
        let remove = id.to_owned();
        let toggle = id.to_owned();
        let enabled = self
            .controller
            .read(cx)
            .preferences(cx)
            .shortcuts
            .tasks
            .iter()
            .find(|task| task.id == id)
            .is_some_and(|task| task.enabled);
        let busy = self.controller.read(cx).busy(cx);
        row.child(
            gpui_kit::component::switch::Switch::new(format!("enable-task-{id}"))
                .small()
                .checked(enabled)
                .disabled(busy)
                .on_click(cx.listener(move |this, checked, _, cx| {
                    let mut config = this.controller.read(cx).preferences(cx).shortcuts;
                    if let Some(task) = config.tasks.iter_mut().find(|task| task.id == toggle) {
                        task.enabled = *checked;
                    }
                    this.controller.update(cx, |c, cx| {
                        c.set_preference(PreferenceChange::Shortcuts(config), cx)
                    });
                })),
        )
        .child(
            Button::new(format!("edit-task-{id}"))
                .ghost()
                .small()
                .icon(IconName::SquarePen)
                .tooltip(t(cx, "shortcut-edit"))
                .accessibility_label(t(cx, "shortcut-edit"))
                .disabled(busy)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(Some(edit.clone()), window, cx)
                })),
        )
        .child(
            Button::new(format!("delete-task-{id}"))
                .ghost()
                .small()
                .icon(IconName::Trash2)
                .tooltip(t(cx, "shortcut-delete"))
                .accessibility_label(t(cx, "shortcut-delete"))
                .disabled(busy)
                .on_click(cx.listener(move |this, _, window, cx| {
                    let target = remove.clone();
                    let controller = this.controller.clone();
                    window.open_dialog(cx, move |dialog, _, cx| {
                        let target = target.clone();
                        let controller = controller.clone();
                        dialog
                            .title(t(cx, "shortcut-delete"))
                            .footer(dialog_buttons("shortcut-delete", false, false, cx))
                            .on_ok(move |_, _, cx| {
                                let mut config = controller.read(cx).preferences(cx).shortcuts;
                                config.tasks.retain(|task| task.id != target);
                                controller.update(cx, |c, cx| {
                                    c.set_preference(PreferenceChange::Shortcuts(config), cx)
                                });
                                true
                            })
                    });
                })),
        )
        .into_any_element()
    }
    fn edit(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let definition = id
            .and_then(|id| {
                self.controller
                    .read(cx)
                    .preferences(cx)
                    .shortcuts
                    .tasks
                    .into_iter()
                    .find(|t| t.id == id)
            })
            .unwrap_or_else(|| ShortcutTask {
                id: uuid::Uuid::new_v4().to_string(),
                enabled: true,
                ..Default::default()
            });
        editor::open(definition, self.controller.clone(), window, cx);
    }
}
struct BindingInput {
    id: String,
    controller: Entity<ConfigController>,
    input: Entity<InputState>,
    saved: String,
    capture: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
fn value(config: &Shortcuts, id: &str) -> String {
    if id.is_empty() {
        config.launcher.clone()
    } else {
        config
            .tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.binding.clone())
            .unwrap_or_default()
    }
}
impl BindingInput {
    fn new(
        id: String,
        controller: Entity<ConfigController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved = value(&controller.read(cx).preferences(cx).shortcuts, &id);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(saved.clone())
                .placeholder(t(cx, "settings-key-unbound"))
        });
        let sub = cx.subscribe_in(&input, window, |this, _, e, window, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                this.save(cx);
            }
            if matches!(e, InputEvent::Blur) {
                this.capture = None;
            }
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
            let _ = window;
        });
        let store = controller.read(cx).store.clone();
        let config = store.observe_in(cx, window, |this, op, window, cx| {
            if !op.is_running()
                && let Some(config) = op.data().and_then(|d| d.configured())
            {
                let next = value(&config.shortcuts, &this.id);
                if next != this.saved {
                    this.saved = next.clone();
                    this.input.update(cx, |s, cx| s.set_value(next, window, cx));
                }
            }
            cx.notify();
        });
        Self {
            id,
            controller,
            input,
            saved,
            capture: None,
            _subscriptions: vec![sub, config],
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) || self.capture.is_some() {
            return;
        }
        let mut config = self.controller.read(cx).preferences(cx).shortcuts;
        let binding = self.input.read(cx).value().trim().to_owned();
        if self.id.is_empty() {
            config.launcher = binding;
        } else if let Some(task) = config.tasks.iter_mut().find(|t| t.id == self.id) {
            task.binding = binding;
        }
        self.controller.update(cx, |c, cx| {
            c.set_preference(PreferenceChange::Shortcuts(config), cx)
        });
    }
}
impl Render for BindingInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.controller.read(cx).busy(cx);
        let dirty = self.input.read(cx).value().as_ref() != self.saved;
        let suffix = h_flex()
            .child(
                Button::new("clear")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Eraser)
                    .tooltip(t(cx, "settings-key-clear"))
                    .accessibility_label(t(cx, "settings-key-clear"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.capture = None;
                        this.input.update(cx, |s, cx| s.set_value("", window, cx));
                    })),
            )
            .child(
                Button::new("record")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Keyboard)
                    .tooltip(t(cx, "settings-key-record"))
                    .accessibility_label(t(cx, "settings-key-record"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.input.focus_handle(cx).focus(window, cx);
                        let listener = cx.listener(|this, event: &KeystrokeEvent, window, cx| {
                            cx.stop_propagation();
                            if event.keystroke.key != "escape" {
                                this.input.update(cx, |s, cx| {
                                    s.set_value(event.keystroke.unparse(), window, cx)
                                });
                            }
                            this.capture = None;
                            cx.notify();
                        });
                        this.capture = Some(cx.intercept_keystrokes(listener));
                        cx.notify();
                    })),
            );
        h_flex()
            .w(px(360.))
            .gap_1()
            .child(
                div().flex_1().child(
                    Input::new(&self.input)
                        .readonly(self.capture.is_some())
                        .disabled(busy)
                        .suffix(suffix),
                ),
            )
            .when(dirty || self.capture.is_some(), |row| {
                row.child(
                    Button::new("confirm")
                        .ghost()
                        .small()
                        .icon(IconName::Check)
                        .tooltip(t(cx, "action-confirm"))
                        .accessibility_label(t(cx, "action-confirm"))
                        .disabled(busy || self.capture.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                )
                .child(
                    Button::new("cancel")
                        .ghost()
                        .small()
                        .icon(IconName::X)
                        .tooltip(t(cx, "action-cancel"))
                        .accessibility_label(t(cx, "action-cancel"))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.capture = None;
                            this.input
                                .update(cx, |s, cx| s.set_value(this.saved.clone(), window, cx));
                            cx.notify();
                        })),
                )
            })
    }
}
