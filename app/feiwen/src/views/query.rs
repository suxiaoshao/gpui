use std::collections::HashSet;

use super::{
    Workspace,
    workspace::{RouterType, WorkspaceEvent},
};
use crate::{
    errors::FeiwenError,
    store::{
        Db,
        service::{Novel, TagWithId},
    },
};
use gpui::*;
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    scroll::ScrollableElement,
    v_flex,
};
use tags_select::TagsSelect;

mod tags_select;

#[derive(Default)]
enum QueryData {
    Err(FeiwenError),
    Ok(Vec<Novel>),
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
    tag_select_view: Entity<TagsSelect>,
    search_input: Entity<InputState>,
    data: QueryData,
    _subscriptions: Vec<Subscription>,
    query: Entity<Query>,
}

impl QueryView {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|_cx| Query::new());
        let _subscriptions = vec![cx.subscribe_in(&query, window, Self::subscribe_in)];
        Self {
            workspace,
            tag_select_view: cx.new(TagsSelect::new),
            search_input: cx.new(|cx| InputState::new(window, cx).placeholder("Search")),
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
                match Novel::search(&search_name, &selected_tags, conn) {
                    Ok(data) => self.data = QueryData::Ok(data),
                    Err(err) => self.data = QueryData::Err(err),
                }
                cx.notify();
            }
        }
    }
}

impl Render for QueryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header =
            div()
                .flex()
                .flex_initial()
                .gap_2()
                .child(Input::new(&self.search_input))
                .child(Button::new("search").label("Search").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.query.update(cx, |_, cx| {
                            cx.emit(QueryEvent::Search);
                        });
                    },
                )))
                .child(
                    Button::new("router-fetch")
                        .primary()
                        .label("fetch")
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
            .child(self.tag_select_view.clone())
            .child(match &self.data {
                QueryData::Err(feiwen_error) => div()
                    .flex_1()
                    .child(Alert::error("error-alert", feiwen_error.to_string()).title("Error")),
                QueryData::Ok(novels) => v_flex().flex_1().overflow_hidden().child(
                    v_flex()
                        .id("novel-list")
                        .size_full()
                        .children(novels.iter().take(100).cloned())
                        .overflow_y_scrollbar(),
                ),
                QueryData::Init => div(),
            })
    }
}
