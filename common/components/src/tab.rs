/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-06-18 06:25:20
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-06-19 09:39:23
 * @FilePath: /gpui-app/common/components/src/tab.rs
 */
use gpui::*;
use prelude::FluentBuilder;
use theme::Theme;

pub trait TabItem {
    type Value: Eq;
    fn label(&self) -> SharedString;
    fn value(&self) -> Self::Value;
}

pub trait TabList {
    type Item: TabItem;
    fn items(&self) -> impl IntoIterator<Item = Self::Item>;
    fn select(&mut self, value: &<Self::Item as TabItem>::Value);
    fn get_select_item(&self) -> &Self::Item;
    fn div(&self, cx: &mut WindowContext) -> Div;
    fn panel(&self) -> impl IntoElement;
}

pub struct Tab<List>
where
    List: TabList,
{
    pub options: List,
}

impl<List> Tab<List>
where
    List: TabList,
{
    pub fn new(options: List) -> Self {
        Self { options }
    }
}

impl<List> Render for Tab<List>
where
    List: TabList<Item: 'static> + 'static,
    <List::Item as TabItem>::Value: 'static,
{
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let divider_color = theme.divider_color();
        let button_color = theme.button_bg_color();
        let items = self.options.items().into_iter();
        let selected_value = self.options.get_select_item().value();
        self.options
            .div(cx)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap_4()
                    .px_2()
                    .cursor_pointer()
                    .flex_row()
                    .children(items.map(|item| {
                        let value = item.value();
                        let is_selected = value == selected_value;
                        let func = cx.listener(move |this, _event, cx| {
                            this.options.select(&value);
                            cx.notify();
                        });
                        let label = item.label();
                        div()
                            .id(label.clone())
                            .child(label)
                            .on_mouse_up(MouseButton::Left, |_event, cx| {
                                cx.prevent_default();
                            })
                            .on_click(move |event, cx| {
                                cx.stop_propagation();
                                func(event, cx);
                            })
                            .when(is_selected, |this| {
                                this.border_b(px(2.0)).border_color(button_color)
                            })
                    })),
            )
            .child(div().bg(divider_color).h(px(1.0)))
            .child(div().flex_1().child(self.options.panel()))
    }
}
