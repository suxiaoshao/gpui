use components::button;
use gpui::{div, ParentElement, Render, StatefulInteractiveElement, Styled, ViewContext};

use crate::{
    errors::FeiwenResult,
    store::{service::Tag, Db},
};

enum TagDataState {}

pub(crate) struct TagsSelect {
    data: FeiwenResult<Vec<Tag>>,
}

impl TagsSelect {
    pub fn new(cx: &mut ViewContext<Self>) -> Self {
        let tags = Self::get_data(cx);
        Self { data: tags }
    }
    fn get_data(cx: &mut ViewContext<Self>) -> FeiwenResult<Vec<Tag>> {
        let conn = cx.global::<Db>();
        let conn = &mut conn.get()?;
        let tags = Tag::tags(conn)?;
        Ok(tags)
    }
    fn retry(&mut self, cx: &mut ViewContext<Self>) {
        let data = Self::get_data(cx);
        self.data = data;
        cx.notify();
    }
}

impl Render for TagsSelect {
    fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
        let child =
            match &self.data {
                Ok(data) => div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .children(data.iter().map(|item| item.name.to_string())),
                Err(err) => {
                    div()
                        .child(err.to_string())
                        .child(button("retry").child("Retry").on_click(cx.listener(
                            |this, _, cx| {
                                this.retry(cx);
                            },
                        )))
                }
            };
        div().child(child)
    }
}
