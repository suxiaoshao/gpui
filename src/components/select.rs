use std::ops::Deref;

use gpui::*;
use ui::{FluentBuilder, StyledExt};

use super::Button;

pub trait SelectItem {
    type Value: Eq;
    fn value(&self) -> Self::Value;
    fn display(&self) -> impl IntoElement;
    fn id(&self) -> ElementId;
    fn label(&self) -> String;
}

pub trait SelectList {
    type Item: SelectItem;
    type Value: Eq;
    fn items(&self) -> impl IntoIterator<Item = Self::Item>;
    fn select(&mut self, value: &<Self::Item as SelectItem>::Value);
    fn default(&self) -> Self::Item;
    fn get_select_item(&self) -> &Self::Item;
    fn get_select_value(&self) -> <Self::Item as SelectItem>::Value {
        self.get_select_item().value()
    }
}

#[derive(Debug, Clone)]
pub struct Select<List>
where
    List: SelectList,
{
    pub options: List,
    pub focus_handle: FocusHandle,
    pub open: bool,
}

impl<List> Select<List>
where
    List: SelectList,
{
    pub fn new(options: List, cx: &mut ViewContext<Self>) -> Self {
        Self {
            options,
            focus_handle: cx.focus_handle(),
            open: false,
        }
    }
    fn select(&mut self, value: &<List::Item as SelectItem>::Value) {
        self.options.select(value);
    }
}

impl<List> Render for Select<List>
where
    List: SelectList + 'static,
    List::Item: 'static,
    <List::Item as SelectItem>::Value: 'static,
{
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let options = self.options.items().into_iter();
        let select_value = self.options.get_select_item();
        div()
            .child(
                Button::new(select_value.label(), select_value.id()).on_click(cx.listener(
                    |this, _, cx| {
                        this.focus_handle.focus(cx);
                    },
                )),
            )
            .when(self.focus_handle.is_focused(cx), |x| {
                x.child(
                    div()
                        .track_focus(&self.focus_handle)
                        .absolute()
                        .elevation_3(cx)
                        .max_h(px(200.0))
                        .children(options.into_iter().map(|data| {
                            let value = data.value();
                            let on_click = cx.listener(move |this, _, cx| {
                                this.select(&value);
                                cx.notify();
                            });
                            SelectItemElement::new(data, on_click)
                        })),
                )
            })
    }
}

type OnClick = Box<dyn Fn(&ClickEvent, &mut WindowContext) + 'static>;

struct SelectItemElement<T: SelectItem> {
    data: T,
    on_click: OnClick,
}

impl<T: SelectItem> SelectItemElement<T> {
    fn new(data: T, on_click: impl Fn(&ClickEvent, &mut WindowContext) + 'static) -> Self {
        Self {
            data,
            on_click: Box::new(on_click),
        }
    }
}

impl<T: SelectItem> Deref for SelectItemElement<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: SelectItem + 'static> IntoElement for SelectItemElement<T> {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

impl<T: SelectItem + 'static> RenderOnce for SelectItemElement<T> {
    fn render(self, _cx: &mut WindowContext) -> impl IntoElement {
        div()
            .id(self.id())
            .child(self.display())
            .on_mouse_up(MouseButton::Left, |event, cx| {
                dbg!(event);
                cx.prevent_default();
            })
            .on_click(move |event, cx| {
                cx.stop_propagation();
                dbg!(event);
                (self.on_click)(event, cx)
            })
    }
}
