use super::{
    Workspace,
    workspace::{RouterType, WorkspaceEvent},
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use tags_select::TagsSelect;

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
            .size_full()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div().flex().child(
                    Button::new("router-fetch")
                        .primary()
                        .label("fetch")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Fetch));
                            });
                        })),
                ),
            )
            .child(self.tag_select_view.clone())
    }
}
