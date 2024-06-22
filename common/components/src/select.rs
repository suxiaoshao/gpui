use gpui::{prelude::*, *};
use std::ops::Deref;

pub trait SelectItem {
    type Value: Eq;
    fn value(&self) -> Self::Value;
    fn display_item(&self) -> impl IntoElement;
    fn id(&self) -> ElementId;
    fn label(&self) -> String;
}

pub trait SelectList {
    type Item: SelectItem;
    type Value: Eq;
    fn items(&self) -> impl IntoIterator<Item = Self::Item>;
    fn select(&mut self, cx: &mut WindowContext, value: &<Self::Item as SelectItem>::Value);
    fn get_select_item(&self, cx: &mut WindowContext) -> Self::Item;
    fn trigger_element(
        &self,
        cx: &mut WindowContext,
        func: impl Fn(&ClickEvent, &mut WindowContext) + 'static,
    ) -> impl IntoElement;
}

#[derive(Debug, Clone)]
pub struct Select<List>
where
    List: SelectList,
{
    pub options: List,
    pub menu_focus_handle: FocusHandle,
    pub button_focus_handle: FocusHandle,
}

impl<List> Select<List>
where
    List: SelectList,
{
    pub fn new(options: List, cx: &mut ViewContext<Self>) -> Self {
        Self {
            options,
            menu_focus_handle: cx.focus_handle(),
            button_focus_handle: cx.focus_handle(),
        }
    }
    fn select(&mut self, cx: &mut WindowContext, value: &<List::Item as SelectItem>::Value) {
        self.options.select(cx, value);
    }
}

impl<List> Render for Select<List>
where
    List: SelectList<Item: 'static> + 'static,
    <List::Item as SelectItem>::Value: 'static,
{
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let options = self.options.items().into_iter();
        let func = cx.listener(|this, _event, cx| {
            match this.menu_focus_handle.is_focused(cx) {
                true => this.button_focus_handle.focus(cx),
                false => this.menu_focus_handle.focus(cx),
            };
            cx.notify();
        });
        let trigger_element = self.options.trigger_element(cx, func);
        div()
            .child(trigger_element)
            .when(self.menu_focus_handle.is_focused(cx), |x| {
                x.child(
                    div()
                        .whitespace_nowrap()
                        .track_focus(&self.menu_focus_handle)
                        .absolute()
                        .max_h(px(200.0))
                        .children(options.into_iter().map(|data| {
                            let value = data.value();
                            let on_click = cx.listener(move |this, _, cx| {
                                this.select(cx, &value);
                                this.button_focus_handle.focus(cx);
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
            .child(self.display_item())
            .on_mouse_up(MouseButton::Left, |_event, cx| {
                cx.prevent_default();
            })
            .cursor_pointer()
            .on_click(move |event, cx| {
                cx.stop_propagation();
                (self.on_click)(event, cx)
            })
    }
}
