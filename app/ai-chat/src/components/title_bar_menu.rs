use crate::foundation::assets::APP_ICON_ASSET_PATH;
use gpui::{
    App, AppContext as _, ClickEvent, Context, DismissEvent, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, OwnedMenu, OwnedMenuItem,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Subscription,
    Window, actions, anchored, deferred, div, img, prelude::FluentBuilder as _, px,
};
use gpui_component::{ActiveTheme, GlobalState, h_flex, menu::PopupMenu};

actions!(
    title_bar_app_menu,
    [
        CancelTitleBarMenu,
        SelectPreviousTitleBarMenu,
        SelectNextTitleBarMenu
    ]
);

const CONTEXT: &str = "TitleBarAppMenuBar";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CancelTitleBarMenu, Some(CONTEXT)),
        KeyBinding::new("left", SelectPreviousTitleBarMenu, Some(CONTEXT)),
        KeyBinding::new("right", SelectNextTitleBarMenu, Some(CONTEXT)),
    ]);
}

pub(crate) fn title_bar_leading(menu_bar: Entity<TitleBarAppMenuBar>) -> impl IntoElement {
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

pub(crate) struct TitleBarAppMenuBar {
    menus: Vec<Entity<TitleBarAppMenu>>,
    selected_index: Option<usize>,
    action_context: Option<FocusHandle>,
}

impl TitleBarAppMenuBar {
    pub(crate) fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let mut this = Self {
                menus: Vec::new(),
                selected_index: None,
                action_context: None,
            };
            this.reload(cx);
            this
        })
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        let menu_bar = cx.entity();
        let menus: Vec<OwnedMenu> = GlobalState::global(cx).app_menus().to_vec();
        self.menus = menus
            .iter()
            .enumerate()
            .map(|(ix, menu)| TitleBarAppMenu::new(ix, menu, menu_bar.clone(), cx))
            .collect();
        self.selected_index = None;
        self.action_context = None;
        cx.notify();
    }

    fn on_move_left(
        &mut self,
        _: &SelectPreviousTitleBarMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selected_index) = self.selected_index else {
            return;
        };

        let new_ix = if selected_index == 0 {
            self.menus.len().saturating_sub(1)
        } else {
            selected_index.saturating_sub(1)
        };
        self.set_selected_index(Some(new_ix), window, cx);
    }

    fn on_move_right(
        &mut self,
        _: &SelectNextTitleBarMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selected_index) = self.selected_index else {
            return;
        };

        let new_ix = if selected_index + 1 >= self.menus.len() {
            0
        } else {
            selected_index + 1
        };
        self.set_selected_index(Some(new_ix), window, cx);
    }

    fn on_cancel(&mut self, _: &CancelTitleBarMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.set_selected_index(None, window, cx);
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_index.is_none() && ix.is_some() {
            self.action_context = window.focused(cx);
        } else if ix.is_none() {
            if let Some(action_context) = self.action_context.as_ref() {
                action_context.focus(window, cx);
            }
            self.action_context = None;
        }

        self.selected_index = ix;
        cx.notify();
    }

    #[inline]
    fn has_activated_menu(&self) -> bool {
        self.selected_index.is_some()
    }
}

impl Render for TitleBarAppMenuBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("title-bar-app-menu-bar")
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::on_move_left))
            .on_action(cx.listener(Self::on_move_right))
            .on_action(cx.listener(Self::on_cancel))
            .h_full()
            .gap_x_1()
            .overflow_x_scroll()
            .children(self.menus.clone())
    }
}

struct TitleBarAppMenu {
    menu_bar: Entity<TitleBarAppMenuBar>,
    ix: usize,
    name: SharedString,
    menu: OwnedMenu,
    popup_menu: Option<Entity<PopupMenu>>,
    _subscription: Option<Subscription>,
}

impl TitleBarAppMenu {
    fn new(
        ix: usize,
        menu: &OwnedMenu,
        menu_bar: Entity<TitleBarAppMenuBar>,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = menu.name.clone();
        cx.new(|_| Self {
            ix,
            menu_bar,
            name,
            menu: menu.clone(),
            popup_menu: None,
            _subscription: None,
        })
    }

    fn build_popup_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<PopupMenu> {
        let action_context = self.menu_bar.read(cx).action_context.clone();
        let popup_menu = match self.popup_menu.as_ref() {
            None => {
                let items = self.menu.items.clone();
                let popup_menu = PopupMenu::build(window, cx, |menu, window, cx| {
                    let menu = popup_menu_from_owned_items(menu, items, window, cx);
                    if let Some(action_context) = action_context {
                        menu.action_context(action_context)
                    } else {
                        menu
                    }
                });
                self._subscription =
                    Some(cx.subscribe_in(&popup_menu, window, Self::handle_dismiss));
                self.popup_menu = Some(popup_menu.clone());

                popup_menu
            }
            Some(menu) => menu.clone(),
        };

        let focus_handle = popup_menu.read(cx).focus_handle(cx);
        if !focus_handle.contains_focused(window, cx) {
            focus_handle.focus(window, cx);
        }

        popup_menu
    }

    fn handle_dismiss(
        &mut self,
        _: &Entity<PopupMenu>,
        _: &DismissEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._subscription.take();
        self.popup_menu.take();
        self.menu_bar.update(cx, |state, cx| {
            state.on_cancel(&CancelTitleBarMenu, window, cx);
        });
    }

    fn handle_trigger_click(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_selected = self.menu_bar.read(cx).selected_index == Some(self.ix);

        self.menu_bar.update(cx, |state, cx| {
            let new_ix = if is_selected { None } else { Some(self.ix) };
            state.set_selected_index(new_ix, window, cx);
        });
    }

    fn handle_hover(&mut self, hovered: &bool, window: &mut Window, cx: &mut Context<Self>) {
        if !*hovered {
            return;
        }

        let has_activated_menu = self.menu_bar.read(cx).has_activated_menu();
        if !has_activated_menu {
            return;
        }

        self.menu_bar.update(cx, |state, cx| {
            state.set_selected_index(Some(self.ix), window, cx);
        });
    }
}

impl Render for TitleBarAppMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_bar = self.menu_bar.read(cx);
        let is_selected = menu_bar.selected_index == Some(self.ix);

        div()
            .id(self.ix)
            .relative()
            .child(
                title_bar_menu_trigger(self.name.clone(), is_selected, cx)
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(Self::handle_trigger_click)),
            )
            .on_hover(cx.listener(Self::handle_hover))
            .when(is_selected, |this| {
                this.child(deferred(
                    anchored()
                        .anchor(gpui::Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .size_full()
                                .occlude()
                                .top_1()
                                .child(self.build_popup_menu(window, cx)),
                        ),
                ))
            })
    }
}

fn title_bar_menu_trigger(
    label: impl Into<SharedString>,
    is_selected: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("title-bar-menu-trigger")
        .flex()
        .items_center()
        .h(px(24.))
        .px_2()
        .py_0p5()
        .rounded(px(4.))
        .text_sm()
        .line_height(gpui::relative(1.))
        .text_color(cx.theme().foreground)
        .child(label.into())
        .hover(|this| this.bg(cx.theme().secondary_hover))
        .when(is_selected, |this| this.bg(cx.theme().secondary_active))
}

fn popup_menu_from_owned_items(
    mut menu: PopupMenu,
    items: Vec<OwnedMenuItem>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    for item in items {
        match item {
            OwnedMenuItem::Action {
                name,
                action,
                checked,
                disabled,
                ..
            } => {
                menu =
                    menu.menu_with_check_and_disabled(name, checked, action.boxed_clone(), disabled)
            }
            OwnedMenuItem::Separator => {
                menu = menu.separator();
            }
            OwnedMenuItem::Submenu(submenu) => {
                let submenu_items = submenu.items.clone();
                menu = menu.submenu(submenu.name, window, cx, move |menu, window, cx| {
                    popup_menu_from_owned_items(menu, submenu_items.clone(), window, cx)
                });
            }
            OwnedMenuItem::SystemMenu(_) => {}
        }
    }

    menu
}

#[cfg(test)]
pub(crate) fn title_bar_menu_names(menus: &[OwnedMenu]) -> Vec<String> {
    menus.iter().map(|menu| menu.name.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::title_bar_menu_names;
    use crate::{app::menus, foundation::i18n::I18n};

    #[test]
    fn builds_title_bar_menu_names_from_owned_menus() {
        let i18n = I18n::english_for_test();

        assert_eq!(
            title_bar_menu_names(&menus::component_app_menus(&i18n)),
            vec!["AI Chat", "Window"]
        );
    }
}
