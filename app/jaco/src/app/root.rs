use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Sizable, WindowExt as _,
    alert::Alert,
    button::Button,
    dialog::DialogButtonProps,
    notification::{Notification, NotificationType},
    spinner::Spinner,
    v_flex,
};

use crate::{
    database::{self, DatabaseResource},
    features::home::HomeView,
    foundation::I18n,
    state::{self, config::ConfigOperation},
};

pub(crate) struct JacoRoot {
    focus_handle: FocusHandle,
    home: Option<Entity<HomeView>>,
    home_binding: Option<state::session::DatabaseBinding>,
    runtime: Option<Entity<state::conversation_runtime::ConversationRuntimeStore>>,
    workspace: Option<Entity<state::JacoWorkspaceStore>>,
    _subscriptions: Vec<Subscription>,
}

impl JacoRoot {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = state::config::store(cx);
        let database = database::store(cx);
        let shutdown = super::AppShutdownStore::global(cx);
        let mut root = Self {
            focus_handle: cx.focus_handle(),
            home: None,
            home_binding: None,
            runtime: None,
            workspace: None,
            _subscriptions: Vec::new(),
        };
        root._subscriptions
            .push(config.observe_in(cx, window, |root, _, window, cx| root.sync_home(window, cx)));
        root._subscriptions.push(config.observe_select_in(
            cx,
            window,
            state::selectors::SelectAppPresentation::current(cx),
            |root, _presentation, window, cx| root.apply_presentation(window, cx),
        ));
        root._subscriptions.push(
            database.observe_in(cx, window, |root, _, window, cx| root.sync_home(window, cx)),
        );
        root._subscriptions
            .push(shutdown.observe_in(cx, window, |_root, _, _window, cx| cx.notify()));
        root.sync_home(window, cx);
        root
    }

    fn sync_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(outcome) = database::take_backup_outcome(cx) {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("path", outcome.backup_dir.display().to_string());
            window.push_notification(
                Notification::new()
                    .title(cx.global::<I18n>().t("critical-database-backup-fresh"))
                    .message(
                        cx.global::<I18n>()
                            .t_with_args("critical-database-backup-succeeded", &args),
                    )
                    .with_type(NotificationType::Success),
                cx,
            );
            if let Some(error) = outcome.fresh_error {
                tracing::error!(
                    %error,
                    "database backup succeeded but fresh database creation failed"
                );
            }
        }
        database::sync_from_config(cx);
        if let Err(error) = super::init_ready_services(cx) {
            tracing::error!(?error, "initialize ready Jaco services failed");
        }
        let binding = database::store(cx).read(cx, |resource| match resource {
            DatabaseResource::Bound { operation, .. } => {
                operation.data().map(|data| data.binding.clone())
            }
            DatabaseResource::AwaitingConfig => None,
        });
        if binding != self.home_binding {
            if let Some(runtime) = self.runtime.take() {
                runtime
                    .update(cx, |runtime, cx| runtime.shutdown_all(cx))
                    .detach();
            }
            self.workspace = None;
            self.home = None;
            self.home_binding = binding.clone();
        }
        if binding.is_some() {
            if self.home.is_none() && database::is_ready(cx) {
                if self.runtime.is_none() {
                    match state::conversation_runtime::create(cx) {
                        Ok(runtime) => self.runtime = Some(runtime),
                        Err(error) => {
                            tracing::error!(?error, "initialize conversation runtime failed");
                            return;
                        }
                    }
                }
                if self.workspace.is_none() {
                    self.workspace = Some(state::workspace::create(cx));
                }
                let workspace = self
                    .workspace
                    .as_ref()
                    .expect("workspace was initialized")
                    .clone();
                let runtime = self
                    .runtime
                    .as_ref()
                    .expect("conversation runtime was initialized")
                    .clone();
                self.home = Some(cx.new(|cx| HomeView::new(workspace, runtime, window, cx)));
            }
            if let Some(runtime) = self.runtime.as_ref() {
                state::conversation_runtime::retry_recovery_if_needed(runtime, cx);
            }
        } else {
            self.home = None;
            self.home_binding = None;
            self.runtime = None;
            self.workspace = None;
        }
        cx.notify();
    }

    fn apply_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::foundation::init_i18n(cx);
        super::menus::sync_app_menus(cx);
        state::theme::apply_current_theme(window, cx);
        if let Some(home) = self.home.as_ref() {
            home.update(cx, |home, cx| home.reload_app_menu_bar(cx));
        }
        cx.refresh_windows();
    }

    pub(crate) fn runtime(
        &self,
    ) -> Option<Entity<state::conversation_runtime::ConversationRuntimeStore>> {
        self.runtime.clone()
    }

    pub(crate) fn workspace(&self) -> Option<Entity<state::JacoWorkspaceStore>> {
        self.workspace.clone()
    }

    pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(home) = &self.home {
            home.update(cx, |home, cx| home.focus_chat_form(window, cx));
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    fn render_config(&self, cx: &mut Context<Self>) -> AnyElement {
        let (phase, running, problem) = state::config::store(cx).read(cx, |operation| {
            (
                operation.phase(),
                operation.is_running(),
                operation.problem().map(ToString::to_string),
            )
        });
        match phase {
            gpui_operation::repair::Phase::Idle | gpui_operation::repair::Phase::Loading => {
                loading_page(cx.global::<I18n>().t("critical-config-loading"))
            }
            gpui_operation::repair::Phase::Ready | gpui_operation::repair::Phase::Refreshing => {
                loading_page(cx.global::<I18n>().t("critical-database-loading"))
            }
            gpui_operation::repair::Phase::Unavailable
            | gpui_operation::repair::Phase::RepairingUnavailable
            | gpui_operation::repair::Phase::Degraded
            | gpui_operation::repair::Phase::RepairingDegraded => {
                self.config_problem(problem.unwrap_or_default(), running, cx)
            }
        }
    }

    fn config_problem(&self, message: String, running: bool, cx: &mut Context<Self>) -> AnyElement {
        let (retry_write, create_default, overwrite_pending) =
            state::config::store(cx).read(cx, |operation| {
                let problem = operation.problem();
                (
                    problem.is_some_and(|problem| {
                        problem.supports(state::config::ConfigRepair::RetryWrite)
                    }),
                    problem.is_some_and(|problem| {
                        problem.supports(state::config::ConfigRepair::BackupAndCreateDefault)
                    }),
                    problem.is_some_and(|problem| {
                        problem.supports(state::config::ConfigRepair::BackupAndOverwritePending)
                    }),
                )
            });
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Alert::error("critical-config-error", message)
                    .title(cx.global::<I18n>().t("critical-config-error-title")),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Button::new("critical-config-reload")
                            .label(cx.global::<I18n>().t("critical-config-reload"))
                            .loading(running)
                            .disabled(running)
                            .on_click(|_, _, cx| state::config::request_reload(cx)),
                    )
                    .children(retry_write.then(|| {
                        Button::new("critical-config-retry-write")
                            .label(cx.global::<I18n>().t("critical-config-retry-write"))
                            .disabled(running)
                            .on_click(|_, _, cx| {
                                if let Err(error) = state::config::request_repair(
                                    state::config::ConfigRepair::RetryWrite,
                                    cx,
                                ) {
                                    tracing::error!(?error, "retry config write failed");
                                }
                            })
                    }))
                    .children(create_default.then(|| {
                        Button::new("critical-config-backup-default")
                            .label(cx.global::<I18n>().t("critical-config-backup-default"))
                            .disabled(running)
                            .on_click(|_, window, cx| {
                                confirm_config_repair(
                                    state::config::ConfigRepair::BackupAndCreateDefault,
                                    window,
                                    cx,
                                );
                            })
                    }))
                    .children(overwrite_pending.then(|| {
                        Button::new("critical-config-backup-overwrite")
                            .label(cx.global::<I18n>().t("critical-config-backup-overwrite"))
                            .disabled(running)
                            .on_click(|_, window, cx| {
                                confirm_config_repair(
                                    state::config::ConfigRepair::BackupAndOverwritePending,
                                    window,
                                    cx,
                                );
                            })
                    })),
            )
            .into_any_element()
    }

    fn render_database(&self, cx: &mut Context<Self>) -> AnyElement {
        let snapshot = database::store(cx).read(cx, |resource| match resource {
            DatabaseResource::AwaitingConfig => None,
            DatabaseResource::Bound { operation, .. } => Some((
                operation.phase(),
                operation.is_running(),
                operation.problem().map(ToString::to_string),
                operation
                    .problem()
                    .is_some_and(database::DatabaseProblem::can_create_fresh),
            )),
        });
        match snapshot {
            None => loading_page(cx.global::<I18n>().t("critical-database-loading")),
            Some((phase, running, problem, can_create_fresh)) => match phase {
                gpui_operation::repair::Phase::Idle | gpui_operation::repair::Phase::Loading => {
                    loading_page(cx.global::<I18n>().t("critical-database-loading"))
                }
                gpui_operation::repair::Phase::Ready => self
                    .home
                    .as_ref()
                    .cloned()
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| {
                        loading_page(cx.global::<I18n>().t("critical-session-loading"))
                    }),
                gpui_operation::repair::Phase::Refreshing => self.render_read_only_home(cx),
                gpui_operation::repair::Phase::Unavailable
                | gpui_operation::repair::Phase::RepairingUnavailable
                | gpui_operation::repair::Phase::Degraded
                | gpui_operation::repair::Phase::RepairingDegraded
                    if self.home.is_some() =>
                {
                    self.render_read_only_home(cx)
                }
                gpui_operation::repair::Phase::Unavailable
                | gpui_operation::repair::Phase::RepairingUnavailable
                | gpui_operation::repair::Phase::Degraded
                | gpui_operation::repair::Phase::RepairingDegraded => v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .child(
                        Alert::error("critical-database-error", problem.unwrap_or_default())
                            .title(cx.global::<I18n>().t("critical-database-error-title")),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                Button::new("critical-database-refresh")
                                    .label(cx.global::<I18n>().t("critical-database-refresh"))
                                    .loading(running)
                                    .disabled(running)
                                    .on_click(|_, _, cx| database::request_refresh(cx)),
                            )
                            .children(can_create_fresh.then(|| {
                                Button::new("critical-database-backup-fresh")
                                    .label(cx.global::<I18n>().t("critical-database-backup-fresh"))
                                    .disabled(running)
                                    .on_click(|_, window, cx| {
                                        confirm_database_repair(window, cx);
                                    })
                            })),
                    )
                    .into_any_element(),
            },
        }
    }

    fn render_read_only_home(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(home) = self.home.as_ref().cloned() else {
            return loading_page(cx.global::<I18n>().t("critical-session-loading"));
        };
        div()
            .relative()
            .size_full()
            .child(home)
            .child(
                v_flex()
                    .absolute()
                    .inset_0()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .bg(cx.theme().background.opacity(0.9))
                    .child(
                        Alert::warning(
                            "critical-read-only",
                            cx.global::<I18n>().t("critical-read-only-description"),
                        )
                        .title(cx.global::<I18n>().t("critical-read-only-title")),
                    ),
            )
            .into_any_element()
    }

    fn render_shutdown(&self, cx: &mut Context<Self>) -> AnyElement {
        let status = v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(cx.theme().background.opacity(0.92))
            .child(Spinner::new().large())
            .child(cx.global::<I18n>().t("app-shutdown-draining"));
        div()
            .relative()
            .size_full()
            .children(self.home.as_ref().cloned())
            .child(status)
            .into_any_element()
    }
}

impl Focusable for JacoRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for JacoRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if super::AppShutdownStore::global(cx)
            .read(cx, |phase| *phase == super::AppShutdownPhase::Draining)
        {
            return self.render_shutdown(cx);
        }
        let (config_has_data, config_exact_ready) =
            state::config::store(cx).read(cx, |operation| {
                (
                    operation.data().is_some(),
                    matches!(operation, ConfigOperation::Ready(_)),
                )
            });
        if !config_has_data {
            return self.render_config(cx);
        }
        if !config_exact_ready && self.home.is_some() {
            return self.render_read_only_home(cx);
        }
        self.render_database(cx)
    }
}

fn loading_page(label: String) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .child(Spinner::new().large())
        .child(label)
        .into_any_element()
}

fn confirm_config_repair(repair: state::config::ConfigRepair, window: &mut Window, cx: &mut App) {
    let title = cx.global::<I18n>().t("critical-config-confirm-title");
    let description = cx.global::<I18n>().t("critical-config-confirm-description");
    window.open_dialog(cx, move |dialog, _, cx| {
        dialog
            .title(title.clone())
            .child(description.clone())
            .button_props(
                DialogButtonProps::default()
                    .show_cancel(true)
                    .cancel_text(cx.global::<I18n>().t("critical-action-cancel"))
                    .ok_text(cx.global::<I18n>().t("critical-action-confirm"))
                    .on_ok(move |_, _, cx| {
                        if let Err(error) = state::config::request_repair(repair, cx) {
                            tracing::error!(?error, "repair config failed");
                        }
                        true
                    }),
            )
    });
}

fn confirm_database_repair(window: &mut Window, cx: &mut App) {
    let title = cx.global::<I18n>().t("critical-database-confirm-title");
    let description = cx
        .global::<I18n>()
        .t("critical-database-confirm-description");
    window.open_dialog(cx, move |dialog, _, cx| {
        dialog
            .title(title.clone())
            .child(description.clone())
            .button_props(
                DialogButtonProps::default()
                    .show_cancel(true)
                    .cancel_text(cx.global::<I18n>().t("critical-action-cancel"))
                    .ok_text(cx.global::<I18n>().t("critical-action-confirm"))
                    .on_ok(|_, window, cx| {
                        choose_database_backup_path(window, cx);
                        true
                    }),
            )
    });
}

fn choose_database_backup_path(window: &mut Window, cx: &mut App) {
    let initial_dir = database::store(cx).read(cx, |resource| match resource {
        DatabaseResource::Bound { target, .. } => target.data_dir.clone(),
        DatabaseResource::AwaitingConfig => std::env::temp_dir(),
    });
    let suggested = format!(
        "jaco-database-backup-{}",
        time::OffsetDateTime::now_utc().unix_timestamp()
    );
    let prompt = cx.prompt_for_new_path(&initial_dir, Some(&suggested));
    let window_handle = window.window_handle();
    window
        .spawn(cx, async move |cx| {
            let backup_dir = match prompt.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    tracing::error!(?error, "choose database backup path failed");
                    return;
                }
                Err(error) => {
                    tracing::error!(?error, "database backup path prompt canceled");
                    return;
                }
            };
            if let Err(error) =
                window_handle.update(
                    cx,
                    |_, _window, cx| match database::backup_and_create_fresh(backup_dir.clone(), cx)
                    {
                        Ok(()) => {}
                        Err(error) => tracing::error!(?error, "repair database failed"),
                    },
                )
            {
                tracing::error!(?error, "complete database repair failed");
            }
        })
        .detach();
}
