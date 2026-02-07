use crate::{
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    hotkey::TemporaryData,
    views::temporary::{detail::TemplateDetailView, list::TemporaryList},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Sizable,
    alert::Alert,
    list::{List, ListState},
};
use tracing::{Level, event};

mod detail;
mod list;

const CONTEXT: &str = "temporary-list";

pub fn init(cx: &mut App) {
    event!(Level::INFO, "Initializing temporary view");
    detail::init(cx);
}

pub(crate) struct TemporaryView {
    _subscription: Vec<Subscription>,
    templates: AiChatResult<Entity<ListState<TemporaryList>>>,
    focus_handle: FocusHandle,
    selected_item: Option<Entity<TemplateDetailView>>,
}

impl TemporaryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscription = vec![cx.observe_window_activation(window, |_this, window, cx| {
            if !window.is_window_active() {
                let temporary_data = cx.global_mut::<TemporaryData>();
                temporary_data.hide(window);
            }
        })];
        let templates = Self::get_templates(cx).map(|templates| {
            let on_confirm = cx.listener(move |this, state: &ConversationTemplate, window, cx| {
                let on_esc = cx.listener(|this, _, window, cx| {
                    this.selected_item = None;
                    if let Ok(templates) = &this.templates {
                        templates.update(cx, |this, cx| {
                            this.focus(window, cx);
                        });
                    }
                    cx.notify();
                });
                this.selected_item =
                    Some(cx.new(move |cx| TemplateDetailView::new(state, on_esc, window, cx)));
            });
            cx.new(move |cx| {
                let mut list_state =
                    ListState::new(TemporaryList::new(templates, on_confirm), window, cx)
                        .searchable(true);
                list_state.focus(window, cx);
                list_state
            })
        });
        Self {
            _subscription,
            templates,
            focus_handle: cx.focus_handle(),
            selected_item: None,
        }
    }
    fn get_templates(cx: &mut Context<Self>) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }
}

impl Render for TemporaryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .map(|this| {
                this.map(|this| match &self.selected_item {
                    Some(selected_item) => this.child(selected_item.clone()),
                    None => match &self.templates {
                        Ok(templates) => this.child(List::new(templates).large()),
                        Err(err) => this
                            .child(Alert::error("temporary-alert", err.to_string()).title("Error")),
                    },
                })
            })
    }
}
