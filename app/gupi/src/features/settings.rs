mod global_keys;
mod keys;
mod layout;
mod onboarding;
mod preferences;
mod resources;
mod sticky;
#[cfg(test)]
mod tests;
mod theme_grid;

use crate::foundation::assets::IconName;
use crate::pi::PiProbeController;
use crate::{
    foundation::i18n::t,
    state::config::{
        AppConfig, AppLanguage, ConfigController, ConfigRepair, PiSettings, PreferenceChange,
        ThemeMode,
    },
};
use gpui_form::{ControlBinding, ControlProjection, Form};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputGroup, InputGroupAddon, InputGroupAddonAlignment, InputState},
    v_flex,
};
use gpui_kit::component::{
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::SearchableVec,
};
use gpui_kit::*;
use preferences::{LanguageItem, language_items};

pub(crate) struct SettingsView {
    focus_handle: FocusHandle,
    pub form: Entity<Form<AppConfig>>,
    controller: Entity<ConfigController>,
    input: Entity<InputState>,
    pi_input: Entity<InputState>,
    config_path: Entity<InputState>,
    _binding: ControlBinding,
    _pi_binding: ControlBinding,
    _subscriptions: Vec<Subscription>,
    error: Option<String>,
    confirmation: Option<ConfigRepair>,
    draft_pi: Entity<PiProbeController>,
    applied_pi: Entity<PiProbeController>,
    resources: Entity<resources::ResourcesView>,
    keys: Entity<keys::KeysView>,
    global_keys: Entity<global_keys::GlobalKeys>,
    step: usize,
    transition: Option<onboarding::PageTransition>,
    transition_serial: u64,
    page_scroll: [ScrollHandle; 4],
    theme_scroll: ScrollHandle,
    language: Entity<ComboboxState<SearchableVec<LanguageItem>>>,
}
impl SettingsView {
    pub fn new(
        form: Entity<Form<AppConfig>>,
        controller: Entity<ConfigController>,
        draft_pi: Entity<PiProbeController>,
        applied_pi: Entity<PiProbeController>,
        focus_handle: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let resources = cx.new(|cx| {
            resources::ResourcesView::new(controller.clone(), applied_pi.clone(), window, cx)
        });
        let global_keys = cx.new(|_| global_keys::GlobalKeys::new(controller.clone()));
        let keys = cx.new(|cx| keys::KeysView::new(controller.clone(), window, cx));
        let config_path = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                controller
                    .read(cx)
                    .path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        });
        let resources_sub = cx.observe(&resources, |_, _, cx| cx.notify());
        let keys_sub = cx.observe(&keys, |_, _, cx| cx.notify());
        let applied_sub = cx.observe(&applied_pi, |_, _, cx| cx.notify());
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("pi")
                .default_value(AppConfig::PI_COMMAND.get(&form, cx).unwrap_or_default())
        });
        let (binding, writer) = AppConfig::PI_COMMAND.bind_control_in(
            &form,
            &input,
            |input, projection, window, cx| {
                if let ControlProjection::Value(value) = projection {
                    input.set_value(value.unwrap_or_default(), window, cx);
                }
            },
            window,
            cx,
        );
        let guard = controller.clone();
        let input_sub = cx.subscribe_in(&input, window, move |_, input, event, window, cx| {
            if guard.read(cx).busy(cx) {
                return;
            }
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                writer.defer_set(
                    if value.trim().is_empty() {
                        None
                    } else {
                        Some(value)
                    },
                    window,
                    cx,
                );
            }
        });
        let language = cx
            .new(|cx| ComboboxState::new(language_items(cx), vec![], window, cx).searchable(true));
        let pi_form = controller.read(cx).pi_form.clone();
        let pi_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("pi")
                .default_value(PiSettings::COMMAND.get(&pi_form, cx).unwrap_or_default())
        });
        let (pi_binding, pi_writer) = PiSettings::COMMAND.bind_control_in(
            &pi_form,
            &pi_input,
            |input, projection, window, cx| {
                if let ControlProjection::Value(value) = projection {
                    input.set_value(value.unwrap_or_default(), window, cx);
                }
            },
            window,
            cx,
        );
        let guard = controller.clone();
        let pi_input_sub =
            cx.subscribe_in(&pi_input, window, move |_, input, event, window, cx| {
                if !guard.read(cx).busy(cx) && matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    pi_writer.defer_set(
                        if value.trim().is_empty() {
                            None
                        } else {
                            Some(value)
                        },
                        window,
                        cx,
                    );
                }
            });
        let guard = controller.clone();
        let language_sub = cx.subscribe_in(&language, window, move |_, _, event, _, cx| {
            if !guard.read(cx).busy(cx)
                && let ComboboxEvent::Confirm(values) = event
                && let Some(value) = values.first()
            {
                guard.update(cx, |owner, cx| {
                    owner.set_preference(PreferenceChange::Language(*value), cx)
                });
            }
        });
        let locale_sub =
            cx.observe_global_in::<crate::foundation::i18n::I18n>(window, |this, window, cx| {
                let selected = this.controller.read(cx).preferences(cx).language;
                let items = language_items(cx);
                this.language.update(cx, |control, cx| {
                    control.set_items(items, window, cx);
                    control.set_selected_values(&[selected], window, cx);
                });
                cx.notify();
            });
        let temporary_sub =
            cx.observe_global::<crate::app::temporary::Temporary>(|_, cx| cx.notify());
        let pi_sub = cx.observe(&draft_pi, |_, _, cx| cx.notify());
        let form_sub = cx.observe_in(&form, window, |this, _, window, cx| {
            this.refresh_language(window, cx)
        });
        let pi_form_sub = cx.observe(&pi_form, |_, _, cx| cx.notify());
        let store = controller.read(cx).store.clone();
        let store_sub = store.observe_in(cx, window, |this, _, window, cx| {
            this.refresh_language(window, cx)
        });
        Self {
            focus_handle,
            form,
            controller,
            input,
            pi_input,
            config_path,
            _binding: binding,
            _pi_binding: pi_binding,
            _subscriptions: vec![
                temporary_sub,
                resources_sub,
                keys_sub,
                applied_sub,
                input_sub,
                form_sub,
                store_sub,
                language_sub,
                locale_sub,
                pi_sub,
                pi_input_sub,
                pi_form_sub,
            ],
            draft_pi,
            applied_pi,
            resources,
            keys,
            global_keys,
            step: 0,
            transition: None,
            transition_serial: 0,
            page_scroll: std::array::from_fn(|_| ScrollHandle::new()),
            theme_scroll: ScrollHandle::new(),
            language,
            error: None,
            confirmation: None,
        }
    }
    fn refresh_language(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.controller.read(cx).preferences(cx).language;
        self.language.update(cx, |control, cx| {
            control.set_selected_values(&[selected], window, cx)
        });
        cx.notify();
    }
    fn pi_command(&self, cx: &App) -> Option<String> {
        let controller = self.controller.read(cx);
        if controller.is_onboarding(cx) {
            AppConfig::PI_COMMAND.get(&self.form, cx)
        } else {
            PiSettings::COMMAND.get(&controller.pi_form, cx)
        }
    }
    fn pi_input(&self, cx: &App) -> &Entity<InputState> {
        if self.controller.read(cx).is_onboarding(cx) {
            &self.input
        } else {
            &self.pi_input
        }
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self.controller.update(cx, |owner, cx| {
            if owner.is_onboarding(cx) {
                owner.submit_draft(cx)
            } else {
                owner.submit_pi(cx)
            }
        });
        self.error = result.err();
        if self.error.is_some() {
            self.pi_input(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    fn request(&mut self, action: ConfigRepair, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) {
            return;
        }
        if matches!(action, ConfigRepair::BackupAndReset)
            || (matches!(action, ConfigRepair::Reload) && {
                let controller = self.controller.read(cx);
                if controller.is_onboarding(cx) {
                    self.form.read(cx).is_dirty()
                } else {
                    controller.pi_form.read(cx).is_dirty()
                }
            })
        {
            self.confirmation = Some(action);
            cx.notify();
            return;
        }
        self.apply_repair(action, cx);
    }
    fn apply_repair(&mut self, action: ConfigRepair, cx: &mut Context<Self>) {
        self.error = self
            .controller
            .update(cx, |owner, cx| owner.repair(action, cx))
            .err();
        cx.notify();
    }
}
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.controller.read(cx).store.clone();
        let onboarding = store.read(cx, |op| {
            op.data().is_some_and(|data| data.configured().is_none())
        });
        if onboarding {
            return self.render_onboarding(window, cx).into_any_element();
        }
        self.render_settings(cx)
    }
}
impl SettingsView {
    pub fn stop_resources(&self, cx: &mut Context<Self>) {
        self.resources
            .read(cx)
            .controller
            .clone()
            .update(cx, |owner, _| owner.stop());
    }
    fn render_config_actions(
        &self,
        options: &gpui_kit::component::setting::RenderOptions,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let store = self.controller.read(cx).store.clone();
        let (busy, problem, write_failed, can_write, backup) = store.read(cx, |op| {
            (
                op.is_running(),
                op.problem().map(|p| p.key()),
                op.problem().is_some_and(|p| p.is_write()),
                op.problem().is_some() && op.data().is_some(),
                op.problem()
                    .and_then(|p| p.backup().cloned())
                    .or_else(|| op.data().and_then(|d| d.backup.clone())),
            )
        });
        let path = self
            .controller
            .read(cx)
            .path()
            .map(std::path::Path::to_path_buf);
        let mut view = v_flex().gap_2().child(
            div()
                .debug_selector(|| "settings-config-path".into())
                .child(
                    InputGroup::new("config-path")
                        .readonly(true)
                        .input(Input::new(&self.config_path))
                        .addon(
                            InputGroupAddon::new("actions")
                                .align(InputGroupAddonAlignment::InlineEnd)
                                .child(
                                    Button::new("reload-config")
                                        .debug_selector(|| "settings-config-reload".into())
                                        .ghost()
                                        .icon(IconName::RotateCw)
                                        .tooltip(t(cx, "settings-reload"))
                                        .accessibility_label(t(cx, "settings-reload"))
                                        .disabled(busy)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.request(ConfigRepair::Reload, cx)
                                        })),
                                )
                                .child(
                                    Button::new("open-config")
                                        .debug_selector(|| "settings-config-open".into())
                                        .ghost()
                                        .icon(IconName::FileText)
                                        .tooltip(t(cx, "settings-config-open"))
                                        .accessibility_label(t(cx, "settings-config-open"))
                                        .disabled(path.is_none())
                                        .on_click(move |_, _, cx| {
                                            if let Some(path) = &path {
                                                cx.open_with_system(path);
                                            }
                                        }),
                                ),
                        ),
                ),
        );
        if can_write {
            view = view.child(
                Button::new("write-current")
                    .label(t(cx, "settings-write-current"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .update(cx, |owner, cx| owner.write_committed(cx))
                    })),
            );
        }
        if let Some(key) = self.error.as_deref().or(problem) {
            view = view.child(div().text_color(cx.theme().danger).child(t(cx, key)));
            if !write_failed {
                view = view.child(
                    Button::new("reset")
                        .label(t(cx, "action-reset"))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(ConfigRepair::BackupAndReset, cx)
                        })),
                );
            }
        }
        if let Some(backup) = backup {
            view = view.child(format!(
                "{}: {}",
                t(cx, "recovery-backup"),
                backup.display()
            ));
        }
        if self.confirmation.is_some() {
            view = view.child(
                v_flex().gap_2().child(t(cx, "recovery-confirm")).child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("confirm")
                                .label(t(cx, "action-confirm"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if !this.controller.read(cx).busy(cx)
                                        && let Some(action) = this.confirmation.take()
                                    {
                                        this.apply_repair(action, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("cancel")
                                .label(t(cx, "action-cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirmation = None;
                                    cx.notify();
                                })),
                        ),
                ),
            );
        }
        if options.layout() == Axis::Horizontal {
            view.w(px(360.)).into_any_element()
        } else {
            view.w_full().into_any_element()
        }
    }
}

fn dialog_buttons(label: &str, busy: bool, confirm_disabled: bool, cx: &App) -> impl IntoElement {
    use gpui_kit::component::dialog::{Cancel, Confirm, DialogFooter};
    DialogFooter::new()
        .child(
            Button::new("cancel")
                .label(t(cx, "action-cancel"))
                .disabled(busy)
                .on_click(|_, window, cx| window.dispatch_action(Box::new(Cancel), cx)),
        )
        .child(
            Button::new("confirm")
                .primary()
                .label(t(cx, label))
                .disabled(busy || confirm_disabled)
                .on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(Confirm { secondary: false }), cx)
                }),
        )
}
