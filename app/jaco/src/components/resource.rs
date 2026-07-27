use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Sizable, WindowExt as _, alert::Alert, button::Button,
    dialog::DialogButtonProps, spinner::Spinner, v_flex,
};

use crate::{
    app,
    database::{self, DatabaseResource},
    foundation::I18n,
    state,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CriticalResourceAction {
    ReloadConfig,
    RetryConfigWrite,
    BackupAndCreateDefaultConfig,
    BackupAndOverwritePendingConfig,
    RefreshDatabase,
    BackupAndCreateFreshDatabase,
    RetrySession,
}

#[derive(Clone)]
pub(crate) struct CriticalResourceProblem {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) message: SharedString,
    pub(crate) running: bool,
    pub(crate) warning: bool,
    pub(crate) actions: Vec<CriticalResourceAction>,
}

#[derive(IntoElement)]
pub(crate) enum CriticalResourcesView {
    Loading { label: SharedString },
    Problem(CriticalResourceProblem),
}

impl CriticalResourcesView {
    pub(crate) fn loading(label: impl Into<SharedString>) -> Self {
        Self::Loading {
            label: label.into(),
        }
    }

    pub(crate) fn problem(problem: CriticalResourceProblem) -> Self {
        Self::Problem(problem)
    }

    pub(crate) fn overlay(self, content: impl IntoElement, cx: &App) -> AnyElement {
        div()
            .relative()
            .size_full()
            .child(content)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().background.opacity(0.92))
                    .child(self),
            )
            .into_any_element()
    }
}

impl RenderOnce for CriticalResourcesView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            Self::Loading { label } => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(Spinner::new().large())
                .child(label)
                .into_any_element(),
            Self::Problem(problem) => {
                let alert = if problem.warning {
                    Alert::warning(problem.id, problem.message).title(problem.title)
                } else {
                    Alert::error(problem.id, problem.message).title(problem.title)
                };
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .child(alert)
                    .child(
                        v_flex().gap_2().children(
                            problem
                                .actions
                                .into_iter()
                                .map(|action| action_button(action, problem.running, cx)),
                        ),
                    )
                    .into_any_element()
            }
        }
    }
}

fn action_button(action: CriticalResourceAction, running: bool, cx: &App) -> Button {
    let (id, label) = match action {
        CriticalResourceAction::ReloadConfig => {
            ("critical-config-reload", "critical-config-reload")
        }
        CriticalResourceAction::RetryConfigWrite => {
            ("critical-config-retry-write", "critical-config-retry-write")
        }
        CriticalResourceAction::BackupAndCreateDefaultConfig => (
            "critical-config-backup-default",
            "critical-config-backup-default",
        ),
        CriticalResourceAction::BackupAndOverwritePendingConfig => (
            "critical-config-backup-overwrite",
            "critical-config-backup-overwrite",
        ),
        CriticalResourceAction::RefreshDatabase => {
            ("critical-database-refresh", "critical-database-refresh")
        }
        CriticalResourceAction::BackupAndCreateFreshDatabase => (
            "critical-database-backup-fresh",
            "critical-database-backup-fresh",
        ),
        CriticalResourceAction::RetrySession => {
            ("critical-session-retry", "critical-session-retry")
        }
    };
    Button::new(id)
        .label(cx.global::<I18n>().t(label))
        .loading(running)
        .disabled(running)
        .on_click(move |_, window, cx| run_action(action, window, cx))
}

fn run_action(action: CriticalResourceAction, window: &mut Window, cx: &mut App) {
    match action {
        CriticalResourceAction::ReloadConfig => state::config::request_reload(cx),
        CriticalResourceAction::RetryConfigWrite => {
            if let Err(error) =
                state::config::request_repair(state::config::ConfigRepair::RetryWrite, cx)
            {
                tracing::error!(?error, "retry config write failed");
            }
        }
        CriticalResourceAction::BackupAndCreateDefaultConfig => confirm_config_repair(
            state::config::ConfigRepair::BackupAndCreateDefault,
            window,
            cx,
        ),
        CriticalResourceAction::BackupAndOverwritePendingConfig => confirm_config_repair(
            state::config::ConfigRepair::BackupAndOverwritePending,
            window,
            cx,
        ),
        CriticalResourceAction::RefreshDatabase => database::request_refresh(cx),
        CriticalResourceAction::BackupAndCreateFreshDatabase => confirm_database_repair(window, cx),
        CriticalResourceAction::RetrySession => app::session::request_retry(cx),
    }
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
    let task = window.spawn(cx, async move |cx| {
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
        if let Err(error) = window_handle.update(cx, |_, _window, cx| {
            if let Err(error) = database::backup_and_create_fresh(backup_dir.clone(), cx) {
                tracing::error!(?error, "repair database failed");
            }
        }) {
            tracing::error!(?error, "complete database repair failed");
        }
    });
    app::tasks::retain_window(window, task, cx);
}
