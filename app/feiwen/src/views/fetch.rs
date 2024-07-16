use components::{button, TextInput};
use gpui::*;

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

#[derive(Clone)]
pub struct FetchView {
    workspace: Model<Workspace>,
    url_input: View<TextInput>,
    cookie_input: View<TextInput>,
}

impl FetchView {
    pub fn new(workspace: Model<Workspace>, cx: &mut ViewContext<Self>) -> Self {
        Self {
            workspace,
            url_input: cx.new_view(|cx| TextInput::new(cx, "", "Url")),
            cookie_input: cx.new_view(|cx| TextInput::new(cx, "", "Cookie")),
        }
    }
}

impl Render for FetchView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .h_full()
            .w_full()
            .flex()
            .flex_col()
            .child(
                button("router-query")
                    .child("query")
                    .on_click(cx.listener(|this, _, cx| {
                        this.workspace.update(cx, |_data, cx| {
                            cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Query));
                        });
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_lg().child("url"))
                    .child(self.url_input.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_lg().child("cookie"))
                    .child(self.cookie_input.clone()),
            )
    }
}
