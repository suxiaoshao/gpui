//! Shared input surface. Callers own draft state and business actions.
use gpui_kit::{
    component::{ActiveTheme, h_flex, v_flex},
    *,
};

pub(crate) fn surface(cx: &App) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .pb_2()
        .rounded_2xl()
        .shadow_sm()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

pub(crate) struct Composer {
    input: AnyElement,
    picker: AnyElement,
    leading: Vec<AnyElement>,
    actions: Option<AnyElement>,
}

impl Composer {
    pub fn new(input: impl IntoElement, picker: impl IntoElement) -> Self {
        Self {
            input: input.into_any_element(),
            picker: picker.into_any_element(),
            leading: vec![],
            actions: None,
        }
    }

    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading.push(element.into_any_element());
        self
    }

    pub fn actions(mut self, element: impl IntoElement) -> Self {
        self.actions = Some(element.into_any_element());
        self
    }

    pub fn build(self, cx: &App) -> Div {
        surface(cx).child(self.input).child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .flex_wrap()
                .gap_2()
                .px_2()
                .pt_1()
                .children(self.leading)
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(300.))
                        .justify_end()
                        .items_center()
                        .gap_2()
                        .child(div().min_w_0().max_w(px(340.)).child(self.picker))
                        .children(self.actions),
                ),
        )
    }
}
