use super::{chrome, home, settings::SettingsView};
mod palette;
#[cfg(test)]
mod tests;
use crate::{
    app::menus,
    components::recovery::recovery,
    foundation::{
        i18n::{self, t},
        paths,
    },
    pi::PiProbeController,
    state::{
        config::{AppConfig, ConfigContents, ConfigController, ConfigRepair},
        layout, theme,
    },
};
use gpui_form::Form;
use gpui_kit::component::{
    ActiveTheme, Disableable, Root, WindowExt, button::Button, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

enum StartupScreen {
    Quitting,
    LoadingConfig,
    ConfigFailure(&'static str),
    Onboarding,
    Settings,
    Home(std::path::PathBuf),
}
impl StartupScreen {
    fn configured(&self) -> bool {
        matches!(self, Self::Settings | Self::Home(_))
    }
}

pub(crate) struct StartupView {
    focus_handle: FocusHandle,
    pub config: Entity<ConfigController>,
    applied_pi: Entity<PiProbeController>,
    draft_pi: Entity<PiProbeController>,
    settings: Entity<SettingsView>,
    home: Option<Entity<home::HomeView>>,
    pub show_settings: bool,
    config_confirm: bool,
    log_warning: bool,
    _subscriptions: Vec<Subscription>,
    quit_task: Option<Task<()>>,
    palette: Option<Entity<super::command_palette::CommandPalette>>,
}
impl StartupView {
    pub fn new(log_warning: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let config = cx.new(|cx| ConfigController::new(&form, cx));
        let applied_pi = cx.new(|_| PiProbeController::new());
        let draft_pi = cx.new(|_| PiProbeController::new());
        cx.global_mut::<crate::app::temporary::Temporary>().config = Some(config.clone());
        let settings = cx.new(|cx| {
            SettingsView::new(
                form.clone(),
                config.clone(),
                draft_pi.clone(),
                applied_pi.clone(),
                focus_handle.clone(),
                window,
                cx,
            )
        });
        let store = config.read(cx).store.clone();
        let config_sub = store.observe_in(cx, window, |this, op, window, cx| {
            if let Some(value) = op.data().and_then(|d| d.configured()) {
                crate::app::temporary::set_command(value.pi_executable(), cx);
                i18n::apply(value.language, cx);
                theme::apply(value, window, cx);
                crate::state::keybindings::apply(&value.keybindings, cx);
                if !op.is_running() {
                    crate::app::shortcuts::apply(value, cx);
                }
                menus::refresh(cx);
                if this.show_settings {
                    this.applied_pi
                        .update(cx, |pi, cx| pi.request(value.pi_command.clone(), false, cx));
                }
            }
            cx.notify();
        });
        let form_sub = cx.observe_in(&form, window, move |this, _, window, cx| {
            let draft = this.config.read(cx).preferences(cx);
            i18n::apply(draft.language, cx);
            theme::apply(&draft, window, cx);
            menus::refresh(cx);
        });
        let preview_config = config.clone();
        let appearance = window.observe_window_appearance(move |window, cx| {
            theme::apply(&preview_config.read(cx).preferences(cx), window, cx);
        });
        let accent_config = config.clone();
        let accent = cx.observe_global_in::<app_theme::SystemAccentThemeState>(
            window,
            move |_, window, cx| {
                theme::apply(&accent_config.read(cx).preferences(cx), window, cx);
            },
        );
        config.update(cx, |owner, cx| owner.reload(cx));
        Self {
            focus_handle,
            config,
            applied_pi,
            draft_pi,
            settings,
            home: None,
            show_settings: false,
            config_confirm: false,
            log_warning,
            _subscriptions: vec![config_sub, form_sub, appearance, accent],
            quit_task: None,
            palette: None,
        }
    }
    pub fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_quitting() {
            return;
        }
        tracing::info!("managed quit started");
        crate::app::shortcuts::shutdown(cx);
        self.config.update(cx, |owner, _| owner.draining = true);
        self.applied_pi.update(cx, |pi, _| pi.stop());
        self.draft_pi.update(cx, |pi, _| pi.stop());
        self.settings
            .update(cx, |settings, cx| settings.stop_resources(cx));
        let pi = crate::state::pi::global(cx);
        let flush_temporary = crate::app::temporary::drain(cx);
        let close_pi = pi.update(cx, |state, cx| state.close_all(cx));
        let flush_home = self
            .home
            .as_ref()
            .map(|home| home.update(cx, |home, cx| home.flush(cx)));
        let placement = layout::capture(window, cx.global::<layout::LayoutState>());
        self.quit_task = Some(cx.spawn(async move |owner, cx| {
            loop {
                let busy = owner
                    .read_with(cx, |owner, cx| {
                        owner.config.read(cx).store.read(cx, |op| op.is_running())
                            || owner.applied_pi.read(cx).operation.is_running()
                            || owner.draft_pi.read(cx).operation.is_running()
                    })
                    .unwrap_or(false);
                if !busy {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(20))
                    .await;
            }
            if let Some(flush) = flush_temporary {
                flush.await;
            }
            if let Some(flush) = flush_home {
                flush.await;
            }
            let result = smol::unblock(move || {
                paths::config_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| layout::save(&dir.join("state.toml"), &placement))
            })
            .await;
            if let Err(error) = result {
                tracing::error!(%error, "layout save failed");
            }
            let _ = close_pi.await;
            tracing::info!("managed quit completed");
            cx.update(|cx| cx.quit());
        }));
        cx.notify();
    }
    pub fn set_settings_visible(
        &mut self,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_settings != visible {
            if let Some(home) = &self.home {
                home.update(cx, |home, cx| home.close_commands(window, cx));
            }
            if self.palette.take().is_some_and(|p| p.read(cx).is_open)
                && window.has_active_dialog(cx)
            {
                window.close_dialog(cx);
            }
        }
        if visible && !self.show_settings {
            self.settings
                .update(cx, |settings, cx| settings.activate(cx));
        }
        self.show_settings = visible;
        if !visible && let Some(home) = &self.home {
            home.update(cx, |home, cx| home.focus_composer(window, cx));
        } else {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }
    pub fn is_quitting(&self) -> bool {
        self.quit_task.is_some()
    }
    fn screen(&self, cx: &App) -> StartupScreen {
        if self.is_quitting() {
            return StartupScreen::Quitting;
        }
        self.config
            .read(cx)
            .store
            .read(cx, |op| match op.data().map(|data| &data.contents) {
                Some(ConfigContents::Missing) => StartupScreen::Onboarding,
                Some(ConfigContents::Configured(config)) => {
                    if self.show_settings {
                        StartupScreen::Settings
                    } else {
                        StartupScreen::Home(config.pi_executable())
                    }
                }
                None => match op.problem() {
                    Some(problem) if !op.is_running() => {
                        StartupScreen::ConfigFailure(problem.key())
                    }
                    _ => StartupScreen::LoadingConfig,
                },
            })
    }
}
impl Render for StartupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = self.screen(cx);
        let configured = screen.configured();
        let main = matches!(screen, StartupScreen::Home(_));
        let onboarding = matches!(screen, StartupScreen::Onboarding);
        let settings_page = matches!(screen, StartupScreen::Settings);
        let page_title = match &screen {
            StartupScreen::Settings => t(cx, "menu-settings"),
            StartupScreen::ConfigFailure(_) => t(cx, "recovery-config-title"),
            StartupScreen::Onboarding => t(cx, "startup-welcome"),
            StartupScreen::Quitting => t(cx, "startup-quitting"),
            StartupScreen::LoadingConfig | StartupScreen::Home(_) => t(cx, "app-title"),
        };
        if !main {
            window.set_window_title(&page_title);
        }
        if let StartupScreen::Home(command) = &screen {
            if let Some(home) = &self.home {
                home.read(cx)
                    .state
                    .clone()
                    .update(cx, |state, _| state.set_command(command.clone()));
            } else {
                self.home = Some(cx.new(|cx| home::HomeView::new(command.clone(), window, cx)));
            }
        }
        let mut content = v_flex()
            .gap_5()
            .w_full()
            .when(!onboarding && !settings_page, |view| view.max_w(px(800.)))
            .when(settings_page, |view| view.h_full().min_h_0())
            .when(!configured, |view| view.h_full().min_h_0());
        if self.log_warning {
            content = content.child(t(cx, "error-log"));
        }
        match screen {
            StartupScreen::Quitting => content = content.child(t(cx, "startup-quitting")),
            StartupScreen::LoadingConfig => content = content.child(t(cx, "startup-checking")),
            StartupScreen::ConfigFailure(problem) => {
                content = content
                    .child(recovery(t(cx, "recovery-config-title"), t(cx, problem), cx))
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
            }
            StartupScreen::Onboarding | StartupScreen::Settings => {
                content = content.child(self.settings.clone())
            }
            StartupScreen::Home(_) => {}
        }
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .when(!main, |view| {
                view.child(
                    chrome::title_bar(cx).child(
                        h_flex()
                            .size_full()
                            .pl(chrome::leading_space(window))
                            .pr_3()
                            .gap_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .when(self.show_settings, |view| {
                                view.child(chrome::control(
                                    "settings-back-control",
                                    chrome::button("settings-back")
                                        .icon(crate::foundation::assets::IconName::ArrowLeft)
                                        .accessibility_label(t(cx, "menu-show-main"))
                                        .tooltip(t(cx, "menu-show-main"))
                                        .disabled(self.is_quitting() || !configured)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_settings_visible(false, window, cx);
                                        })),
                                ))
                            })
                            .child(page_title),
                    ),
                )
            })
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .when(configured && !main && !settings_page, |view| {
                        view.overflow_y_scroll()
                    })
                    .when(!configured || main || settings_page, |view| {
                        view.overflow_hidden()
                    })
                    .when(!main && !settings_page, |view| view.py_8())
                    .when(!onboarding && !main && !settings_page, |view| view.px_8())
                    .flex()
                    .justify_center()
                    .when(configured, |view| view.items_start())
                    .child(if main {
                        self.home.clone().unwrap().into_any_element()
                    } else {
                        content.into_any_element()
                    }),
            )
            .key_context("GupiApplication")
            .on_action(
                cx.listener(|this, _: &menus::ShowCommandPalette, window, cx| {
                    this.open_palette(window, cx)
                }),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
