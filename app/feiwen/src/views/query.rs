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
    workspace: Entity<Workspace>,
    tag_select_view: Entity<TagsSelect>,
}

impl QueryView {
    pub(crate) fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        Self {
            workspace,
            tag_select_view: cx.new(TagsSelect::new),
        }
    }
}

impl Render for QueryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .w_full()
            .child(
                button("router-fetch")
                    .child("fetch")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.workspace.update(cx, |_data, cx| {
                            cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                        });
                    })),
            )
            .child(self.tag_select_view.clone())
    }
}
