use components::button;
use gpui::*;
use tags_select::TagsSelect;

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

mod tags_select;

#[derive(Clone)]
pub(crate) struct QueryView {
    workspace: Model<Workspace>,
    tag_select_view: View<TagsSelect>,
}

impl QueryView {
    pub(crate) fn new(workspace: Model<Workspace>, cx: &mut ViewContext<Self>) -> Self {
        Self {
            workspace,
            tag_select_view: cx.new_view(TagsSelect::new),
        }
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
            .child(self.tag_select_view.clone())
    }
}
