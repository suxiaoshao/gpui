use crate::{hotkey::TemporaryData, views::temporary::detail::TemplateDetailView};
use gpui::*;
use tracing::{Level, event};

mod detail;

pub fn init(cx: &mut App) {
    event!(Level::INFO, "Initializing temporary view");
    detail::init(cx);
}

pub(crate) struct TemporaryView {
    _subscription: Vec<Subscription>,
    pub(crate) detail: Entity<TemplateDetailView>,
}

impl TemporaryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscription = vec![cx.observe_window_activation(window, |_this, window, cx| {
            if !window.is_window_active() {
                TemporaryData::hide_with_delay(window, cx);
            }
        })];
        let detail = cx.new(|cx| TemplateDetailView::new(window, cx));
        Self {
            _subscription,
            detail,
        }
    }
}

impl Render for TemporaryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.detail.clone())
    }
}
