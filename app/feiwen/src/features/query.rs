use super::fetch::FetchRun;
use crate::app::{RouterType, Workspace, WorkspaceEvent};
use crate::{
    foundation::I18n,
    store::{catalog, database, service::Novel},
};
use advanced::{AdvancedQueryController, QueryDraft};
use fluent_bundle::FluentArgs;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable,
    alert::Alert,
    button::Button,
    label::Label,
    resizable::{h_resizable, resizable_panel, v_resizable},
    table::{DataTable, TableState},
    v_flex,
};
use gpui_operation::Transition;
use gpui_store::Store;
use results_table::ResultsTableDelegate;
use std::time::Instant;
use tracing::{Level, event};

pub(crate) mod advanced;
mod form;
mod results_table;

#[derive(Default)]
enum QueryRun {
    #[default]
    Idle,
    Running {
        snapshot: QueryDraft,
        _task: Task<()>,
    },
    Failed {
        snapshot: QueryDraft,
        problem: QueryProblem,
    },
    Succeeded {
        count: usize,
    },
}

impl QueryRun {
    fn is_searching(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

#[derive(Debug)]
struct QueryProblem(String);

impl std::fmt::Display for QueryProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

struct SearchResult {
    novels: Vec<Novel>,
}

enum QueryMessage {
    ClearTerminal,
    Start {
        snapshot: QueryDraft,
        task: Task<()>,
    },
    Complete(Result<SearchResult, QueryProblem>),
    Cancel,
}

enum QueryEffect {
    None,
    ClearResults,
    ShowResults(Vec<Novel>),
}

impl Transition<QueryMessage> for &mut QueryRun {
    type Output = QueryEffect;

    fn transition(self, message: QueryMessage) -> Self::Output {
        match message {
            QueryMessage::ClearTerminal if !self.is_searching() => {
                *self = QueryRun::Idle;
                QueryEffect::ClearResults
            }
            QueryMessage::Start { snapshot, task } if !self.is_searching() => {
                *self = QueryRun::Running {
                    snapshot,
                    _task: task,
                };
                QueryEffect::None
            }
            QueryMessage::Complete(result) if self.is_searching() => {
                let snapshot = match std::mem::take(self) {
                    QueryRun::Running { snapshot, .. } => snapshot,
                    _ => unreachable!(),
                };
                match result {
                    Ok(result) => {
                        *self = QueryRun::Succeeded {
                            count: result.novels.len(),
                        };
                        QueryEffect::ShowResults(result.novels)
                    }
                    Err(problem) => {
                        *self = QueryRun::Failed { snapshot, problem };
                        QueryEffect::ClearResults
                    }
                }
            }
            QueryMessage::Cancel if self.is_searching() => {
                *self = QueryRun::Idle;
                QueryEffect::ClearResults
            }
            _ => {
                tracing::debug!("ignored query transition");
                QueryEffect::None
            }
        }
    }
}

pub(crate) struct QueryView {
    workspace: Entity<Workspace>,
    fetch_task: Store<FetchRun>,
    advanced: AdvancedQueryController,
    results_table: Entity<TableState<ResultsTableDelegate>>,
    search: QueryRun,
    _subscriptions: Vec<Subscription>,
}

impl QueryView {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        fetch_task: Store<FetchRun>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscriptions = vec![
            fetch_task.observe(cx, |_, _, cx| cx.notify()),
            catalog::store(cx).observe_select_in(
                cx,
                window,
                |state: &catalog::QueryCatalogState| {
                    (state.operation.phase(), state.operation.problem().cloned())
                },
                |_, _, _, cx| cx.notify(),
            ),
            catalog::store(cx).observe_select_in(
                cx,
                window,
                |state: &catalog::QueryCatalogState| state.operation.data().cloned(),
                |view, options, window, cx| {
                    if let Some(options) = options.clone() {
                        view.advanced.update_options(options, window, cx);
                    }
                    cx.notify();
                },
            ),
        ];
        let options = catalog::data(cx).unwrap_or_default();
        let search = QueryRun::Idle;
        Self {
            workspace,
            fetch_task,
            advanced: AdvancedQueryController::new(options, window, cx),
            results_table: cx.new(|cx| {
                TableState::new(ResultsTableDelegate::new(), window, cx)
                    .col_resizable(true)
                    .col_movable(true)
                    .row_selectable(true)
            }),
            search,
            _subscriptions,
        }
    }

    pub(crate) fn request_search(&mut self, cx: &mut Context<Self>) {
        self.start_search(cx);
    }

    pub(crate) fn request_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.advanced.reset(window, cx);
        cx.notify();
    }

    pub(crate) fn request_cancel(&mut self, cx: &mut Context<Self>) {
        let effect = self.search.transition(QueryMessage::Cancel);
        self.apply_query_effect(effect, cx);
        cx.notify();
    }

    pub(crate) fn is_searching(&self) -> bool {
        self.search.is_searching()
    }

    pub(crate) fn can_search(&self, cx: &App) -> bool {
        !self.is_searching()
            && database::is_ready(cx)
            && catalog::phase(cx) == gpui_operation::refresh::Phase::Ready
    }

    pub(crate) fn titlebar_summary(&self, i18n: &I18n) -> String {
        query_titlebar_summary(
            self.advanced.condition_count(),
            self.advanced.sort_count(),
            &self.search,
            i18n,
        )
    }
}

impl Render for QueryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error_label = cx.global::<I18n>().t("query-error-title");
        let catalog_disabled = catalog::phase(cx) != gpui_operation::refresh::Phase::Ready;
        div()
            .flex_1()
            .p_2()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .when_some(self.render_fetch_summary(cx), |this, summary| {
                this.child(summary)
            })
            .when_some(self.render_catalog_status(cx), |this, status| {
                this.child(status)
            })
            .child(self.render_status(error_label, cx))
            .child(
                h_resizable("query-main")
                    .child(
                        resizable_panel()
                            .size(px(560.))
                            .size_range(px(360.)..px(820.))
                            .flex_none()
                            .child(self.advanced.render_filters(catalog_disabled, cx)),
                    )
                    .child(
                        v_resizable("query-side")
                            .child(
                                resizable_panel()
                                    .size(px(220.))
                                    .size_range(px(150.)..px(420.))
                                    .child(self.advanced.render_sorts(cx)),
                            )
                            .child(resizable_panel().child(self.render_results_table(cx))),
                    ),
            )
    }
}

impl QueryView {
    fn start_search(&mut self, cx: &mut Context<Self>) {
        if !self.can_search(cx) {
            event!(Level::INFO, "ignored query request outside the ready gate");
            return;
        }

        let pool = match database::ready_pool(cx) {
            Ok(pool) => pool,
            Err(problem) => {
                event!(Level::INFO, error = %problem, "query database gate closed");
                return;
            }
        };

        let effect = self.search.transition(QueryMessage::ClearTerminal);
        self.apply_query_effect(effect, cx);

        let prepared = match self.advanced.prepare(cx) {
            Ok(prepared) => prepared,
            Err(err) => {
                event!(Level::ERROR, error = %err, "query validation failed");
                cx.notify();
                return;
            }
        };
        let prepared = prepared.map(|draft| {
            let spec = draft.to_spec();
            (draft, spec)
        });
        let (_, (snapshot, spec)) = prepared.into_parts();
        let spec = match spec {
            Ok(spec) => spec,
            Err(err) => {
                event!(Level::ERROR, error = %err, "query compilation failed");
                cx.notify();
                return;
            }
        };

        event!(
            Level::INFO,
            filter_count = spec.filter_count(),
            sort_count = spec.sort_count(),
            "starting feiwen query"
        );
        let this = cx.entity().downgrade();

        let task = cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn(async move {
                    let started_at = Instant::now();
                    event!(
                        Level::INFO,
                        filter_count = spec.filter_count(),
                        sort_count = spec.sort_count(),
                        "running feiwen query in background"
                    );
                    let query_started_at = Instant::now();
                    let conn = pool
                        .get()
                        .map_err(|error| QueryProblem(error.to_string()))?;
                    let novels = Novel::query(&spec, &conn)
                        .map_err(|error| QueryProblem(error.to_string()))?;
                    let query_elapsed_ms = query_started_at.elapsed().as_millis();
                    event!(
                        Level::INFO,
                        result_count = novels.len(),
                        query_elapsed_ms,
                        total_elapsed_ms = started_at.elapsed().as_millis(),
                        "feiwen query completed in background"
                    );
                    Ok(SearchResult { novels })
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_search(result, window, cx)
            });
        });
        let effect = self
            .search
            .transition(QueryMessage::Start { snapshot, task });
        self.apply_query_effect(effect, cx);
        self.results_table.update(cx, |table, cx| {
            table.delegate_mut().set_loading(true);
            table.refresh(cx);
            cx.notify();
        });
        cx.notify();
    }

    fn finish_search(
        &mut self,
        result: Result<SearchResult, QueryProblem>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = &result {
            event!(Level::ERROR, error = %error, "feiwen query failed");
        }
        let effect = self.search.transition(QueryMessage::Complete(result));
        self.apply_query_effect(effect, cx);
        cx.notify();
    }

    fn apply_query_effect(&mut self, effect: QueryEffect, cx: &mut Context<Self>) {
        match effect {
            QueryEffect::None => {}
            QueryEffect::ClearResults => self.set_results_table(Vec::new(), false, cx),
            QueryEffect::ShowResults(novels) => {
                let count = novels.len();
                let table_started_at = Instant::now();
                self.set_results_table(novels, false, cx);
                event!(
                    Level::INFO,
                    result_count = count,
                    set_results_table_elapsed_ms = table_started_at.elapsed().as_millis(),
                    "feiwen query succeeded"
                );
            }
        }
    }

    fn set_results_table(&mut self, novels: Vec<Novel>, loading: bool, cx: &mut Context<Self>) {
        self.results_table.update(cx, |table, cx| {
            table.delegate_mut().set_novels(novels);
            table.delegate_mut().set_loading(loading);
            table.refresh(cx);
            cx.notify();
        });
    }

    fn render_status(&self, error_label: String, cx: &mut Context<Self>) -> Div {
        match &self.search {
            QueryRun::Failed { problem: err, .. } => {
                div()
                    .flex_initial()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Alert::error("query-error-alert", err.to_string()).title(error_label))
                    .child(
                        Button::new("query-load-failed-snapshot")
                            .label(cx.global::<I18n>().t("query-action-load-snapshot"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.load_failed_snapshot(window, cx)
                            })),
                    )
            }
            QueryRun::Idle | QueryRun::Running { .. } | QueryRun::Succeeded { .. } => {
                div().flex_initial()
            }
        }
    }

    fn load_failed_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let QueryRun::Failed { snapshot, .. } = &self.search else {
            return;
        };
        self.advanced.load_draft(snapshot.clone(), window, cx);
        cx.notify();
    }

    fn render_results_table(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(DataTable::new(&self.results_table))
    }

    fn render_fetch_summary(&self, cx: &mut Context<Self>) -> Option<Div> {
        let i18n = cx.global::<I18n>();
        let summary = self.fetch_task.read(cx, |task| {
            task.has_visible_summary()
                .then(|| task.summary_text(i18n))
                .flatten()
        });
        let summary = summary?;
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .bg(cx.theme().tokens.accent.background.opacity(0.35))
                .px_3()
                .py_2()
                .child(
                    Label::new(summary)
                        .text_sm()
                        .text_color(cx.theme().foreground),
                )
                .child(
                    Button::new("query-fetch-summary-open")
                        .label(i18n.t("fetch-summary-open"))
                        .disabled(self.search.is_searching())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                            });
                        })),
                ),
        )
    }

    fn render_catalog_status(&self, cx: &mut Context<Self>) -> Option<Div> {
        use gpui_operation::refresh::Phase;

        let phase = catalog::phase(cx);
        if phase == Phase::Ready {
            return None;
        }
        let i18n = cx.global::<I18n>();
        let message = catalog::problem(cx)
            .map(|problem| problem.to_string())
            .unwrap_or_else(|| i18n.t("query-catalog-loading"));
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Alert::warning("query-catalog-status", message)
                        .title(i18n.t("query-catalog-title")),
                )
                .child(
                    Button::new("query-catalog-reload")
                        .label(i18n.t("query-catalog-reload"))
                        .disabled(matches!(
                            phase,
                            Phase::Loading
                                | Phase::Refreshing
                                | Phase::Retrying
                                | Phase::RefreshingDegraded
                        ))
                        .on_click(|_, _, cx| catalog::request_load(cx)),
                ),
        )
    }
}

fn query_titlebar_summary(
    conditions: usize,
    sorts: usize,
    search: &QueryRun,
    i18n: &I18n,
) -> String {
    format!(
        "{} · {} · {}",
        count_message(i18n, "query-titlebar-conditions", conditions),
        count_message(i18n, "query-titlebar-sorts", sorts),
        query_titlebar_result_label(search, i18n)
    )
}

fn query_titlebar_result_label(search: &QueryRun, i18n: &I18n) -> String {
    match search {
        QueryRun::Idle => i18n.t("query-titlebar-no-results"),
        QueryRun::Running { .. } => i18n.t("query-titlebar-searching"),
        QueryRun::Succeeded { count } => count_message(i18n, "query-titlebar-results", *count),
        QueryRun::Failed { .. } => i18n.t("query-titlebar-failed"),
    }
}

fn count_message(i18n: &I18n, key: &str, count: usize) -> String {
    let mut args = FluentArgs::new();
    args.set("count", count);
    i18n.t_with_args(key, &args)
}

#[cfg(test)]
mod tests {
    use gpui::Task;

    use super::{
        QueryEffect, QueryMessage, QueryProblem, QueryRun, SearchResult, query_titlebar_summary,
    };
    use crate::foundation::i18n::I18n;
    use gpui_operation::Transition;

    #[::core::prelude::v1::test]
    fn query_run_searching_status_matches_state_variants() {
        assert!(!QueryRun::Idle.is_searching());
        assert!(!matches!(QueryRun::Idle, QueryRun::Succeeded { .. }));
        assert!(!matches!(QueryRun::Idle, QueryRun::Failed { .. }));

        let task = QueryRun::Running {
            snapshot: Default::default(),
            _task: Task::ready(()),
        };
        assert!(task.is_searching());

        let data = QueryRun::Succeeded { count: 3 };
        assert!(!data.is_searching());
        assert!(matches!(data, QueryRun::Succeeded { .. }));

        let error = QueryRun::Failed {
            snapshot: Default::default(),
            problem: QueryProblem("查询失败".to_owned()),
        };
        assert!(!error.is_searching());
        assert!(matches!(error, QueryRun::Failed { .. }));
    }

    #[test]
    fn query_titlebar_summary_reflects_search_state() {
        let i18n = I18n::chinese_for_test();
        assert_eq!(
            query_titlebar_summary(0, 0, &QueryRun::Idle, &i18n),
            "0 条条件 · 0 条排序 · 暂无结果"
        );
        assert_eq!(
            query_titlebar_summary(2, 1, &QueryRun::Succeeded { count: 8 }, &i18n),
            "2 条条件 · 1 条排序 · 8 条结果"
        );
        assert_eq!(
            query_titlebar_summary(
                1,
                0,
                &QueryRun::Failed {
                    snapshot: Default::default(),
                    problem: QueryProblem("查询失败".to_owned()),
                },
                &i18n
            ),
            "1 条条件 · 0 条排序 · 查询失败"
        );
    }

    #[test]
    fn query_titlebar_summary_uses_english_locale() {
        let i18n = I18n::english_for_test();
        assert_eq!(
            query_titlebar_summary(0, 0, &QueryRun::Idle, &i18n),
            "0 conditions · 0 sorts · No results"
        );
        assert_eq!(
            query_titlebar_summary(2, 1, &QueryRun::Succeeded { count: 8 }, &i18n),
            "2 conditions · 1 sorts · 8 results"
        );
    }

    #[test]
    fn second_query_start_is_discarded_while_running() {
        let mut first = super::QueryDraft::default();
        first.filters.negated = true;
        let mut state = QueryRun::Idle;
        state.transition(QueryMessage::Start {
            snapshot: first.clone(),
            task: Task::ready(()),
        });
        state.transition(QueryMessage::Start {
            snapshot: Default::default(),
            task: Task::ready(()),
        });
        let QueryRun::Running { snapshot, .. } = state else {
            panic!("query should remain running");
        };
        assert_eq!(snapshot, first);
    }

    #[test]
    fn clear_terminal_happens_before_a_new_query_attempt() {
        let mut state = QueryRun::Failed {
            snapshot: Default::default(),
            problem: QueryProblem("old".to_owned()),
        };
        assert!(matches!(
            state.transition(QueryMessage::ClearTerminal),
            QueryEffect::ClearResults
        ));
        assert!(matches!(state, QueryRun::Idle));
    }

    #[test]
    fn completion_effect_carries_rows_without_a_second_result_authority() {
        let mut state = QueryRun::Idle;
        state.transition(QueryMessage::Start {
            snapshot: Default::default(),
            task: Task::ready(()),
        });
        let effect = state.transition(QueryMessage::Complete(Ok(SearchResult {
            novels: Vec::new(),
        })));
        assert!(matches!(effect, QueryEffect::ShowResults(rows) if rows.is_empty()));
        assert!(matches!(state, QueryRun::Succeeded { count: 0 }));
    }

    #[test]
    fn failed_run_preserves_the_exact_submitted_form_snapshot() {
        let mut snapshot = super::QueryDraft::default();
        snapshot.filters.negated = true;
        let mut state = QueryRun::Idle;
        state.transition(QueryMessage::Start {
            snapshot: snapshot.clone(),
            task: Task::ready(()),
        });
        state.transition(QueryMessage::Complete(Err(QueryProblem(
            "failed".to_owned(),
        ))));

        let QueryRun::Failed {
            snapshot: failed_snapshot,
            ..
        } = state
        else {
            panic!("query should retain its failed snapshot");
        };
        assert_eq!(failed_snapshot, snapshot);
    }
}
