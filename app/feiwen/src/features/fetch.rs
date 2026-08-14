use crate::{
    fetch::{FetchErrorKind, FetchPageError},
    foundation::{I18n, IconName},
    store::database,
};
use async_compat::Compat;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputContentType, InputState},
    label::Label,
    progress::Progress,
    table::{Column, DataTable, TableDelegate, TableState},
    v_flex,
};
use gpui_form::Form;
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInput, IntegerInputState};
use gpui_operation::Transition;
use gpui_store::Store;
use tracing::{Instrument, Level, event};

mod form;
mod run;
mod runner;

pub(crate) use form::FetchRequest;
pub(crate) use run::{FetchPageLog, FetchProgress, FetchRun};

use form::{FetchDraft, FetchValidator};
use run::{
    FetchFailure, FetchMessage, FetchPageLogStatus, FetchStatus, resume_page_after_interrupt,
    retry_page_after_failure,
};
use runner::Runner;

const LOG_PAGE_COLUMN: f32 = 72.;
const LOG_STATUS_COLUMN: f32 = 120.;
const LOG_INSERTED_COLUMN: f32 = 96.;
const LOG_ELAPSED_COLUMN: f32 = 96.;
const LOG_DETAIL_COLUMN: f32 = 520.;

pub(crate) struct FetchView {
    task_state: Store<FetchRun>,
    form: Entity<Form<FetchDraft>>,
    log_table: Entity<TableState<FetchLogTableDelegate>>,
    url_input: Entity<InputState>,
    start_page: Entity<IntegerInputState<u32>>,
    end_page: Entity<IntegerInputState<u32>>,
    cookie_input: Entity<InputState>,
    _form_controls: (
        FormInput,
        FormIntegerInput<u32>,
        FormIntegerInput<u32>,
        FormInput,
    ),
    _subscriptions: Vec<Subscription>,
}

impl FetchView {
    pub(crate) fn new(
        window: &mut Window,
        task_state: Store<FetchRun>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (url_placeholder, start_page_placeholder, end_page_placeholder, cookie_placeholder) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-url-placeholder"),
                i18n.t("fetch-start-page-placeholder"),
                i18n.t("fetch-end-page-placeholder"),
                i18n.t("fetch-cookie-placeholder"),
            )
        };
        let form = cx.new(|_| Form::new(FetchDraft::default()).with_validator(FetchValidator));
        let url_control = FormInput::new(
            &form,
            FetchDraft::URL,
            move |window, cx| InputState::new(window, cx).placeholder(url_placeholder),
            window,
            cx,
        );
        let start_control = FormIntegerInput::new(
            &form,
            FetchDraft::START_PAGE,
            move |window, cx| IntegerInputState::new(window, cx).min(1).step(1),
            window,
            cx,
        )
        .expect("bind fetch start page");
        let start_editor = start_control.read(cx).editor().clone();
        start_editor.update(cx, |input, cx| {
            input.set_placeholder(start_page_placeholder, window, cx);
        });
        let end_control = FormIntegerInput::new(
            &form,
            FetchDraft::END_PAGE,
            move |window, cx| IntegerInputState::new(window, cx).min(1).step(1),
            window,
            cx,
        )
        .expect("bind fetch end page");
        let end_editor = end_control.read(cx).editor().clone();
        end_editor.update(cx, |input, cx| {
            input.set_placeholder(end_page_placeholder, window, cx);
        });
        let cookie_control = FormInput::new(
            &form,
            FetchDraft::COOKIE,
            move |window, cx| InputState::new(window, cx).placeholder(cookie_placeholder),
            window,
            cx,
        );
        let url_input = (*url_control).clone();
        let start_page = (*start_control).clone();
        let end_page = (*end_control).clone();
        let cookie_input = (*cookie_control).clone();
        let log_table = cx.new(|cx| {
            TableState::new(
                FetchLogTableDelegate {
                    task_state: task_state.clone(),
                },
                window,
                cx,
            )
        });

        let _subscriptions = vec![
            task_state.observe(cx, |view, _, cx| {
                view.log_table.update(cx, |table, cx| {
                    table.refresh(cx);
                    cx.notify();
                });
                cx.notify();
            }),
            cx.observe(&form, |_, _, cx| cx.notify()),
        ];

        Self {
            task_state,
            form,
            log_table,
            url_input,
            start_page,
            end_page,
            cookie_input,
            _form_controls: (url_control, start_control, end_control, cookie_control),
            _subscriptions,
        }
    }

    fn start_fetch(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::Fresh, cx);
    }

    pub(crate) fn request_start_fetch(&mut self, cx: &mut Context<Self>) {
        self.start_fetch(cx);
    }

    pub(crate) fn is_running(&self, cx: &App) -> bool {
        self.task_state.read(cx, FetchRun::is_running)
    }

    pub(crate) fn can_start(&self, cx: &App) -> bool {
        let draft = FetchDraft::URL.get(&self.form, cx);
        let start_page = FetchDraft::START_PAGE.get(&self.form, cx);
        let end_page = FetchDraft::END_PAGE.get(&self.form, cx);
        !self.is_running(cx)
            && database::is_ready(cx)
            && !draft.trim().is_empty()
            && start_page <= end_page
    }

    pub(crate) fn titlebar_summary(&self, i18n: &I18n, cx: &App) -> String {
        self.task_state
            .read(cx, |state| state.titlebar_summary(i18n))
    }

    fn resume_fetch(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::ResumeInterrupted, cx);
    }

    fn retry_failed_page(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::RetryFailed, cx);
    }

    fn interrupt_fetch(&mut self, cx: &mut Context<Self>) {
        self.task_state
            .update(cx, |state| state.transition(FetchMessage::Interrupt));
    }

    fn load_snapshot_into_form(&mut self, cx: &mut Context<Self>) {
        let snapshot = self
            .task_state
            .read(cx, |state| state.snapshot().map(FetchDraft::from));
        if let Some(snapshot) = snapshot {
            self.form.update(cx, |form, cx| form.replace(snapshot, cx));
        }
    }

    fn start_fetch_from(&mut self, mode: RunMode, cx: &mut Context<Self>) {
        event!(Level::INFO, mode = mode.label(), "starting fetch request");
        if self.task_state.read(cx, FetchRun::is_running) {
            event!(
                Level::INFO,
                mode = mode.label(),
                "ignored fetch request while running"
            );
            return;
        }
        if !database::is_ready(cx) {
            event!(
                Level::INFO,
                mode = mode.label(),
                "ignored fetch request while database is unavailable"
            );
            return;
        }
        let fresh_request = if matches!(mode, RunMode::Fresh) {
            let Ok(prepared) = self.form.update(cx, |form, cx| form.prepare(cx)) else {
                return;
            };
            Some(FetchRequest::from(prepared.into_parts().1))
        } else {
            None
        };
        let conn = match database::ready_pool(cx).and_then(|pool| {
            pool.get()
                .map_err(super::super::store::database::DatabaseProblem::new)
        }) {
            Ok(conn) => conn,
            Err(err) => {
                event!(
                    Level::ERROR,
                    mode = mode.label(),
                    error = %err,
                    "failed to get database pool for fetch"
                );
                self.task_state.update(cx, |state| {
                    state.transition(FetchMessage::Rejected {
                        request: fresh_request.clone(),
                        error: FetchPageError {
                            page: fresh_request
                                .as_ref()
                                .or(state.snapshot())
                                .map_or(1, |request| request.start_page),
                            kind: FetchErrorKind::Database,
                            message: err.to_string(),
                        },
                    });
                });
                return;
            }
        };

        let (request, clear_logs) = {
            let state = self.task_state.read(cx, |state| {
                (
                    state.snapshot().cloned(),
                    state.last_success_page(),
                    state.failed_page(),
                )
            });
            let start_page = match mode {
                RunMode::Fresh => fresh_request.as_ref().unwrap().start_page,
                RunMode::ResumeInterrupted => {
                    let snapshot = state.0.as_ref().expect("interrupted run has snapshot");
                    match resume_page_after_interrupt(
                        state.1,
                        snapshot.start_page,
                        snapshot.end_page,
                    ) {
                        Some(page) => page,
                        None => {
                            event!(
                                Level::INFO,
                                mode = mode.label(),
                                "no interrupted fetch page to resume"
                            );
                            return;
                        }
                    }
                }
                RunMode::RetryFailed => {
                    let snapshot = state.0.as_ref().expect("failed run has snapshot");
                    match retry_page_after_failure(state.2, snapshot.start_page, snapshot.end_page)
                    {
                        Some(page) => page,
                        None => {
                            event!(
                                Level::INFO,
                                mode = mode.label(),
                                "no failed fetch page to retry"
                            );
                            return;
                        }
                    }
                }
            };
            let request = fresh_request.clone().unwrap_or_else(|| {
                let snapshot = state.0.as_ref().unwrap();
                FetchRequest {
                    url: snapshot.url.clone(),
                    start_page,
                    end_page: snapshot.end_page,
                    cookie: snapshot.cookie.clone(),
                }
            });
            (request, matches!(mode, RunMode::Fresh))
        };
        event!(
            Level::INFO,
            mode = mode.label(),
            start_page = request.start_page,
            end_page = request.end_page,
            has_cookie = !request.cookie.is_empty(),
            clear_logs,
            "fetch task scheduled"
        );

        let runner_request = request.clone();
        let task = cx.spawn(async move |owner, cx| {
            let span = tracing::info_span!(
                "feiwen_fetch",
                start_page = runner_request.start_page,
                end_page = runner_request.end_page,
                has_cookie = !runner_request.cookie.is_empty()
            );
            let mut runner = Runner::new(runner_request, owner, conn, cx);
            Compat::new(async move { runner.run().await })
                .instrument(span)
                .await;
        });
        self.task_state.update(cx, |state| {
            state.transition(FetchMessage::Start {
                request,
                clear_logs,
                task,
            });
        });
    }
}

#[derive(Clone, Copy)]
enum RunMode {
    Fresh,
    ResumeInterrupted,
    RetryFailed,
}

impl RunMode {
    fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::ResumeInterrupted => "resume_interrupted",
            Self::RetryFailed => "retry_failed",
        }
    }
}

impl Render for FetchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_running = self.task_state.read(cx, FetchRun::is_running);
        div()
            .h_full()
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(cx.theme().tokens.background.background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_3()
                    .gap_3()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(self.render_form_panel(is_running, cx))
                            .child(self.render_status_panel(cx)),
                    )
                    .child(self.render_logs_panel(cx)),
            )
    }
}

impl FetchView {
    fn render_form_panel(&self, _is_running: bool, cx: &mut Context<Self>) -> Div {
        let (
            section_config,
            field_url,
            field_start_page,
            field_end_page,
            field_cookie,
            cookie_hidden,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-section-config"),
                i18n.t("fetch-field-url"),
                i18n.t("fetch-field-start-page"),
                i18n.t("fetch-field-end-page"),
                i18n.t("fetch-field-cookie"),
                i18n.t("fetch-cookie-hidden"),
            )
        };
        let field_color = cx.theme().foreground;
        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.background.background)
            .rounded_lg()
            .child(section_title(IconName::Settings, section_config, cx))
            .child(field_label(IconName::Link, field_url, field_color, cx))
            .child(Input::new(&self.url_input).content_type(InputContentType::Url))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(field_label(
                                IconName::FileText,
                                field_start_page,
                                field_color,
                                cx,
                            ))
                            .child(IntegerInput::new(&self.start_page)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(field_label(
                                IconName::FileText,
                                field_end_page,
                                field_color,
                                cx,
                            ))
                            .child(IntegerInput::new(&self.end_page)),
                    ),
            )
            .child(field_label(IconName::Cookie, field_cookie, field_color, cx))
            .child(Input::new(&self.cookie_input))
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::EyeOff)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(cookie_hidden)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
    }

    fn render_status_panel(&self, cx: &mut Context<Self>) -> Div {
        let section_status = cx.global::<I18n>().t("fetch-section-status");
        let (status, snapshot) = self
            .task_state
            .read(cx, |state| (state.status(), state.snapshot().cloned()));
        let status_body = match &status {
            FetchStatus::Idle => self.render_idle_status(cx),
            FetchStatus::Running(progress) => self.render_progress_status(progress, cx),
            FetchStatus::Interrupted(progress) => self.render_interrupted_status(progress, cx),
            FetchStatus::Failed(failure) => self.render_failed_status(failure, cx),
            FetchStatus::Success(progress) => self.render_success_status(progress, cx),
        };

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.background.background)
            .rounded_lg()
            .child(section_title(IconName::Info, section_status, cx))
            .when_some(snapshot, |this, snapshot| {
                this.child(self.render_snapshot(&snapshot, cx))
            })
            .child(status_body)
    }

    fn render_snapshot(&self, snapshot: &FetchRequest, cx: &mut Context<Self>) -> Div {
        let i18n = cx.global::<I18n>();
        v_flex()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .child(
                Label::new(i18n.t("fetch-snapshot-title"))
                    .text_sm()
                    .font_semibold(),
            )
            .child(
                Label::new(format!("{}: {}", i18n.t("fetch-field-url"), snapshot.url))
                    .text_xs()
                    .truncate(),
            )
            .child(
                Label::new(format!(
                    "{}: {}–{}",
                    i18n.t("fetch-snapshot-pages"),
                    snapshot.start_page,
                    snapshot.end_page
                ))
                .text_xs(),
            )
            .child(
                Label::new(format!(
                    "{}: {}",
                    i18n.t("fetch-field-cookie"),
                    if snapshot.cookie.is_empty() {
                        i18n.t("fetch-snapshot-cookie-unset")
                    } else {
                        i18n.t("fetch-snapshot-cookie-set")
                    }
                ))
                .text_xs(),
            )
    }

    fn render_idle_status(&self, cx: &mut Context<Self>) -> Div {
        let (title, desc) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-state-idle-title"),
                i18n.t("fetch-state-idle-desc"),
            )
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(status_title(
                IconName::Info,
                title,
                cx.theme().muted_foreground,
            ))
            .child(
                Label::new(desc)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }

    fn render_progress_status(&self, progress: &FetchProgress, cx: &mut Context<Self>) -> Div {
        let (title, interrupt_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-state-running-title"),
                i18n.t("fetch-action-interrupt"),
            )
        };
        status_layout(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(status_title(
                    IconName::LoaderCircle,
                    title,
                    cx.theme().primary,
                ))
                .child(
                    Progress::new("fetch-progress")
                        .value(progress_percent(progress))
                        .with_size(px(8.)),
                )
                .child(metrics_grid(progress, cx)),
            action_panel(
                IconName::CircleStop,
                Button::new("fetch-interrupt")
                    .danger()
                    .icon(IconName::CircleStop)
                    .label(interrupt_label)
                    .on_click(cx.listener(|this, _, _, cx| this.interrupt_fetch(cx))),
                cx,
            ),
        )
    }

    fn render_interrupted_status(&self, progress: &FetchProgress, cx: &mut Context<Self>) -> Div {
        let (title, next_page_label, resume_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-state-interrupted-title"),
                i18n.t("fetch-stat-next-page"),
                i18n.t("fetch-action-resume-interrupted"),
            )
        };
        status_layout(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(status_title(
                    IconName::CirclePause,
                    title,
                    cx.theme().warning,
                ))
                .child(metrics_grid(progress, cx))
                .child(
                    Label::new(format!(
                        "{} {}",
                        next_page_label,
                        resume_page_after_interrupt(
                            progress.last_success_page,
                            progress.start_page,
                            progress.end_page
                        )
                        .unwrap_or(progress.end_page)
                    ))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
                ),
            action_panel(
                IconName::CirclePlay,
                v_flex()
                    .gap_2()
                    .child(
                        Button::new("fetch-resume")
                            .warning()
                            .icon(IconName::CirclePlay)
                            .label(resume_label)
                            .on_click(cx.listener(|this, _, _, cx| this.resume_fetch(cx))),
                    )
                    .child(
                        Button::new("fetch-load-snapshot")
                            .label(cx.global::<I18n>().t("fetch-action-load-snapshot"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.load_snapshot_into_form(cx)),
                            ),
                    ),
                cx,
            ),
        )
    }

    fn render_failed_status(&self, failure: &FetchFailure, cx: &mut Context<Self>) -> Div {
        let (
            title,
            failed_page_label,
            error_kind_title,
            error_kind_label,
            error_detail_label,
            stop_note,
            retry_label,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-state-failed-title"),
                i18n.t("fetch-stat-failed-page"),
                i18n.t("fetch-stat-error-kind"),
                error_kind_label(failure.kind, i18n),
                i18n.t("fetch-stat-error-detail"),
                i18n.t("fetch-failed-stop-note"),
                i18n.t("fetch-action-retry-failed"),
            )
        };
        status_layout(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(status_title(IconName::OctagonX, title, cx.theme().danger))
                .child(metrics_grid(&failure.progress, cx))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(metric_card(
                            failed_page_label,
                            failure.page.to_string(),
                            cx.theme().danger,
                            cx,
                        ))
                        .child(metric_card(
                            error_kind_title,
                            error_kind_label,
                            cx.theme().danger,
                            cx,
                        ))
                        .child(metric_card(
                            error_detail_label,
                            failure.message.clone(),
                            cx.theme().danger,
                            cx,
                        )),
                )
                .child(
                    Label::new(stop_note)
                        .text_sm()
                        .text_color(cx.theme().danger),
                ),
            action_panel(
                IconName::RotateCcw,
                v_flex()
                    .gap_2()
                    .child(
                        Button::new("fetch-retry-failed")
                            .danger()
                            .icon(IconName::RotateCcw)
                            .label(retry_label)
                            .on_click(cx.listener(|this, _, _, cx| this.retry_failed_page(cx))),
                    )
                    .child(
                        Button::new("fetch-load-snapshot")
                            .label(cx.global::<I18n>().t("fetch-action-load-snapshot"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.load_snapshot_into_form(cx)),
                            ),
                    ),
                cx,
            ),
        )
    }

    fn render_success_status(&self, progress: &FetchProgress, cx: &mut Context<Self>) -> Div {
        let title = cx.global::<I18n>().t("fetch-state-success");
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(status_title(
                IconName::CircleCheck,
                title,
                cx.theme().success,
            ))
            .child(metrics_grid(progress, cx))
    }

    fn render_logs_panel(&self, cx: &mut Context<Self>) -> Div {
        let section_logs = cx.global::<I18n>().t("fetch-section-page-logs");
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.background.background)
            .rounded_lg()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(section_title(IconName::List, section_logs, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(DataTable::new(&self.log_table).small().stripe(true)),
            )
    }
}

fn section_title(icon: IconName, label: String, cx: &mut Context<FetchView>) -> Div {
    h_flex()
        .gap_2()
        .child(Icon::new(icon).size_4().text_color(cx.theme().foreground))
        .child(Label::new(label).font_medium())
}

fn field_label(icon: IconName, label: String, color: Hsla, cx: &mut Context<FetchView>) -> Div {
    h_flex()
        .gap_1()
        .child(
            Icon::new(icon)
                .size_3()
                .text_color(cx.theme().muted_foreground),
        )
        .child(Label::new(label).text_sm().font_medium().text_color(color))
}

fn status_layout(details: Div, actions: Div) -> Div {
    div()
        .flex()
        .gap_4()
        .items_stretch()
        .child(details.flex_1().min_w_0())
        .child(actions)
}

fn action_panel(icon: IconName, action: impl IntoElement, cx: &mut Context<FetchView>) -> Div {
    div()
        .w(px(240.))
        .flex_none()
        .flex()
        .flex_col()
        .gap_3()
        .border_l_1()
        .border_color(cx.theme().border)
        .pl_4()
        .child(
            h_flex()
                .gap_2()
                .child(
                    Icon::new(icon)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    Label::new(cx.global::<I18n>().t("fetch-section-actions"))
                        .text_sm()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .child(action)
}

fn status_title(icon: IconName, label: String, color: Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(Icon::new(icon).size_4().text_color(color))
        .child(Label::new(label).text_lg().font_medium().text_color(color))
}

fn progress_percent(progress: &FetchProgress) -> f32 {
    let page_count = progress.page_count().max(1) as f32;
    let completed = progress.completed_pages().min(progress.page_count()) as f32;
    (completed / page_count * 100.0).clamp(0.0, 100.0)
}

fn metrics_grid(progress: &FetchProgress, cx: &mut Context<FetchView>) -> Div {
    let (current_page_label, completed_pages_label, total_label) = {
        let i18n = cx.global::<I18n>();
        (
            i18n.t("fetch-stat-current-page"),
            i18n.t("fetch-stat-completed-pages"),
            i18n.t("fetch-stat-total"),
        )
    };
    div()
        .flex()
        .gap_3()
        .child(metric_card(
            current_page_label,
            format!("{} / {}", progress.current_page, progress.end_page),
            cx.theme().primary,
            cx,
        ))
        .child(metric_card(
            completed_pages_label,
            progress.completed_pages().to_string(),
            cx.theme().success,
            cx,
        ))
        .child(metric_card(
            total_label,
            progress.total.to_string(),
            cx.theme().foreground,
            cx,
        ))
}

fn metric_card(label: String, value: String, color: Hsla, cx: &mut Context<FetchView>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .p_2()
        .child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(Label::new(value).text_sm().font_medium().text_color(color))
}

struct FetchLogTableDelegate {
    task_state: Store<FetchRun>,
}

impl FetchLogTableDelegate {
    fn log_at(&self, row_ix: usize, cx: &App) -> Option<FetchPageLog> {
        self.task_state.read(cx, |state| {
            state
                .logs()
                .len()
                .checked_sub(row_ix + 1)
                .and_then(|ix| state.logs().get(ix))
                .cloned()
        })
    }

    fn status_cell(log: &FetchPageLog, cx: &mut Context<TableState<Self>>) -> Div {
        let (label, color, icon) = match log.status {
            FetchPageLogStatus::Running => (
                cx.global::<I18n>().t("fetch-log-status-running"),
                cx.theme().primary,
                IconName::LoaderCircle,
            ),
            FetchPageLogStatus::Success => (
                cx.global::<I18n>().t("fetch-log-status-success"),
                cx.theme().success,
                IconName::CircleCheck,
            ),
            FetchPageLogStatus::Failed => (
                cx.global::<I18n>().t("fetch-log-status-failed"),
                cx.theme().danger,
                IconName::OctagonX,
            ),
        };

        h_flex()
            .gap_1()
            .rounded_full()
            .px_2()
            .py_1()
            .bg(color.opacity(0.10))
            .child(Icon::new(icon).size_3().text_color(color))
            .child(Label::new(label).text_xs().text_color(color))
    }
}

impl TableDelegate for FetchLogTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        5
    }

    fn rows_count(&self, cx: &App) -> usize {
        self.task_state.read(cx, |state| state.logs().len())
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        let i18n = cx.global::<I18n>();
        match col_ix {
            0 => Column::new("page", i18n.t("fetch-log-column-page"))
                .width(px(LOG_PAGE_COLUMN))
                .fixed_left()
                .resizable(false),
            1 => Column::new("status", i18n.t("fetch-log-column-status"))
                .width(px(LOG_STATUS_COLUMN))
                .resizable(false),
            2 => Column::new("inserted", i18n.t("fetch-log-column-inserted"))
                .width(px(LOG_INSERTED_COLUMN))
                .resizable(false),
            3 => Column::new("elapsed", i18n.t("fetch-log-column-elapsed"))
                .width(px(LOG_ELAPSED_COLUMN))
                .resizable(false),
            _ => Column::new("detail", i18n.t("fetch-log-column-detail"))
                .width(px(LOG_DETAIL_COLUMN))
                .resizable(true),
        }
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let mut row = div().id(("fetch-log-row", row_ix));
        if self
            .log_at(row_ix, cx)
            .is_some_and(|log| log.status == FetchPageLogStatus::Failed)
        {
            row = row.bg(cx.theme().tokens.danger.background.opacity(0.06));
        }
        row
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(log) = self.log_at(row_ix, cx) else {
            return Label::new("").into_any_element();
        };

        match col_ix {
            0 => Label::new(log.page.to_string())
                .text_sm()
                .into_any_element(),
            1 => Self::status_cell(&log, cx).into_any_element(),
            2 => Label::new(
                log.inserted
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            )
            .text_sm()
            .into_any_element(),
            3 => Label::new(
                log.elapsed_ms
                    .map(format_elapsed)
                    .unwrap_or_else(|| "-".to_string()),
            )
            .text_sm()
            .into_any_element(),
            _ => {
                let message = log_message(&log, cx.global::<I18n>());
                Label::new(message)
                    .text_sm()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .into_any_element()
            }
        }
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, cx: &App) -> String {
        let Some(log) = self.log_at(row_ix, cx) else {
            return String::new();
        };

        match col_ix {
            0 => log.page.to_string(),
            1 => match log.status {
                FetchPageLogStatus::Running => cx.global::<I18n>().t("fetch-log-status-running"),
                FetchPageLogStatus::Success => cx.global::<I18n>().t("fetch-log-status-success"),
                FetchPageLogStatus::Failed => cx.global::<I18n>().t("fetch-log-status-failed"),
            },
            2 => log
                .inserted
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string()),
            3 => log
                .elapsed_ms
                .map(format_elapsed)
                .unwrap_or_else(|| "-".to_string()),
            _ => log_message(&log, cx.global::<I18n>()),
        }
    }
}

fn log_message(log: &FetchPageLog, i18n: &I18n) -> String {
    match log.status {
        FetchPageLogStatus::Running => i18n.t("fetch-log-message-running"),
        FetchPageLogStatus::Success => format!(
            "{} {}",
            i18n.t("fetch-log-message-success"),
            log.inserted.unwrap_or(0)
        ),
        FetchPageLogStatus::Failed => log.message.clone(),
    }
}

fn format_elapsed(elapsed_ms: u128) -> String {
    if elapsed_ms >= 1000 {
        format!("{:.2}s", elapsed_ms as f64 / 1000.0)
    } else {
        format!("{elapsed_ms}ms")
    }
}

fn error_kind_label(kind: FetchErrorKind, i18n: &I18n) -> String {
    match kind {
        FetchErrorKind::Database => i18n.t("fetch-error-kind-database"),
        FetchErrorKind::Network => i18n.t("fetch-error-kind-network"),
        FetchErrorKind::Parse => i18n.t("fetch-error-kind-parse"),
        FetchErrorKind::Other => i18n.t("fetch-error-kind-other"),
    }
}

#[cfg(test)]
mod tests {
    use super::{FetchProgress, progress_percent};

    #[test]
    fn progress_percent_clamps_to_component_range() {
        let progress = |last_success_page| FetchProgress {
            start_page: 3,
            end_page: 6,
            current_page: 3,
            last_success_page,
            total: 0,
        };

        assert_eq!(progress_percent(&progress(None)), 0.0);
        assert_eq!(progress_percent(&progress(Some(4))), 50.0);
        assert_eq!(progress_percent(&progress(Some(10))), 100.0);
    }
}
