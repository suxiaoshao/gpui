use gpui::*;
use gpui_component::{
    ActiveTheme as _, Sizable, WindowExt as _,
    notification::{Notification, NotificationType},
    spinner::Spinner,
    v_flex,
};

use crate::{
    components::resource::{
        CriticalResourceAction, CriticalResourceProblem, CriticalResourcesView,
    },
    database::{self, DatabasePhase, DatabaseResource},
    features::{conversation::resources, home::HomeView},
    foundation::I18n,
    state::{self, config::ConfigOperation},
};

pub(crate) struct JacoRoot {
    focus_handle: FocusHandle,
    home: Option<Entity<HomeView>>,
    _theme_binding: state::theme::WindowThemeBinding,
    _subscriptions: Vec<Subscription>,
}

impl JacoRoot {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = state::config::store(cx);
        let database = database::store(cx);
        let conversation_resources = resources::store(cx);
        let shutdown = crate::app::AppShutdownStore::global(cx);
        let mut root = Self {
            focus_handle: cx.focus_handle(),
            home: None,
            _theme_binding: state::theme::WindowThemeBinding::new(window, cx),
            _subscriptions: Vec::new(),
        };
        root._subscriptions.push(config.observe_select_in(
            cx,
            window,
            state::config::SelectConfigGateStatus,
            |_root, _status, _window, cx| cx.notify(),
        ));
        root._subscriptions.push(
            database.observe_in(cx, window, |root, _, window, cx| root.sync_home(window, cx)),
        );
        root._subscriptions.push(conversation_resources.observe_in(
            cx,
            window,
            |root, _, window, cx| root.sync_home(window, cx),
        ));
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
        let resources = resources::ready_data(cx);
        if let Some(resources) = resources {
            if self.home.is_none() && database::is_ready(cx) {
                self.home = Some(cx.new(|cx| HomeView::new(resources.runtime, window, cx)));
            }
        } else {
            self.home = None;
        }
        cx.notify();
    }

    pub(crate) fn reload_app_menu_bar(&mut self, cx: &mut Context<Self>) {
        if let Some(home) = self.home.as_ref() {
            home.update(cx, |home, cx| home.reload_app_menu_bar(cx));
        }
    }

    pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(home) = &self.home {
            home.update(cx, |home, cx| home.focus_chat_form(window, cx));
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    fn render_config(&self, cx: &mut Context<Self>) -> AnyElement {
        let (phase, running, problem, actions) = state::config::store(cx).read(cx, |operation| {
            let problem = operation.problem();
            let mut actions = vec![CriticalResourceAction::ReloadConfig];
            if problem
                .is_some_and(|problem| problem.supports(state::config::ConfigRepair::RetryWrite))
            {
                actions.push(CriticalResourceAction::RetryConfigWrite);
            }
            if problem.is_some_and(|problem| {
                problem.supports(state::config::ConfigRepair::BackupAndCreateDefault)
            }) {
                actions.push(CriticalResourceAction::BackupAndCreateDefaultConfig);
            }
            if problem.is_some_and(|problem| {
                problem.supports(state::config::ConfigRepair::BackupAndOverwritePending)
            }) {
                actions.push(CriticalResourceAction::BackupAndOverwritePendingConfig);
            }
            (
                operation.phase(),
                operation.is_running(),
                problem.map(ToString::to_string),
                actions,
            )
        });
        match phase {
            gpui_operation::repair::Phase::Idle | gpui_operation::repair::Phase::Loading => {
                CriticalResourcesView::loading(cx.global::<I18n>().t("critical-config-loading"))
                    .into_any_element()
            }
            gpui_operation::repair::Phase::Ready | gpui_operation::repair::Phase::Refreshing => {
                CriticalResourcesView::loading(cx.global::<I18n>().t("critical-database-loading"))
                    .into_any_element()
            }
            gpui_operation::repair::Phase::Unavailable
            | gpui_operation::repair::Phase::RepairingUnavailable
            | gpui_operation::repair::Phase::Degraded
            | gpui_operation::repair::Phase::RepairingDegraded => {
                CriticalResourcesView::problem(CriticalResourceProblem {
                    id: "critical-config-error",
                    title: cx.global::<I18n>().t("critical-config-error-title").into(),
                    message: problem.unwrap_or_default().into(),
                    running,
                    warning: false,
                    actions,
                })
                .into_any_element()
            }
        }
    }

    fn config_problem_layer(&self, cx: &mut Context<Self>) -> CriticalResourcesView {
        let (running, message, actions) = state::config::store(cx).read(cx, |operation| {
            let problem = operation.problem();
            let mut actions = vec![CriticalResourceAction::ReloadConfig];
            if problem
                .is_some_and(|problem| problem.supports(state::config::ConfigRepair::RetryWrite))
            {
                actions.push(CriticalResourceAction::RetryConfigWrite);
            }
            if problem.is_some_and(|problem| {
                problem.supports(state::config::ConfigRepair::BackupAndCreateDefault)
            }) {
                actions.push(CriticalResourceAction::BackupAndCreateDefaultConfig);
            }
            if problem.is_some_and(|problem| {
                problem.supports(state::config::ConfigRepair::BackupAndOverwritePending)
            }) {
                actions.push(CriticalResourceAction::BackupAndOverwritePendingConfig);
            }
            (
                operation.is_running(),
                operation.problem().map(ToString::to_string),
                actions,
            )
        });
        CriticalResourcesView::problem(CriticalResourceProblem {
            id: "critical-config-error",
            title: cx.global::<I18n>().t("critical-config-error-title").into(),
            message: message
                .unwrap_or_else(|| cx.global::<I18n>().t("critical-read-only-description"))
                .into(),
            running,
            warning: true,
            actions,
        })
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
            None => {
                CriticalResourcesView::loading(cx.global::<I18n>().t("critical-database-loading"))
                    .into_any_element()
            }
            Some((phase, running, problem, can_create_fresh)) => match phase {
                DatabasePhase::Idle | DatabasePhase::Loading => CriticalResourcesView::loading(
                    cx.global::<I18n>().t("critical-database-loading"),
                )
                .into_any_element(),
                DatabasePhase::Ready => self
                    .home
                    .as_ref()
                    .cloned()
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| self.render_session_pending(cx)),
                DatabasePhase::Refreshing if self.home.is_some() => self.database_problem(
                    cx.global::<I18n>().t("critical-read-only-description"),
                    running,
                    false,
                    true,
                    cx,
                ),
                DatabasePhase::Refreshing => CriticalResourcesView::loading(
                    cx.global::<I18n>().t("critical-database-loading"),
                )
                .into_any_element(),
                DatabasePhase::Retiring | DatabasePhase::Unavailable | DatabasePhase::Repairing
                    if self.home.is_some() =>
                {
                    self.database_problem(
                        problem.unwrap_or_default(),
                        running,
                        can_create_fresh,
                        true,
                        cx,
                    )
                }
                DatabasePhase::Retiring | DatabasePhase::Unavailable | DatabasePhase::Repairing => {
                    self.database_problem(
                        problem.unwrap_or_default(),
                        running,
                        can_create_fresh,
                        false,
                        cx,
                    )
                }
            },
        }
    }

    fn database_problem(
        &self,
        message: String,
        running: bool,
        can_create_fresh: bool,
        overlay_home: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut actions = vec![CriticalResourceAction::RefreshDatabase];
        if can_create_fresh {
            actions.push(CriticalResourceAction::BackupAndCreateFreshDatabase);
        }
        let view = CriticalResourcesView::problem(CriticalResourceProblem {
            id: "critical-database-error",
            title: cx
                .global::<I18n>()
                .t("critical-database-error-title")
                .into(),
            message: message.into(),
            running,
            warning: overlay_home,
            actions,
        });
        if overlay_home {
            view.overlay(self.home.as_ref().expect("checked home").clone(), cx)
        } else {
            view.into_any_element()
        }
    }

    fn render_session_pending(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = resources::store(cx).read(cx, |resources| match resources {
            resources::ConversationResourcesState::Failed(message) => Some(message.clone()),
            resources::ConversationResourcesState::AwaitingDatabase
            | resources::ConversationResourcesState::Ready(_) => None,
        });
        let path = database::store(cx).read(cx, |resource| match resource {
            DatabaseResource::Bound { target, .. } => Some(target.database_path.clone()),
            DatabaseResource::AwaitingConfig => None,
        });
        let Some(message) = message else {
            return CriticalResourcesView::loading(
                cx.global::<I18n>().t("critical-session-loading"),
            )
            .into_any_element();
        };
        let mut args = fluent_bundle::FluentArgs::new();
        args.set(
            "path",
            path.map(|path| path.display().to_string())
                .unwrap_or_default(),
        );
        args.set("message", message);
        CriticalResourcesView::problem(CriticalResourceProblem {
            id: "critical-session-error",
            title: cx.global::<I18n>().t("critical-session-error-title").into(),
            message: cx
                .global::<I18n>()
                .t_with_args("critical-session-error-description", &args)
                .into(),
            running: false,
            warning: false,
            actions: vec![CriticalResourceAction::RetryConversationResources],
        })
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
        if crate::app::AppShutdownStore::global(cx)
            .read(cx, |phase| *phase == crate::app::AppShutdownPhase::Draining)
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
        if !config_exact_ready && let Some(home) = self.home.as_ref() {
            return self.config_problem_layer(cx).overlay(home.clone(), cx);
        }
        self.render_database(cx)
    }
}
