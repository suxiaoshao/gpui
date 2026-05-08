use super::{
    Workspace,
    fetch::FetchTaskState,
    workspace::{RouterType, WorkspaceEvent},
};
use crate::{
    errors::{FeiwenError, FeiwenResult},
    foundation::I18n,
    store::{Db, service::Novel},
};
use advanced::{AdvancedQueryState, QueryOptions};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    resizable::{h_resizable, resizable_panel, v_resizable},
    table::{DataTable, TableState},
    v_flex,
};
use results_table::ResultsTableDelegate;
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

    #[cfg(test)]
    fn is_data(&self) -> bool {
        matches!(self, Self::Data { .. })
    }

    #[cfg(test)]
    fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
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
    options: QueryOptions,
    novels: Vec<Novel>,
}

enum QueryEvent {
    RouteToFetch,
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
            Ok(mut conn) => match QueryOptions::load(&mut conn) {
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
    fn subscribe_in(
        &mut self,
        _state: &Entity<Query>,
        event: &QueryEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            QueryEvent::RouteToFetch => {
                if self.search.is_searching() {
                    return;
                }
                self.workspace.update(cx, |_data, cx| {
                    cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                });
            }
            QueryEvent::Search => {
                self.start_search(cx);
            }
            QueryEvent::Reset => {
                if self.search.is_searching() {
                    return;
                }
                let options = match cx.global::<Db>().get() {
                    Ok(mut conn) => QueryOptions::load(&mut conn),
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
        let (search_label, route_fetch_label, error_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("query-search-button"),
                i18n.t("query-route-fetch-button"),
                i18n.t("query-error-title"),
            )
        };
        let searching = self.search.is_searching();
        let header = div()
            .flex()
            .flex_initial()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                v_flex()
                    .gap_1()
                    .child(Label::new("高级检索").font_semibold())
                    .child(
                        Label::new("通过结构化条件、集合选择和字段排序检索作品")
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("query-reset")
                            .label("重置")
                            .disabled(searching)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.query.update(cx, |_, cx| {
                                    cx.emit(QueryEvent::Reset);
                                });
                            })),
                    )
                    .child(
                        Button::new("search")
                            .primary()
                            .label(search_label)
                            .loading(searching)
                            .disabled(searching)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.query.update(cx, |_, cx| {
                                    cx.emit(QueryEvent::Search);
                                });
                            })),
                    )
                    .child(
                        Button::new("router-fetch")
                            .label(route_fetch_label)
                            .disabled(searching)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.query.update(cx, |_data, cx| {
                                    cx.emit(QueryEvent::RouteToFetch);
                                });
                            })),
                    ),
            );
        div()
            .flex_1()
            .p_2()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(header)
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

        event!(Level::INFO, "starting feiwen query");
        let pool = cx.global::<Db>().pool();
        let this = cx.entity().downgrade();

        self.advanced.set_disabled(true, cx);
        self.set_table_loading(true, cx);

        let task = cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn(async move {
                    event!(Level::INFO, "running feiwen query in background");
                    let mut conn = pool.get()?;
                    let options = QueryOptions::load(&mut conn)?;
                    let novels = Novel::query(&spec, &mut conn)?;
                    event!(
                        Level::INFO,
                        result_count = novels.len(),
                        "feiwen query completed in background"
                    );
                    Ok(SearchResult { options, novels })
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_search(result, cx));
        });
        self.search = SearchState::Task(task);
        cx.notify();
    }

    fn finish_search(&mut self, result: FeiwenResult<SearchResult>, cx: &mut Context<Self>) {
        self.advanced.set_disabled(false, cx);
        match result {
            Ok(result) => {
                let count = result.novels.len();
                event!(Level::INFO, result_count = count, "feiwen query succeeded");
                self.advanced.set_options(result.options, cx);
                self.set_results_table(result.novels, false, cx);
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

    fn render_status(&self, error_label: String, cx: &mut Context<Self>) -> Div {
        match &self.search {
            SearchState::Error(err) => div()
                .flex_initial()
                .child(Alert::error("query-error-alert", err.to_string()).title(error_label)),
            SearchState::Data { count } => h_flex()
                .flex_initial()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(cx.theme().accent.opacity(0.35))
                .child(
                    Label::new(format!("共 {count} 条结果"))
                        .text_sm()
                        .text_color(cx.theme().foreground),
                ),
            SearchState::Init | SearchState::Task(_) => div().flex_initial(),
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

#[cfg(test)]
mod tests {
    use gpui::Task;

    use super::{QueryError, SearchState};

    #[::core::prelude::v1::test]
    fn search_state_helpers_match_state_variants() {
        assert!(!SearchState::Init.is_searching());
        assert!(!SearchState::Init.is_data());
        assert!(!SearchState::Init.is_error());

        let task = SearchState::Task(Task::ready(()));
        assert!(task.is_searching());

        let data = SearchState::Data { count: 3 };
        assert!(!data.is_searching());
        assert!(data.is_data());

        let error = SearchState::Error(QueryError::Validation("请选择字段".to_owned()));
        assert!(!error.is_searching());
        assert!(error.is_error());
    }
}
