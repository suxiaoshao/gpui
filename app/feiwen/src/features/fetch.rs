use super::{
    Workspace,
    workspace::{RouterType, WorkspaceEvent},
};
use crate::{
    fetch::{self, FetchErrorKind, FetchPageError},
    foundation::{I18n, IconName},
    store::{Db, service::Novel},
};
use async_compat::Compat;
use diesel::{
    SqliteConnection,
    r2d2::{ConnectionManager, PooledConnection},
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    table::{Column, DataTable, TableDelegate, TableState},
};
use regex::Regex;
use reqwest::Client;
use std::time::Instant;
use tracing::{Instrument, Level, event};

const MAX_PAGE_LOGS: usize = 80;
const LOG_PAGE_COLUMN: f32 = 72.;
const LOG_STATUS_COLUMN: f32 = 120.;
const LOG_INSERTED_COLUMN: f32 = 96.;
const LOG_ELAPSED_COLUMN: f32 = 96.;
const LOG_DETAIL_COLUMN: f32 = 520.;

#[derive(Clone)]
struct FetchRequest {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FetchPageLogStatus {
    Running,
    Success,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FetchPageLog {
    page: u32,
    status: FetchPageLogStatus,
    inserted: Option<usize>,
    elapsed_ms: Option<u128>,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FetchProgress {
    start_page: u32,
    end_page: u32,
    current_page: u32,
    last_success_page: Option<u32>,
    total: i64,
}

impl FetchProgress {
    fn completed_pages(&self) -> u32 {
        self.last_success_page
            .filter(|page| *page >= self.start_page)
            .map(|page| page - self.start_page + 1)
            .unwrap_or(0)
    }

    fn page_count(&self) -> u32 {
        page_count(self.start_page, self.end_page)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FetchFailure {
    progress: FetchProgress,
    page: u32,
    kind: FetchErrorKind,
    message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum FetchStatus {
    #[default]
    Idle,
    Running(FetchProgress),
    Interrupted(FetchProgress),
    Failed(FetchFailure),
    Success(FetchProgress),
}

pub(crate) struct FetchTaskState {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
    status: FetchStatus,
    logs: Vec<FetchPageLog>,
    task: Option<Task<()>>,
}

impl Default for FetchTaskState {
    fn default() -> Self {
        Self {
            url: String::new(),
            start_page: 1,
            end_page: 1,
            cookie: String::new(),
            status: FetchStatus::Idle,
            logs: Vec::new(),
            task: None,
        }
    }
}

impl FetchTaskState {
    fn set_url(&mut self, url: String) {
        self.url = url;
    }

    fn set_start_page(&mut self, start_page: u32) {
        self.start_page = start_page.max(1);
    }

    fn set_end_page(&mut self, end_page: u32) {
        self.end_page = end_page.max(1);
    }

    fn set_cookie(&mut self, cookie: String) {
        self.cookie = cookie;
    }

    fn is_running(&self) -> bool {
        matches!(self.status, FetchStatus::Running(_))
    }

    pub(crate) fn has_visible_summary(&self) -> bool {
        !matches!(self.status, FetchStatus::Idle | FetchStatus::Success(_))
    }

    pub(crate) fn summary_text(&self, i18n: &I18n) -> Option<String> {
        match &self.status {
            FetchStatus::Idle => None,
            FetchStatus::Running(progress) => Some(format!(
                "{} {} / {} · {} {} · {} {}",
                i18n.t("fetch-state-running-title"),
                progress.current_page,
                progress.end_page,
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages(),
                i18n.t("fetch-stat-total"),
                progress.total
            )),
            FetchStatus::Interrupted(progress) => Some(format!(
                "{} · {} {} · {} {}",
                i18n.t("fetch-state-interrupted-title"),
                i18n.t("fetch-stat-next-page"),
                resume_page_after_interrupt(
                    progress.last_success_page,
                    progress.start_page,
                    progress.end_page
                )
                .unwrap_or(progress.end_page),
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages()
            )),
            FetchStatus::Failed(failure) => Some(format!(
                "{} · {} {} · {}",
                i18n.t("fetch-state-failed-title"),
                i18n.t("fetch-stat-failed-page"),
                failure.page,
                failure.message
            )),
            FetchStatus::Success(progress) => Some(format!(
                "{} · {} {} · {} {}",
                i18n.t("fetch-state-success"),
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages(),
                i18n.t("fetch-stat-total"),
                progress.total
            )),
        }
    }

    fn request_from(&self, start_page: u32) -> FetchRequest {
        FetchRequest {
            url: self.url.clone(),
            start_page,
            end_page: self.end_page,
            cookie: self.cookie.clone(),
        }
    }

    fn begin_run(&mut self, run_start_page: u32, total: i64, clear_logs: bool) {
        let last_success_page = if clear_logs {
            None
        } else {
            self.last_success_page()
        };
        if clear_logs {
            self.logs.clear();
        }
        self.status = FetchStatus::Running(FetchProgress {
            start_page: self.start_page,
            end_page: self.end_page,
            current_page: run_start_page,
            last_success_page,
            total,
        });
    }

    fn interrupt(&mut self) {
        self.task = None;
        if let FetchStatus::Running(progress) = self.status {
            self.status = FetchStatus::Interrupted(progress);
        }
    }

    fn mark_page_started(&mut self, page: u32) {
        if let FetchStatus::Running(progress) = &mut self.status {
            progress.current_page = page;
        }
        self.upsert_log(FetchPageLog {
            page,
            status: FetchPageLogStatus::Running,
            inserted: None,
            elapsed_ms: None,
            message: "fetching".to_string(),
        });
    }

    fn mark_page_succeeded(&mut self, page: u32, inserted: usize, total: i64, elapsed_ms: u128) {
        if let FetchStatus::Running(progress) = &mut self.status {
            progress.current_page = page;
            progress.last_success_page = Some(page);
            progress.total = total;
        }
        self.upsert_log(FetchPageLog {
            page,
            status: FetchPageLogStatus::Success,
            inserted: Some(inserted),
            elapsed_ms: Some(elapsed_ms),
            message: "success".to_string(),
        });
    }

    fn mark_failed(&mut self, error: FetchPageError, elapsed_ms: Option<u128>) {
        let progress = match &self.status {
            FetchStatus::Running(progress) => *progress,
            FetchStatus::Interrupted(progress) => *progress,
            FetchStatus::Failed(failure) => failure.progress,
            FetchStatus::Success(progress) => *progress,
            FetchStatus::Idle => FetchProgress {
                start_page: self.start_page,
                end_page: self.end_page,
                current_page: error.page,
                last_success_page: None,
                total: 0,
            },
        };
        self.task = None;
        self.status = FetchStatus::Failed(FetchFailure {
            progress,
            page: error.page,
            kind: error.kind,
            message: error.message.clone(),
        });
        self.upsert_log(FetchPageLog {
            page: error.page,
            status: FetchPageLogStatus::Failed,
            inserted: None,
            elapsed_ms,
            message: error.message,
        });
    }

    fn mark_succeeded(&mut self) {
        self.task = None;
        if let FetchStatus::Running(progress) = self.status {
            self.status = FetchStatus::Success(progress);
        }
    }

    fn upsert_log(&mut self, log: FetchPageLog) {
        if let Some(existing) = self
            .logs
            .iter_mut()
            .find(|existing| existing.page == log.page)
        {
            *existing = log;
        } else {
            self.logs.push(log);
        }
        if self.logs.len() > MAX_PAGE_LOGS {
            let overflow = self.logs.len() - MAX_PAGE_LOGS;
            self.logs.drain(0..overflow);
        }
    }

    fn last_success_page(&self) -> Option<u32> {
        match &self.status {
            FetchStatus::Running(progress)
            | FetchStatus::Interrupted(progress)
            | FetchStatus::Success(progress) => progress.last_success_page,
            FetchStatus::Failed(failure) => failure.progress.last_success_page,
            FetchStatus::Idle => None,
        }
    }

    fn failed_page(&self) -> Option<u32> {
        match &self.status {
            FetchStatus::Failed(failure) => Some(failure.page),
            _ => None,
        }
    }
}

struct Runner<'a> {
    request: FetchRequest,
    task_state: WeakEntity<FetchTaskState>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
    cx: &'a mut AsyncApp,
}

impl Runner<'_> {
    async fn run(&mut self) {
        let mut total = match Novel::count(&mut self.conn) {
            Ok(total) => total,
            Err(err) => {
                self.mark_failed(FetchPageError::new(self.request.start_page, err), None);
                return;
            }
        };

        let start_page = self.request.start_page;
        self.update_state(|state| state.begin_run(start_page, total, false));

        let client = Client::new();
        for page in self.request.start_page..=self.request.end_page {
            self.update_state(|state| state.mark_page_started(page));
            let started_at = Instant::now();
            let novels =
                match fetch::fetch_page(&self.request.url, page, &self.request.cookie, &client)
                    .await
                {
                    Ok(novels) => novels,
                    Err(err) => {
                        self.mark_failed(
                            FetchPageError::new(page, err),
                            Some(started_at.elapsed().as_millis()),
                        );
                        return;
                    }
                };

            let inserted = novels.len();
            for novel in novels {
                if let Err(err) = novel.save(&mut self.conn) {
                    self.mark_failed(
                        FetchPageError::new(page, err),
                        Some(started_at.elapsed().as_millis()),
                    );
                    return;
                }
            }

            match Novel::count(&mut self.conn) {
                Ok(next_total) => total = next_total,
                Err(err) => {
                    self.mark_failed(
                        FetchPageError::new(page, err),
                        Some(started_at.elapsed().as_millis()),
                    );
                    return;
                }
            }
            self.update_state(|state| {
                state.mark_page_succeeded(page, inserted, total, started_at.elapsed().as_millis())
            });
        }

        self.update_state(FetchTaskState::mark_succeeded);
    }

    fn mark_failed(&mut self, error: FetchPageError, elapsed_ms: Option<u128>) {
        event!(
            Level::ERROR,
            page = error.page,
            kind = %error.kind,
            message = %error.message,
            "Failed to fetch page"
        );
        self.update_state(|state| state.mark_failed(error, elapsed_ms));
    }

    fn update_state(&mut self, update: impl FnOnce(&mut FetchTaskState)) {
        if let Err(err) = self.task_state.update(self.cx, |state, cx| {
            update(state);
            cx.notify();
        }) {
            event!(Level::ERROR, "Failed to update fetch task state: {:?}", err);
        }
    }
}

pub(crate) struct FetchView {
    workspace: Entity<Workspace>,
    task_state: Entity<FetchTaskState>,
    log_table: Entity<TableState<FetchLogTableDelegate>>,
    url_input: Entity<InputState>,
    start_page: Entity<InputState>,
    end_page: Entity<InputState>,
    cookie_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl FetchView {
    pub(crate) fn new(
        window: &mut Window,
        workspace: Entity<Workspace>,
        task_state: Entity<FetchTaskState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let integer_regex = Regex::new(r"^\d+$").unwrap();
        let (url_placeholder, start_page_placeholder, end_page_placeholder, cookie_placeholder) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-url-placeholder"),
                i18n.t("fetch-start-page-placeholder"),
                i18n.t("fetch-end-page-placeholder"),
                i18n.t("fetch-cookie-placeholder"),
            )
        };
        let mut _subscriptions = vec![];
        let url_input = cx.new(|cx| InputState::new(window, cx).placeholder(url_placeholder));
        let start_page = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(start_page_placeholder)
                .pattern(integer_regex.clone())
        });
        let end_page = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(end_page_placeholder)
                .pattern(integer_regex)
        });
        let cookie_input = cx.new(|cx| InputState::new(window, cx).placeholder(cookie_placeholder));
        let log_table = cx.new(|cx| {
            TableState::new(
                FetchLogTableDelegate {
                    task_state: task_state.clone(),
                },
                window,
                cx,
            )
        });

        _subscriptions.push(cx.subscribe_in(
            &url_input,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value().to_string();
                    view.task_state.update(cx, |task, _| task.set_url(text));
                }
                InputEvent::PressEnter { .. } => {
                    view.start_page.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &start_page,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    let page = text.parse().unwrap_or(1);
                    view.task_state
                        .update(cx, |task, _| task.set_start_page(page));
                }
                InputEvent::PressEnter { .. } => {
                    view.end_page.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &start_page,
            window,
            |view, state, event, window, cx| match event {
                NumberInputEvent::Step(StepAction::Decrement) => {
                    let start_page = match view.task_state.read(cx).start_page {
                        0 | 1 => 1,
                        n => n - 1,
                    };
                    view.task_state
                        .update(cx, |task, _| task.set_start_page(start_page));
                    state.update(cx, |input, cx| {
                        input.set_value(start_page.to_string(), window, cx);
                    });
                }
                NumberInputEvent::Step(StepAction::Increment) => {
                    let start_page = view.task_state.read(cx).start_page + 1;
                    view.task_state
                        .update(cx, |task, _| task.set_start_page(start_page));
                    state.update(cx, |input, cx| {
                        input.set_value(start_page.to_string(), window, cx);
                    });
                }
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &end_page,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    let page = text.parse().unwrap_or(1);
                    view.task_state
                        .update(cx, |task, _| task.set_end_page(page));
                }
                InputEvent::PressEnter { .. } => {
                    view.cookie_input.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &end_page,
            window,
            |view, state, event, window, cx| match event {
                NumberInputEvent::Step(StepAction::Decrement) => {
                    let end_page = match view.task_state.read(cx).end_page {
                        0 | 1 => 1,
                        n => n - 1,
                    };
                    view.task_state
                        .update(cx, |task, _| task.set_end_page(end_page));
                    state.update(cx, |input, cx| {
                        input.set_value(end_page.to_string(), window, cx);
                    });
                }
                NumberInputEvent::Step(StepAction::Increment) => {
                    let end_page = view.task_state.read(cx).end_page + 1;
                    view.task_state
                        .update(cx, |task, _| task.set_end_page(end_page));
                    state.update(cx, |input, cx| {
                        input.set_value(end_page.to_string(), window, cx);
                    });
                }
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &cookie_input,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value().to_string();
                    view.task_state.update(cx, |task, _| task.set_cookie(text));
                }
                InputEvent::PressEnter { .. } => {
                    view.url_input.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.observe(&task_state, |view, _, cx| {
            view.log_table.update(cx, |table, cx| {
                table.refresh(cx);
                cx.notify();
            });
            cx.notify();
        }));

        Self {
            workspace,
            task_state,
            log_table,
            url_input,
            start_page,
            end_page,
            cookie_input,
            _subscriptions,
        }
    }

    fn start_fetch(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::Fresh, cx);
    }

    fn resume_fetch(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::ResumeInterrupted, cx);
    }

    fn retry_failed_page(&mut self, cx: &mut Context<Self>) {
        self.start_fetch_from(RunMode::RetryFailed, cx);
    }

    fn interrupt_fetch(&mut self, cx: &mut Context<Self>) {
        self.task_state.update(cx, |state, cx| {
            state.interrupt();
            cx.notify();
        });
    }

    fn start_fetch_from(&mut self, mode: RunMode, cx: &mut Context<Self>) {
        let conn = match cx.global::<Db>().get() {
            Ok(conn) => conn,
            Err(err) => {
                self.task_state.update(cx, |state, cx| {
                    state.mark_failed(
                        FetchPageError {
                            page: state.start_page.max(1),
                            kind: FetchErrorKind::Database,
                            message: err.to_string(),
                        },
                        None,
                    );
                    cx.notify();
                });
                return;
            }
        };

        let (request, clear_logs) = {
            let state = self.task_state.read(cx);
            if state.is_running() {
                return;
            }
            let start_page = match mode {
                RunMode::Fresh => {
                    if state.start_page > state.end_page {
                        self.task_state.update(cx, |state, cx| {
                            state.mark_failed(
                                FetchPageError {
                                    page: state.start_page,
                                    kind: FetchErrorKind::Other,
                                    message: "起始页不能大于结束页".to_string(),
                                },
                                None,
                            );
                            cx.notify();
                        });
                        return;
                    }
                    state.start_page
                }
                RunMode::ResumeInterrupted => {
                    match resume_page_after_interrupt(
                        state.last_success_page(),
                        state.start_page,
                        state.end_page,
                    ) {
                        Some(page) => page,
                        None => return,
                    }
                }
                RunMode::RetryFailed => {
                    match retry_page_after_failure(
                        state.failed_page(),
                        state.start_page,
                        state.end_page,
                    ) {
                        Some(page) => page,
                        None => return,
                    }
                }
            };
            (
                state.request_from(start_page),
                matches!(mode, RunMode::Fresh),
            )
        };

        let task_state = self.task_state.downgrade();
        self.task_state.update(cx, |state, cx| {
            state.begin_run(request.start_page, 0, clear_logs);
            cx.notify();
        });

        let task = cx.spawn(async move |_, cx| {
            let span = tracing::info_span!(
                "feiwen_fetch",
                start_page = request.start_page,
                end_page = request.end_page,
                has_cookie = !request.cookie.is_empty()
            );
            let mut runner = Runner {
                request,
                task_state,
                conn,
                cx,
            };
            Compat::new(async move { runner.run().await })
                .instrument(span)
                .await;
        });
        self.task_state.update(cx, |state, cx| {
            state.task = Some(task);
            cx.notify();
        });
    }
}

#[derive(Clone, Copy)]
enum RunMode {
    Fresh,
    ResumeInterrupted,
    RetryFailed,
}

impl Render for FetchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_running = self.task_state.read(cx).is_running();
        div()
            .h_full()
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_header(cx))
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
    fn render_header(&self, cx: &mut Context<Self>) -> Div {
        let (route_query_label, title) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-route-query-button"),
                i18n.t("fetch-page-title"),
            )
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border)
            .px_3()
            .py_2()
            .child(
                Button::new("router-query")
                    .label(route_query_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.workspace.update(cx, |_data, cx| {
                            cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Query));
                        });
                    })),
            )
            .child(Label::new(title).text_lg().font_medium())
            .child(self.render_status_badge(cx))
    }

    fn render_form_panel(&self, is_running: bool, cx: &mut Context<Self>) -> Div {
        let (
            section_config,
            field_url,
            field_start_page,
            field_end_page,
            field_cookie,
            cookie_hidden,
            submit_button,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-section-config"),
                i18n.t("fetch-field-url"),
                i18n.t("fetch-field-start-page"),
                i18n.t("fetch-field-end-page"),
                i18n.t("fetch-field-cookie"),
                i18n.t("fetch-cookie-hidden"),
                i18n.t("fetch-submit-button"),
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
            .bg(cx.theme().background)
            .rounded_lg()
            .child(section_title(IconName::Settings, section_config, cx))
            .child(field_label(IconName::Link, field_url, field_color, cx))
            .child(Input::new(&self.url_input).disabled(is_running))
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
                            .child(NumberInput::new(&self.start_page).disabled(is_running)),
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
                            .child(NumberInput::new(&self.end_page).disabled(is_running)),
                    ),
            )
            .child(field_label(IconName::Cookie, field_cookie, field_color, cx))
            .child(Input::new(&self.cookie_input).disabled(is_running))
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
            .child(
                Button::new("fetch-start")
                    .primary()
                    .icon(IconName::CirclePlay)
                    .label(submit_button)
                    .disabled(is_running)
                    .on_click(cx.listener(|this, _, _, cx| this.start_fetch(cx))),
            )
    }

    fn render_status_panel(&self, cx: &mut Context<Self>) -> Div {
        let section_status = cx.global::<I18n>().t("fetch-section-status");
        let status = self.task_state.read(cx).status.clone();
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
            .bg(cx.theme().background)
            .rounded_lg()
            .child(section_title(IconName::Info, section_status, cx))
            .child(status_body)
    }

    fn render_idle_status(&self, cx: &mut Context<Self>) -> Div {
        let (title, desc) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("fetch-state-idle-title"),
                i18n.t("fetch-state-idle-desc"),
            )
        };
        status_layout(
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
                ),
            action_panel(
                IconName::CirclePlay,
                Button::new("fetch-idle-start")
                    .primary()
                    .icon(IconName::CirclePlay)
                    .label(cx.global::<I18n>().t("fetch-submit-button"))
                    .on_click(cx.listener(|this, _, _, cx| this.start_fetch(cx))),
                cx,
            ),
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
                .child(progress_bar(progress, cx))
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
                Button::new("fetch-resume")
                    .warning()
                    .icon(IconName::CirclePlay)
                    .label(resume_label)
                    .on_click(cx.listener(|this, _, _, cx| this.resume_fetch(cx))),
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
                Button::new("fetch-retry-failed")
                    .danger()
                    .icon(IconName::RotateCcw)
                    .label(retry_label)
                    .on_click(cx.listener(|this, _, _, cx| this.retry_failed_page(cx))),
                cx,
            ),
        )
    }

    fn render_success_status(&self, progress: &FetchProgress, cx: &mut Context<Self>) -> Div {
        let title = cx.global::<I18n>().t("fetch-state-success");
        status_layout(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(status_title(
                    IconName::CircleCheck,
                    title,
                    cx.theme().success,
                ))
                .child(metrics_grid(progress, cx)),
            action_panel(
                IconName::RefreshCcw,
                Button::new("fetch-success-restart")
                    .primary()
                    .icon(IconName::RefreshCcw)
                    .label(cx.global::<I18n>().t("fetch-submit-button"))
                    .on_click(cx.listener(|this, _, _, cx| this.start_fetch(cx))),
                cx,
            ),
        )
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
            .bg(cx.theme().background)
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

    fn render_status_badge(&self, cx: &mut Context<Self>) -> Div {
        let status = self.task_state.read(cx).status.clone();
        let (label, color) = match status {
            FetchStatus::Idle => (
                cx.global::<I18n>().t("fetch-state-idle-title"),
                cx.theme().muted_foreground,
            ),
            FetchStatus::Running(_) => (
                cx.global::<I18n>().t("fetch-state-running-title"),
                cx.theme().primary,
            ),
            FetchStatus::Interrupted(_) => (
                cx.global::<I18n>().t("fetch-state-interrupted-title"),
                cx.theme().warning,
            ),
            FetchStatus::Failed(_) => (
                cx.global::<I18n>().t("fetch-state-failed-title"),
                cx.theme().danger,
            ),
            FetchStatus::Success(_) => (
                cx.global::<I18n>().t("fetch-state-success"),
                cx.theme().success,
            ),
        };
        div()
            .rounded_full()
            .px_2()
            .py_1()
            .border_1()
            .border_color(color.opacity(0.35))
            .bg(color.opacity(0.10))
            .child(Label::new(label).text_xs().text_color(color))
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

fn progress_bar(progress: &FetchProgress, cx: &mut Context<FetchView>) -> Div {
    let page_count = progress.page_count().max(1) as f32;
    let completed = progress.completed_pages().min(progress.page_count()) as f32;
    let width = (completed / page_count).clamp(0.0, 1.0);
    div()
        .w_full()
        .h(px(8.))
        .rounded_full()
        .bg(cx.theme().border.opacity(0.35))
        .child(
            div()
                .h_full()
                .w(relative(width))
                .rounded_full()
                .bg(cx.theme().primary),
        )
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
    task_state: Entity<FetchTaskState>,
}

impl FetchLogTableDelegate {
    fn log_at(&self, row_ix: usize, cx: &App) -> Option<FetchPageLog> {
        let logs = &self.task_state.read(cx).logs;
        logs.len()
            .checked_sub(row_ix + 1)
            .and_then(|ix| logs.get(ix))
            .cloned()
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
        self.task_state.read(cx).logs.len()
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
            row = row.bg(cx.theme().danger.opacity(0.06));
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

fn page_count(start_page: u32, end_page: u32) -> u32 {
    end_page.saturating_sub(start_page).saturating_add(1)
}

fn resume_page_after_interrupt(
    last_success_page: Option<u32>,
    start_page: u32,
    end_page: u32,
) -> Option<u32> {
    let next_page = last_success_page
        .map(|page| page.saturating_add(1))
        .unwrap_or(start_page);
    (next_page <= end_page).then_some(next_page.max(start_page))
}

fn retry_page_after_failure(
    failed_page: Option<u32>,
    start_page: u32,
    end_page: u32,
) -> Option<u32> {
    failed_page
        .filter(|page| *page >= start_page && *page <= end_page)
        .or(Some(start_page).filter(|page| *page <= end_page))
}

#[cfg(test)]
mod tests {
    use super::{resume_page_after_interrupt, retry_page_after_failure};

    #[test]
    fn resumes_from_page_after_last_success() {
        assert_eq!(resume_page_after_interrupt(Some(17), 1, 5201), Some(18));
    }

    #[test]
    fn resumes_from_start_without_success_page() {
        assert_eq!(resume_page_after_interrupt(None, 3, 8), Some(3));
    }

    #[test]
    fn resume_returns_none_after_end_page() {
        assert_eq!(resume_page_after_interrupt(Some(8), 3, 8), None);
    }

    #[test]
    fn retries_from_failed_page_inside_range() {
        assert_eq!(retry_page_after_failure(Some(42), 1, 5201), Some(42));
    }

    #[test]
    fn retry_falls_back_to_start_when_failed_page_is_out_of_range() {
        assert_eq!(retry_page_after_failure(Some(9000), 10, 20), Some(10));
    }
}
