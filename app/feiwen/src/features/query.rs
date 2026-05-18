use super::fetch::FetchTaskState;
use crate::app::{RouterType, Workspace, WorkspaceEvent};
use crate::{
    errors::{FeiwenError, FeiwenResult},
    foundation::I18n,
    store::{Db, service::Novel},
};
use advanced::{AdvancedQueryState, QueryOptions};
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
use results_table::ResultsTableDelegate;
use std::time::Instant;
use tracing::{Level, event};

mod advanced;
mod results_table;

#[derive(Default)]
enum SearchState {
    #[default]
    Init,
    Task(Task<()>),
    Error(QueryError),
    Data {
        count: usize,
    },
}

impl SearchState {
    fn is_searching(&self) -> bool {
        match self {
            Self::Task(task) => {
                let _ = task.is_ready();
                true
            }
            _ => false,
        }
    }
}

enum QueryError {
    Runtime(FeiwenError),
    Validation(String),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime(err) => err.fmt(f),
            Self::Validation(err) => f.write_str(err),
        }
    }
}

struct SearchResult {
    novels: Vec<Novel>,
}

enum QueryEvent {
    Search,
    Reset,
}

impl EventEmitter<QueryEvent> for Query {}

struct Query {}

impl Query {
    fn new() -> Self {
        Self {}
    }
}

pub(crate) struct QueryView {
    workspace: Entity<Workspace>,
    fetch_task: Entity<FetchTaskState>,
    advanced: AdvancedQueryState,
    results_table: Entity<TableState<ResultsTableDelegate>>,
    search: SearchState,
    _subscriptions: Vec<Subscription>,
    query: Entity<Query>,
}

impl QueryView {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        fetch_task: Entity<FetchTaskState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|_cx| Query::new());
        let _subscriptions = vec![
            cx.subscribe_in(&query, window, Self::subscribe_in),
            cx.observe(&fetch_task, |_, _, cx| {
                cx.notify();
            }),
        ];
        let (options, search) = match cx.global::<Db>().get() {
            Ok(conn) => match QueryOptions::load(&conn) {
                Ok(options) => (options, SearchState::Init),
                Err(err) => (
                    QueryOptions::default(),
                    SearchState::Error(QueryError::Runtime(err)),
                ),
            },
            Err(err) => (
                QueryOptions::default(),
                SearchState::Error(QueryError::Runtime(err.into())),
            ),
        };
        Self {
            workspace,
            fetch_task,
            advanced: AdvancedQueryState::new(options, window, cx),
            results_table: cx.new(|cx| {
                TableState::new(ResultsTableDelegate::new(), window, cx)
                    .col_resizable(true)
                    .col_movable(true)
                    .row_selectable(true)
            }),
            search,
            _subscriptions,
            query,
        }
    }

    pub(crate) fn request_search(&mut self, cx: &mut Context<Self>) {
        self.query.update(cx, |_, cx| {
            cx.emit(QueryEvent::Search);
        });
    }

    pub(crate) fn request_reset(&mut self, cx: &mut Context<Self>) {
        self.query.update(cx, |_, cx| {
            cx.emit(QueryEvent::Reset);
        });
    }

    pub(crate) fn is_searching(&self) -> bool {
        self.search.is_searching()
    }

    pub(crate) fn titlebar_summary(&self, i18n: &I18n) -> String {
        query_titlebar_summary(
            self.advanced.condition_count(),
            self.advanced.sort_count(),
            &self.search,
            i18n,
        )
    }

    fn subscribe_in(
        &mut self,
        _state: &Entity<Query>,
        event: &QueryEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            QueryEvent::Search => {
                self.start_search(cx);
            }
            QueryEvent::Reset => {
                if self.search.is_searching() {
                    return;
                }
                let options = match cx.global::<Db>().get() {
                    Ok(conn) => QueryOptions::load(&conn),
                    Err(err) => Err(err.into()),
                };
                match options {
                    Ok(options) => {
                        self.advanced = AdvancedQueryState::new(options, window, cx);
                        self.set_results_table(Vec::new(), false, cx);
                        self.search = SearchState::Init;
                    }
                    Err(err) => self.search = SearchState::Error(QueryError::Runtime(err)),
                }
                cx.notify();
            }
        }
    }
}

impl Render for QueryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error_label = cx.global::<I18n>().t("query-error-title");
        let searching = self.search.is_searching();
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
            .child(self.render_status(error_label, cx))
            .child(
                h_resizable("query-main")
                    .child(
                        resizable_panel()
                            .size(px(560.))
                            .size_range(px(360.)..px(820.))
                            .flex_none()
                            .child(self.advanced.render_filters(searching, cx)),
                    )
                    .child(
                        v_resizable("query-side")
                            .child(
                                resizable_panel()
                                    .size(px(220.))
                                    .size_range(px(150.)..px(420.))
                                    .child(self.advanced.render_sorts(searching, cx)),
                            )
                            .child(resizable_panel().child(self.render_results_table(cx))),
                    ),
            )
    }
}

impl QueryView {
    fn start_search(&mut self, cx: &mut Context<Self>) {
        if self.search.is_searching() {
            event!(Level::INFO, "ignored query request while search is running");
            return;
        }

        let spec = match self.advanced.query_spec(cx) {
            Ok(spec) => spec,
            Err(err) => {
                event!(Level::ERROR, error = %err, "query validation failed");
                self.set_results_table(Vec::new(), false, cx);
                self.search = SearchState::Error(QueryError::Validation(err));
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
        let pool = cx.global::<Db>().pool();
        let this = cx.entity().downgrade();

        self.advanced.set_disabled(true, cx);
        self.set_table_loading(true, cx);

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
                    let conn = pool.get()?;
                    let novels = Novel::query(&spec, &conn)?;
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
        self.search = SearchState::Task(task);
        cx.notify();
    }

    fn finish_search(
        &mut self,
        result: FeiwenResult<SearchResult>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.advanced.set_disabled(false, cx);
        match result {
            Ok(result) => {
                let count = result.novels.len();
                let table_started_at = Instant::now();
                self.set_results_table(result.novels, false, cx);
                event!(
                    Level::INFO,
                    result_count = count,
                    set_results_table_elapsed_ms = table_started_at.elapsed().as_millis(),
                    "feiwen query succeeded"
                );
                self.search = SearchState::Data { count };
            }
            Err(err) => {
                event!(Level::ERROR, error = %err, "feiwen query failed");
                self.set_results_table(Vec::new(), false, cx);
                self.search = SearchState::Error(QueryError::Runtime(err));
            }
        }
        cx.notify();
    }

    fn set_results_table(&mut self, novels: Vec<Novel>, loading: bool, cx: &mut Context<Self>) {
        self.results_table.update(cx, |table, cx| {
            table.delegate_mut().set_novels(novels);
            table.delegate_mut().set_loading(loading);
            table.refresh(cx);
            cx.notify();
        });
    }

    fn set_table_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.results_table.update(cx, |table, cx| {
            table.delegate_mut().set_loading(loading);
            table.refresh(cx);
            cx.notify();
        });
    }

    fn render_status(&self, error_label: String, _cx: &mut Context<Self>) -> Div {
        match &self.search {
            SearchState::Error(err) => div()
                .flex_initial()
                .child(Alert::error("query-error-alert", err.to_string()).title(error_label)),
            SearchState::Init | SearchState::Task(_) | SearchState::Data { .. } => {
                div().flex_initial()
            }
        }
    }

    fn render_results_table(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(DataTable::new(&self.results_table))
    }

    fn render_fetch_summary(&self, cx: &mut Context<Self>) -> Option<Div> {
        let i18n = cx.global::<I18n>();
        let task = self.fetch_task.read(cx);
        if !task.has_visible_summary() {
            return None;
        }
        let summary = task.summary_text(i18n)?;
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .bg(cx.theme().accent.opacity(0.35))
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
}

fn query_titlebar_summary(
    conditions: usize,
    sorts: usize,
    search: &SearchState,
    i18n: &I18n,
) -> String {
    format!(
        "{} · {} · {}",
        count_message(i18n, "query-titlebar-conditions", conditions),
        count_message(i18n, "query-titlebar-sorts", sorts),
        query_titlebar_result_label(search, i18n)
    )
}

fn query_titlebar_result_label(search: &SearchState, i18n: &I18n) -> String {
    match search {
        SearchState::Init => i18n.t("query-titlebar-no-results"),
        SearchState::Task(_) => i18n.t("query-titlebar-searching"),
        SearchState::Data { count } => count_message(i18n, "query-titlebar-results", *count),
        SearchState::Error(_) => i18n.t("query-titlebar-failed"),
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

    use super::{QueryError, SearchState, query_titlebar_summary};
    use crate::foundation::i18n::I18n;

    #[::core::prelude::v1::test]
    fn search_state_searching_status_matches_state_variants() {
        assert!(!SearchState::Init.is_searching());
        assert!(!matches!(SearchState::Init, SearchState::Data { .. }));
        assert!(!matches!(SearchState::Init, SearchState::Error(_)));

        let task = SearchState::Task(Task::ready(()));
        assert!(task.is_searching());

        let data = SearchState::Data { count: 3 };
        assert!(!data.is_searching());
        assert!(matches!(data, SearchState::Data { .. }));

        let error = SearchState::Error(QueryError::Validation("请选择字段".to_owned()));
        assert!(!error.is_searching());
        assert!(matches!(error, SearchState::Error(_)));
    }

    #[test]
    fn query_titlebar_summary_reflects_search_state() {
        let i18n = I18n::chinese_for_test();
        assert_eq!(
            query_titlebar_summary(0, 0, &SearchState::Init, &i18n),
            "0 条条件 · 0 条排序 · 暂无结果"
        );
        assert_eq!(
            query_titlebar_summary(2, 1, &SearchState::Data { count: 8 }, &i18n),
            "2 条条件 · 1 条排序 · 8 条结果"
        );
        assert_eq!(
            query_titlebar_summary(
                1,
                0,
                &SearchState::Error(QueryError::Validation("请选择字段".to_owned())),
                &i18n
            ),
            "1 条条件 · 0 条排序 · 查询失败"
        );
    }

    #[test]
    fn query_titlebar_summary_uses_english_locale() {
        let i18n = I18n::english_for_test();
        assert_eq!(
            query_titlebar_summary(0, 0, &SearchState::Init, &i18n),
            "0 conditions · 0 sorts · No results"
        );
        assert_eq!(
            query_titlebar_summary(2, 1, &SearchState::Data { count: 8 }, &i18n),
            "2 conditions · 1 sorts · 8 results"
        );
    }
}
