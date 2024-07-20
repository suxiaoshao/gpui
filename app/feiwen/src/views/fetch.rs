use components::{button, input_border, IntInput, TextInput};
use gpui::*;
use theme::Theme;

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

#[derive(Clone)]
pub struct FetchView {
    workspace: Model<Workspace>,
    url_input: View<TextInput>,
    start_page: View<IntInput>,
    end_page: View<IntInput>,
    cookie_input: View<TextInput>,
}

impl FetchView {
    pub fn new(workspace: Model<Workspace>, cx: &mut ViewContext<Self>) -> Self {
        Self {
            workspace,
            url_input: cx.new_view(|cx| TextInput::new(cx, "", "Url")),
            start_page: cx.new_view(|cx| IntInput::new(cx, 0, "Start Page")),
            end_page: cx.new_view(|cx| IntInput::new(cx, 0, "End Page")),
            cookie_input: cx.new_view(|cx| TextInput::new(cx, "", "Cookie")),
        }
    }
}

impl Render for FetchView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
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
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_1()
                    .gap_1()
                    .child(input_border(theme).child(self.url_input.clone()))
                    .child(input_border(theme).child(self.start_page.clone()))
                    .child(input_border(theme).child(self.end_page.clone()))
                    .child(input_border(theme).child(self.cookie_input.clone())),
            )
    }
}
