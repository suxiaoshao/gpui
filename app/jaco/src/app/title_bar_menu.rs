use crate::foundation::assets::APP_ICON_ASSET_PATH;
use gpui::{
    Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Styled as _,
    img, px,
};
use gpui_component::{h_flex, menu::AppMenuBar};

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
