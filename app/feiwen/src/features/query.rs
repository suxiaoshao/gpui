use std::collections::HashSet;

use super::{
    Workspace,
    fetch::FetchTaskState,
    workspace::{RouterType, WorkspaceEvent},
};
use crate::{
    errors::FeiwenError,
    foundation::I18n,
    store::{
        Db,
        service::{Novel, TagWithId},
    },
};
use advanced::AdvancedQueryState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme,
    alert::Alert,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    resizable::{h_resizable, resizable_panel, v_resizable},
    table::{DataTable, TableState},
    v_flex,
};
use results_table::ResultsTableDelegate;
use tags_select::TagsSelect;

mod advanced;
mod results_table;
mod tags_select;

#[derive(Default)]
enum QueryData {
    Err(FeiwenError),
    ValidationErr(String),
    Ok(usize),
    #[default]
    Init,
}

enum QueryEvent {
    RouteToFetch,
    Search,
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
    tag_select_view: Entity<TagsSelect>,
    advanced: AdvancedQueryState,
    results_table: Entity<TableState<ResultsTableDelegate>>,
    search_input: Entity<InputState>,
    data: QueryData,
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
        let search_placeholder = cx.global::<I18n>().t("query-search-placeholder");
        Self {
            workspace,
            fetch_task,
            tag_select_view: cx.new(TagsSelect::new),
            advanced: AdvancedQueryState::new(),
            results_table: cx.new(|cx| {
                TableState::new(ResultsTableDelegate::new(), window, cx)
                    .col_resizable(true)
                    .col_movable(true)
                    .row_selectable(true)
            }),
            search_input: cx.new(|cx| InputState::new(window, cx).placeholder(search_placeholder)),
            data: QueryData::Init,
            _subscriptions,
            query,
        }
    }
    fn subscribe_in(
        &mut self,
        _state: &Entity<Query>,
        event: &QueryEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            QueryEvent::RouteToFetch => {
                self.workspace.update(cx, |_data, cx| {
                    cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                });
            }
            QueryEvent::Search => {
                let conn = &mut cx.global::<Db>().get().unwrap();
                let search_name = self.search_input.read(cx).value();
                let selected_tags = self
                    .tag_select_view
                    .read(cx)
                    .get_selected()
                    .into_iter()
                    .map(|TagWithId { name, .. }| name)
                    .collect::<HashSet<_>>();
                let spec = match self.advanced.query_spec(&search_name, &selected_tags, cx) {
                    Ok(spec) => spec,
                    Err(err) => {
                        self.results_table.update(cx, |table, cx| {
                            table.delegate_mut().set_novels(Vec::new());
                            table.refresh(cx);
                            cx.notify();
                        });
                        self.data = QueryData::ValidationErr(err);
                        cx.notify();
                        return;
                    }
                };
                match Novel::query(&spec, conn) {
                    Ok(data) => {
                        let count = data.len();
                        self.results_table.update(cx, |table, cx| {
                            table.delegate_mut().set_novels(data);
                            table.refresh(cx);
                            cx.notify();
                        });
                        self.data = QueryData::Ok(count);
                    }
                    Err(err) => self.data = QueryData::Err(err),
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
        let header = div()
            .flex()
            .flex_initial()
            .gap_2()
            .child(Input::new(&self.search_input))
            .child(
                Button::new("search")
                    .label(search_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.query.update(cx, |_, cx| {
                            cx.emit(QueryEvent::Search);
                        });
                    })),
            )
            .child(
                Button::new("router-fetch")
                    .primary()
                    .label(route_fetch_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.query.update(cx, |_data, cx| {
                            cx.emit(QueryEvent::RouteToFetch);
                        });
                    })),
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
            .child(self.tag_select_view.clone())
            .child(self.render_status(error_label, cx))
            .child(
                h_resizable("query-main")
                    .child(
                        resizable_panel()
                            .size(px(560.))
                            .size_range(px(360.)..px(820.))
                            .flex_none()
                            .child(self.advanced.render_filters(cx)),
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
    fn render_status(&self, error_label: String, cx: &mut Context<Self>) -> Div {
        match &self.data {
            QueryData::Err(feiwen_error) => div()
                .flex_initial()
                .child(Alert::error("error-alert", feiwen_error.to_string()).title(error_label)),
            QueryData::ValidationErr(err) => div()
                .flex_initial()
                .child(Alert::error("query-validation-error", err.clone()).title(error_label)),
            QueryData::Ok(count) => h_flex()
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
            QueryData::Init => div().flex_initial(),
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
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                            });
                        })),
                ),
        )
    }
}
