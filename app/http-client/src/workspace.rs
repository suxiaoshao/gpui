use gpui_kit::component::resizable::{resizable_panel, v_resizable};
use gpui_kit::*;
use http_client_request::{RequestEvent, RequestView};
use http_client_response::{ResponseEvent, ResponseView};

pub(crate) struct WorkspaceView {
    request: Entity<RequestView>,
    response: Entity<ResponseView>,
    _subscriptions: Vec<Subscription>,
}
impl WorkspaceView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let request = cx.new(|cx| RequestView::new(window, cx));
        let response = cx.new(|cx| ResponseView::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&request, window, |this, _, event, window, cx| match event {
                RequestEvent::ResponseChanged(state) => this.response.update(cx, |response, cx| {
                    response.set_state(state.clone(), window, cx)
                }),
            }),
            cx.subscribe(&response, |this, _, event, cx| match event {
                ResponseEvent::ClearRequested => this
                    .request
                    .update(cx, |request, cx| request.clear_response(cx)),
            }),
            cx.observe(&response, |this, response, cx| {
                let running = response.read(cx).save_is_running();
                this.request
                    .update(cx, |request, cx| request.set_save_activity(running, cx));
            }),
        ];
        Self {
            request,
            response,
            _subscriptions: subscriptions,
        }
    }
}
impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            v_resizable("request-response")
                .child(resizable_panel().child(self.request.clone()))
                .child(
                    resizable_panel()
                        .size(window.rem_size() * 20.)
                        .size_range(window.rem_size() * 10. ..Pixels::MAX)
                        .child(self.response.clone()),
                ),
        )
    }
}
