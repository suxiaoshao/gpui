use super::{home, settings::SettingsView};
use crate::{
    app::menus,
    components::recovery::recovery,
    foundation::{
        i18n::{self, t},
        paths,
    },
    pi::PiProbeController,
    state::{
        config::{AppConfig, ConfigController, ConfigRepair},
        layout, theme,
    },
};
use gpui_form::Form;
use gpui_kit::component::{
    ActiveTheme, Disableable, TitleBar, button::Button, h_flex, spinner::Spinner, v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(crate) struct StartupView {
    focus_handle: FocusHandle,
    pub config: Entity<ConfigController>,
    applied_pi: Entity<PiProbeController>,
    draft_pi: Entity<PiProbeController>,
    settings: Entity<SettingsView>,
    pub show_settings: bool,
    pub layout_problem: Option<String>,
    layout_task: Option<Task<()>>,
    layout_confirm: bool,
    config_confirm: bool,
    log_warning: bool,
    applied: Option<AppConfig>,
    _subscriptions: Vec<Subscription>,
    pub draining: bool,
    quit_task: Option<Task<()>>,
}
impl StartupView {
    pub fn new(
        layout_problem: Option<String>,
        log_warning: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let config = cx.new(|cx| ConfigController::new(&form, cx));
        let applied_pi = cx.new(|_| PiProbeController::new());
        let draft_pi = cx.new(|_| PiProbeController::new());
        let settings = cx.new(|cx| {
            SettingsView::new(
                form.clone(),
                config.clone(),
                draft_pi.clone(),
                focus_handle.clone(),
                window,
                cx,
            )
        });
        let store = config.read(cx).store.clone();
        let config_sub = store.observe_in(cx, window, |this, op, _window, cx| {
            let value = op.data().and_then(|d| d.configured()).cloned();
            if this.applied != value {
                if let Some(value) = &value
                    && this.layout_problem.is_none()
                {
                    this.applied_pi
                        .update(cx, |pi, cx| pi.request(value.pi_command.clone(), false, cx));
                }
                this.applied = value;
            }
            cx.notify();
        });
        let pi_sub = cx.observe(&applied_pi, |_, _, cx| cx.notify());
        let preview_form = form.clone();
        let form_sub = cx.observe_in(&form, window, move |_, form, window, cx| {
            let draft = crate::state::config::AppConfig::ROOT.get(&form, cx);
            i18n::apply(draft.language, cx);
            theme::apply(&draft, window, cx);
            menus::refresh(cx);
        });
        let appearance = window.observe_window_appearance(move |window, cx| {
            theme::apply(
                &crate::state::config::AppConfig::ROOT.get(&preview_form, cx),
                window,
                cx,
            );
        });
        let accent_form = form.clone();
        let accent = cx.observe_global_in::<app_theme::SystemAccentThemeState>(
            window,
            move |_, window, cx| {
                theme::apply(
                    &crate::state::config::AppConfig::ROOT.get(&accent_form, cx),
                    window,
                    cx,
                );
            },
        );
        config.update(cx, |owner, cx| owner.reload(cx));
        Self {
            focus_handle,
            config,
            applied_pi,
            draft_pi,
            settings,
            show_settings: false,
            layout_problem,
            layout_task: None,
            layout_confirm: false,
            config_confirm: false,
            log_warning,
            applied: None,
            _subscriptions: vec![config_sub, pi_sub, form_sub, appearance, accent],
            draining: false,
            quit_task: None,
        }
    }
    pub fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        tracing::info!("managed quit started");
        self.draining = true;
        self.config.update(cx, |owner, _| owner.draining = true);
        self.applied_pi.update(cx, |pi, _| pi.stop());
        self.draft_pi.update(cx, |pi, _| pi.stop());
        let placement = layout::capture(window);
        let save_layout = self.layout_problem.is_none();
        self.quit_task = Some(cx.spawn(async move |owner, cx| {
            loop {
                let busy = owner
                    .read_with(cx, |owner, cx| {
                        owner.config.read(cx).store.read(cx, |op| op.is_running())
                            || owner.applied_pi.read(cx).operation.is_running()
                            || owner.draft_pi.read(cx).operation.is_running()
                            || owner.layout_task.is_some()
                    })
                    .unwrap_or(false);
                if !busy {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(20))
                    .await;
            }
            if save_layout {
                let result = smol::unblock(move || {
                    paths::config_dir()
                        .map_err(|e| e.to_string())
                        .and_then(|dir| layout::save(&dir.join("state.toml"), &placement))
                })
                .await;
                if let Err(error) = result {
                    tracing::error!(%error, "layout save failed");
                }
            }
            tracing::info!("managed quit completed");
            cx.update(|cx| cx.quit());
        }));
        cx.notify();
    }
}
impl Render for StartupView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.config.read(cx).store.clone();
        let (configured, missing, config_busy, problem) = store.read(cx, |op| {
            (
                op.data().and_then(|d| d.configured()).is_some(),
                op.data().is_some_and(|d| d.configured().is_none()),
                op.is_running(),
                op.problem().map(|p| p.key),
            )
        });
        let onboarding = !configured && missing && !self.draining;
        let mut content = v_flex()
            .gap_5()
            .w_full()
            .when(!onboarding, |view| view.max_w(px(800.)))
            .when(!configured, |view| view.h_full().min_h_0());
        if self.log_warning {
            content = content.child(t(cx, "error-log"));
        }
        if self.draining {
            content = content.child(t(cx, "startup-quitting"));
        } else if !configured {
            if config_busy && !missing {
                content = content.child(t(cx, "startup-checking"));
            } else if !missing && problem.is_some() {
                content = content
                    .child(recovery(
                        t(cx, "recovery-config-title"),
                        t(cx, problem.unwrap_or("error-config-read")),
                        cx,
                    ))
                    .child(
                        Button::new("config-reload")
                            .label(t(cx, "settings-reload"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.config.update(cx, |owner, cx| owner.reload(cx))
                            })),
                    )
                    .child(
                        Button::new("config-locate")
                            .label(t(cx, "action-locate"))
                            .on_click(|_, _, cx| {
                                if let Ok(path) = paths::config_dir() {
                                    cx.reveal_path(&path);
                                }
                            }),
                    )
                    .child(
                        Button::new("config-reset")
                            .label(t(cx, "action-reset"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.config_confirm = true;
                                cx.notify();
                            })),
                    );
                if self.config_confirm {
                    content = content
                        .child(t(cx, "recovery-confirm"))
                        .child(
                            Button::new("config-confirm")
                                .label(t(cx, "action-confirm"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.config_confirm = false;
                                    this.config.update(cx, |owner, cx| {
                                        if let Err(error) =
                                            owner.repair(ConfigRepair::BackupAndReset, cx)
                                        {
                                            tracing::error!(%error, "prepare config reset failed");
                                        }
                                    });
                                })),
                        )
                        .child(
                            Button::new("config-cancel")
                                .label(t(cx, "action-cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.config_confirm = false;
                                    cx.notify();
                                })),
                        );
                }
            } else {
                content = content.child(self.settings.clone());
            }
        } else if let Some(problem) = &self.layout_problem {
            content = content
                .child(recovery(
                    t(cx, "recovery-layout-title"),
                    t(cx, "error-layout"),
                    cx,
                ))
                .child(problem.clone())
                .child(
                    Button::new("layout-reset")
                        .label(t(cx, "action-reset"))
                        .disabled(self.layout_task.is_some())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.layout_confirm = true;
                            cx.notify();
                        })),
                );
            if self.layout_confirm {
                content = content
                    .child(t(cx, "recovery-confirm"))
                    .child(
                        Button::new("layout-confirm")
                            .label(t(cx, "action-confirm"))
                            .disabled(self.layout_task.is_some())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.layout_task.is_some() || this.draining {
                                    return;
                                }
                                this.layout_task = Some(cx.spawn(async move |owner, cx| {
                                    let result = smol::unblock(|| {
                                        paths::config_dir()
                                            .map_err(|e| e.to_string())
                                            .and_then(|dir| layout::reset(&dir.join("state.toml")))
                                    })
                                    .await;
                                    let _ = owner.update(cx, |this, cx| {
                                        this.layout_problem = result.err();
                                        this.layout_task = None;
                                        this.layout_confirm = false;
                                        if this.layout_problem.is_none()
                                            && let Some(config) = &this.applied
                                        {
                                            this.applied_pi.update(cx, |pi, cx| {
                                                pi.request(config.pi_command.clone(), false, cx)
                                            });
                                        }
                                        cx.notify();
                                    });
                                }));
                            })),
                    )
                    .child(
                        Button::new("layout-cancel")
                            .label(t(cx, "action-cancel"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.layout_confirm = false;
                                cx.notify();
                            })),
                    );
            }
        } else {
            let pi = self.applied_pi.read(cx);
            if self.show_settings {
                content = content.child(self.settings.clone());
            } else if pi.operation.is_running() {
                content = content.child(
                    h_flex()
                        .gap_2()
                        .child(Spinner::new())
                        .child(t(cx, "startup-checking")),
                );
            } else if let Some(data) = self
                .applied
                .as_ref()
                .and_then(|config| pi.ready_for(config.pi_command.as_deref()))
            {
                content = content.child(home::home(data, cx));
            } else {
                content = content
                    .child(recovery(
                        t(cx, "recovery-pi-title"),
                        t(cx, "error-pi-probe"),
                        cx,
                    ))
                    .child(self.settings.clone());
            }
            let pi = self.applied_pi.read(cx);
            if let Some(problem) = pi.operation.problem() {
                content = content.child(div().text_sm().child(t(cx, problem.key())));
            }
            content = content.child(
                Button::new("probe-retry")
                    .label(t(cx, "action-check-pi"))
                    .disabled(pi.operation.is_running())
                    .on_click(cx.listener(|this, _, _, cx| {
                        let command = this.applied.as_ref().and_then(|c| c.pi_command.clone());
                        this.applied_pi
                            .update(cx, |pi, cx| pi.request(command, true, cx));
                    })),
            );
        }
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    h_flex().w_full().justify_between().child("Gupi").child(
                        Button::new("settings")
                            .icon(if self.show_settings {
                                crate::foundation::assets::IconName::ArrowLeft
                            } else {
                                crate::foundation::assets::IconName::Settings
                            })
                            .accessibility_label(t(
                                cx,
                                if self.show_settings {
                                    "menu-show-main"
                                } else {
                                    "menu-settings"
                                },
                            ))
                            .tooltip(t(
                                cx,
                                if self.show_settings {
                                    "menu-show-main"
                                } else {
                                    "menu-settings"
                                },
                            ))
                            .disabled(self.draining || !configured)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.focus_handle.focus(window, cx);
                                this.show_settings = !this.show_settings;
                                cx.notify();
                            })),
                    ),
                ),
            )
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .when(configured, |view| view.overflow_y_scroll())
                    .when(!configured, |view| view.overflow_hidden())
                    .py_8()
                    .when(!onboarding, |view| view.px_8())
                    .flex()
                    .justify_center()
                    .when(configured, |view| view.items_start())
                    .child(content),
            )
    }
}
