use components::button;
use gpui::*;

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

#[derive(Clone)]
pub struct QueryView {
    workspace: Model<Workspace>,
}

impl QueryView {
    pub fn new(workspace: Model<Workspace>, _cx: &mut ViewContext<Self>) -> Self {
        Self { workspace }
    }
}

impl Render for QueryView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .h_full()
            .w_full()
            .child(
                button("router-fetch")
                    .child("fetch")
                    .on_click(cx.listener(|this, _, cx| {
                        this.workspace.update(cx, |_data, cx| {
                            cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                        });
                    })),
            )
    }
}
