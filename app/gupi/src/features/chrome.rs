use crate::app::menus;
use gpui_kit::component::{
    ActiveTheme, InteractiveElementExt as _, Sizable, TitleBar,
    button::{Button, ButtonVariants},
};
use gpui_kit::*;

pub(crate) const HEIGHT: Pixels = px(46.);

pub(crate) fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id).ghost().small()
}

/// Stop gestures outside the complete control, so popover triggers still receive
/// their child's events before the titlebar's window-move handlers do.
pub(crate) fn control(id: &'static str, child: impl IntoElement) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_double_click(|_, _, cx| cx.stop_propagation())
        .child(child)
        .into_any_element()
}

/// Space for native macOS controls; fullscreen lets AppKit place them itself.
pub(crate) fn leading_space(window: &Window) -> Pixels {
    if cfg!(target_os = "macos") && !window.is_fullscreen() {
        px(88.)
    } else {
        px(12.)
    }
}

pub(crate) fn title_bar(cx: &App) -> TitleBar {
    TitleBar::new()
        .h(HEIGHT)
        .pl_0()
        .border_b_0()
        .bg(cx.theme().background)
        .on_close_window(|_, window, cx| {
            window.dispatch_action(Box::new(menus::Quit), cx);
        })
}

/// A window retains its own menu interaction/focus state; menu definitions come
/// from the same source as the native macOS menus.
pub(crate) fn app_menu_bar(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    #[cfg(not(target_os = "macos"))]
    {
        use gpui_kit::component::menu::AppMenuBar;
        let state = window.use_keyed_state("gupi-app-menu", cx, |_, cx| {
            let menu = AppMenuBar::new(cx);
            let weak = menu.downgrade();
            let subscription = cx.observe_global::<crate::app::menus::MenusChanged>(move |cx| {
                let _ = weak.update(cx, |menu, cx| menu.reload(cx));
            });
            (menu, subscription)
        });
        Some(
            div()
                .h_8()
                .w_full()
                .flex_none()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(state.read(cx).0.clone())
                .into_any_element(),
        )
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (window, cx);
        None
    }
}
