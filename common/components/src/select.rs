use gpui::{prelude::*, *};
use std::ops::Deref;
use theme::Theme;

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
    fn select(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        value: &<Self::Item as SelectItem>::Value,
    );
    fn get_select_item(&self, window: &mut Window, cx: &mut App) -> Self::Item;
    fn trigger_element(
        &self,
        window: &mut Window,
        cx: &mut App,
        func: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
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
    pub fn new(options: List, cx: &mut Context<Self>) -> Self {
        Self {
            options,
            menu_focus_handle: cx.focus_handle(),
            button_focus_handle: cx.focus_handle(),
        }
    }
    fn select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        value: &<List::Item as SelectItem>::Value,
    ) {
        self.options.select(window, cx, value);
    }
}

impl<List> Render for Select<List>
where
    List: SelectList<Item: 'static> + 'static,
    <List::Item as SelectItem>::Value: 'static,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let options = self.options.items().into_iter();
        let func = cx.listener(|this, _event, window, cx| {
            match this.menu_focus_handle.is_focused(window) {
                true => this.button_focus_handle.focus(window),
                false => this.menu_focus_handle.focus(window),
            };
            cx.notify();
        });
        let theme = cx.global::<Theme>();
        let bg = theme.bg_color();
        let trigger_element = self.options.trigger_element(window, cx, func);
        div()
            .child(trigger_element)
            .when(self.menu_focus_handle.is_focused(window), |x| {
                x.child(deferred(
                    div()
                        .whitespace_nowrap()
                        .bg(bg)
                        .track_focus(&self.menu_focus_handle)
                        .absolute()
                        .max_h(px(200.0))
                        .children(options.into_iter().map(|data| {
                            let value = data.value();
                            let on_click = cx.listener(move |this, _, window, cx| {
                                this.select(window, cx, &value);
                                this.button_focus_handle.focus(window);
                                cx.notify();
                            });
                            SelectItemElement::new(data, on_click)
                        })),
                ))
            })
    }
}

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

struct SelectItemElement<T: SelectItem> {
    data: T,
    on_click: OnClick,
}

impl<T: SelectItem> SelectItemElement<T> {
    fn new(data: T, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
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
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id(self.id())
            .child(self.display_item())
            .on_mouse_up(MouseButton::Left, |_event, window, _cx| {
                window.prevent_default();
            })
            .cursor_pointer()
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                (self.on_click)(event, window, cx)
            })
    }
}
