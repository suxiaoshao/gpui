use crate::{
    components::delete_confirm::open_delete_confirm_dialog,
    database::Conversation,
    store::{ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
    input::Delete,
    menu::DropdownMenu,
    sidebar::SidebarMenuItem,
};
use std::ops::Deref;

pub(super) fn sidebar_item(conversation: &Conversation) -> SidebarMenuItem {
    let id = conversation.id;
    let title = conversation.title.clone();
    SidebarMenuItem::new(SharedString::from(format!(
        "{} {}",
        conversation.icon, conversation.title
    )))
    .on_click(move |_this, _window, cx| {
        let chat_data = cx.global::<ChatData>().deref().clone();
        chat_data.update(cx, move |_this, cx| {
            cx.emit(ChatDataEvent::AddTab(id));
        });
    })
    .suffix(
        div()
            .on_action(move |_: &Delete, window, cx| {
                let chat_data = cx.global::<ChatData>().deref().clone();
                open_delete_confirm_dialog(
                    "Delete Conversation",
                    SharedString::from(format!(
                        "Delete conversation \"{title}\"? This action cannot be undone."
                    )),
                    move |_window, cx| {
                        chat_data.update(cx, move |_this, cx| {
                            cx.emit(ChatDataEvent::DeleteConversation(id));
                        });
                    },
                    window,
                    cx,
                );
            })
            .child(
                Button::new(id)
                    .icon(IconName::EllipsisVertical)
                    .ghost()
                    .xsmall()
                    .dropdown_menu(|this, _window, _cx| {
                        this.check_side(gpui_component::Side::Left).menu_with_icon(
                            "Delete",
                            IconName::Delete,
                            Box::new(Delete),
                        )
                    }),
            ),
    )
}
