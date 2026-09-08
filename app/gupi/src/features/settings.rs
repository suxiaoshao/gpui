mod onboarding;
mod preferences;
mod sticky;

use crate::foundation::assets::IconName;
use crate::pi::PiProbeController;
use crate::{
    foundation::i18n::t,
    state::config::{AppConfig, AppLanguage, ConfigController, ConfigRepair, ThemeMode},
};
use gpui_form::{ControlBinding, ControlProjection, Form};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
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
    _binding: ControlBinding,
    _subscriptions: Vec<Subscription>,
    error: Option<String>,
    confirmation: Option<ConfigRepair>,
    draft_pi: Entity<PiProbeController>,
    step: usize,
    transition: Option<onboarding::PageTransition>,
    transition_serial: u64,
    page_scroll: [ScrollHandle; 4],
    theme_scroll: ScrollHandle,
    language: Entity<ComboboxState<SearchableVec<LanguageItem>>>,
    _language_binding: ControlBinding,
}
impl SettingsView {
    pub fn new(
        form: Entity<Form<AppConfig>>,
        controller: Entity<ConfigController>,
        draft_pi: Entity<PiProbeController>,
        focus_handle: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("pi"));
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
        let (language_binding, language_writer) = AppConfig::LANGUAGE.bind_control_in(
            &form,
            &language,
            |control, projection, window, cx| {
                if let ControlProjection::Value(value) = projection {
                    control.set_selected_values(&[value], window, cx);
                }
            },
            window,
            cx,
        );
        let guard = controller.clone();
        let language_sub = cx.subscribe_in(&language, window, move |_, _, event, window, cx| {
            if !guard.read(cx).busy(cx)
                && let ComboboxEvent::Confirm(values) = event
                && let Some(value) = values.first()
            {
                language_writer.defer_set(*value, window, cx);
            }
        });
        let locale_sub =
            cx.observe_global_in::<crate::foundation::i18n::I18n>(window, |this, window, cx| {
                let selected = AppConfig::LANGUAGE.get(&this.form, cx);
                let items = language_items(cx);
                this.language.update(cx, |control, cx| {
                    control.set_items(items, window, cx);
                    control.set_selected_values(&[selected], window, cx);
                });
                cx.notify();
            });
        let pi_sub = cx.observe(&draft_pi, |_, _, cx| cx.notify());
        let form_sub = cx.observe(&form, |_, _, cx| cx.notify());
        let store = controller.read(cx).store.clone();
        let store_sub = store.observe(cx, |_, _, cx| cx.notify());
        Self {
            focus_handle,
            form,
            controller,
            input,
            _binding: binding,
            _subscriptions: vec![
                input_sub,
                form_sub,
                store_sub,
                language_sub,
                locale_sub,
                pi_sub,
            ],
            draft_pi,
            step: 0,
            transition: None,
            transition_serial: 0,
            page_scroll: std::array::from_fn(|_| ScrollHandle::new()),
            theme_scroll: ScrollHandle::new(),
            language,
            _language_binding: language_binding,
            error: None,
            confirmation: None,
        }
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self
            .controller
            .update(cx, |owner, cx| owner.submit_draft(cx));
        self.error = result.err();
        if self.error.is_some() {
            self.input.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    fn request(&mut self, action: ConfigRepair, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy(cx) {
            return;
        }
        if matches!(
            action,
            ConfigRepair::BackupAndWrite | ConfigRepair::BackupAndReset
        ) || (matches!(action, ConfigRepair::Reload) && self.form.read(cx).is_dirty())
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
        let busy = self.controller.read(cx).busy(cx);
        let store = self.controller.read(cx).store.clone();
        let (problem, can_write, conflict, reconcile, write_failed, backup) =
            store.read(cx, |op| {
                (
                    op.problem().map(|p| p.key),
                    op.data().and_then(|d| d.configured()).is_some(),
                    op.problem().is_some_and(|p| p.conflict),
                    op.problem().is_some_and(|p| p.reconcile),
                    op.problem().is_some_and(|p| p.write_source.is_some()),
                    op.problem()
                        .and_then(|p| p.backup.clone())
                        .or_else(|| op.data().and_then(|d| d.backup.clone())),
                )
            });
        let onboarding = store.read(cx, |op| {
            op.data().is_some_and(|data| data.configured().is_none())
        });
        if onboarding {
            return self.render_onboarding(window, cx).into_any_element();
        }
        let mut view = v_flex()
            .gap_6()
            .w_full()
            .child(div().text_xl().child(t(cx, "menu-settings")))
            .child(self.render_language(cx))
            .child(div().h(px(420.)).child(self.render_appearance(window, cx)))
            .child(self.render_pi(cx));
        view = view.child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("save")
                        .icon(IconName::Save)
                        .primary()
                        .label(t(
                            cx,
                            if busy {
                                "startup-checking"
                            } else {
                                "settings-save"
                            },
                        ))
                        .disabled(busy || conflict || reconcile)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.controller.read(cx).busy(cx) {
                                return;
                            }
                            this.submit(window, cx);
                        })),
                )
                .child(
                    Button::new("reload")
                        .icon(IconName::RotateCw)
                        .label(t(cx, "settings-reload"))
                        .disabled(busy)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.request(ConfigRepair::Reload, cx)),
                        ),
                ),
        );
        view = view.child(
            Button::new("locate-config")
                .icon(IconName::FolderOpen)
                .label(t(cx, "action-locate"))
                .on_click(|_, _, cx| {
                    if let Ok(path) = crate::foundation::paths::config_dir() {
                        cx.reveal_path(&path);
                    }
                }),
        );
        if can_write {
            view = view.child(
                Button::new("write-current")
                    .label(t(cx, "settings-write-current"))
                    .disabled(busy || conflict || reconcile)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .update(cx, |owner, cx| owner.write_committed(cx))
                    })),
            );
        }
        if let Some(key) = self.error.as_deref().or(problem) {
            view = view.child(div().text_color(cx.theme().danger).child(t(cx, key)));
            if !reconcile {
                let mut repairs = h_flex().gap_2();
                if conflict {
                    repairs = repairs.child(
                        Button::new("overwrite")
                            .label(t(cx, "action-overwrite"))
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(ConfigRepair::BackupAndWrite, cx)
                            })),
                    );
                } else if !write_failed {
                    repairs = repairs.child(
                        Button::new("reset")
                            .label(t(cx, "action-reset"))
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(ConfigRepair::BackupAndReset, cx)
                            })),
                    );
                }
                view = view.child(repairs);
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
        view.into_any_element()
    }
}
