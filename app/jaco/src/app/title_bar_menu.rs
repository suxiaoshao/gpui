use crate::foundation::assets::APP_ICON_ASSET_PATH;
use gpui_kit::component::{h_flex, menu::AppMenuBar};
use gpui_kit::{
    Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement, Styled, img, px,
};

pub(crate) fn title_bar_leading(menu_bar: Entity<AppMenuBar>) -> impl IntoElement {
    h_flex()
        .items_center()
        .h_full()
        .flex_none()
        .gap_1()
        .pr_2()
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .child(
            img(APP_ICON_ASSET_PATH)
                .size(px(16.))
                .flex_none()
                .rounded(px(3.)),
        )
        .child(menu_bar)
}
