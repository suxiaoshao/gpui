use crate::{
    errors::FeiwenResult,
    i18n::I18n,
    store::{
        Db,
        service::{Tag, TagWithId},
    },
};
use gpui::{Context, ParentElement, Render, SharedString, Styled, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use std::collections::HashMap;

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
    pub fn get_selected(&self) -> Vec<TagWithId> {
        self.selected.values().cloned().collect()
    }
}

impl Render for TagsSelect {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let retry_label = cx.global::<I18n>().t("tags-select-retry");
        let child = match &self.data {
            Ok(data) => div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(self.selected.iter().map(|item| {
                    let id = *item.0;
                    Button::new(SharedString::from(format!("select-tag-{}", item.0)))
                        .info()
                        .flex()
                        .justify_center()
                        .label(item.1.name.clone())
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
                            Button::new(SharedString::from(format!("select-tag-{}", item.id)))
                                .primary()
                                .flex()
                                .justify_center()
                                .label(item.name.to_string())
                                .min_w(px(50.0))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected.insert(tag.id, tag.clone());
                                    cx.notify();
                                }))
                        }),
                ),
            Err(err) => div().child(err.to_string()).child(
                Button::new("retry")
                    .label(retry_label)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.retry(window, cx);
                    })),
            ),
        };
        div().flex_initial().child(child)
    }
}
