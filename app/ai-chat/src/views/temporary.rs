use gpui::*;

pub(crate) struct TemporaryView {
    _subscription: Vec<Subscription>,
}

impl TemporaryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscription = vec![cx.observe_window_activation(window, |this, window, cx| {})];
        Self { _subscription }
    }
}

impl Render for TemporaryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}
