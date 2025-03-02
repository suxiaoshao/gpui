use std::collections::HashMap;

use components::button;
use gpui::{
    div, px, Context, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Window,
};
use theme::Theme;

use crate::{
    errors::FeiwenResult,
    store::{
        service::{Tag, TagWithId},
        Db,
    },
};

pub(crate) struct TagsSelect {
    data: FeiwenResult<Vec<TagWithId>>,
    selected: HashMap<i32, TagWithId>,
}

impl TagsSelect {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tags = Self::get_data(cx);
        Self {
            data: tags,
            selected: HashMap::new(),
        }
    }
    fn get_data(cx: &mut Context<Self>) -> FeiwenResult<Vec<TagWithId>> {
        let conn = cx.global::<Db>();
        let conn = &mut conn.get()?;
        let tags = Tag::tags_with_id(conn)?;
        Ok(tags)
    }
    fn retry(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let data = Self::get_data(cx);
        self.data = data;
        cx.notify();
    }
}

impl Render for TagsSelect {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let theme = cx.global::<Theme>();
        let child = match &self.data {
            Ok(data) => div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(self.selected.iter().map(|item| {
                    let id = *item.0;
                    button(SharedString::from(format!("select-tag-{}", item.0)))
                        .flex()
                        .justify_center()
                        .child(item.1.name.to_string())
                        .border_1()
                        .border_color(theme.button_bg_color())
                        .rounded_lg()
                        .p_1()
                        .min_w(px(50.0))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected.remove(&id);
                            cx.notify();
                        }))
                }))
                .children(
                    data.iter()
                        .filter(|item| !self.selected.contains_key(&item.id))
                        .take(100)
                        .map(|item| {
                            let tag = TagWithId {
                                name: item.name.clone(),
                                id: item.id,
                            };
                            button(SharedString::from(format!("select-tag-{}", item.id)))
                                .flex()
                                .justify_center()
                                .child(item.name.to_string())
                                .border_1()
                                .border_color(theme.text_color())
                                .rounded_lg()
                                .p_1()
                                .min_w(px(50.0))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected.insert(tag.id, tag.clone());
                                    cx.notify();
                                }))
                        }),
                ),
            Err(err) => {
                div()
                    .child(err.to_string())
                    .child(button("retry").child("Retry").on_click(cx.listener(
                        |this, _, window, cx| {
                            this.retry(window, cx);
                        },
                    )))
            }
        };
        div().child(child)
    }
}
