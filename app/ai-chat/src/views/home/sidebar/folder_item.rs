use super::{Add, AddShift, conversation_item};
use crate::{
    components::{
        add_conversation::add_conversation_dialog, add_folder::add_folder_dialog,
        delete_confirm::open_delete_confirm_dialog,
    },
    database::Folder,
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

pub(super) fn sidebar_item(folder: &Folder) -> SidebarMenuItem {
    let id = folder.id;
    let parent_id = Some(folder.id);
    let name = folder.name.clone();
    let children = folder.folders.iter().map(sidebar_item).chain(
        folder
            .conversations
            .iter()
            .map(conversation_item::sidebar_item),
    );

    SidebarMenuItem::new(&folder.name)
        .icon(IconName::Folder)
        .click_to_open(true)
        .children(children)
        .suffix(
            div()
                .on_action(move |_: &AddShift, window, cx| {
                    add_folder_dialog(parent_id, window, cx);
                })
                .on_action(move |_: &Add, window, cx| {
                    add_conversation_dialog(parent_id, window, cx);
                })
                .on_action(move |_: &Delete, window, cx| {
                    let chat_data = cx.global::<ChatData>().deref().clone();
                    open_delete_confirm_dialog(
                        "Delete Folder",
                        SharedString::from(format!(
                            "Delete folder \"{name}\" and its contents? This action cannot be undone."
                        )),
                        move |_window, cx| {
                            chat_data.update(cx, move |_this, cx| {
                                cx.emit(ChatDataEvent::DeleteFolder(id));
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
                            this.check_side(gpui_component::Side::Left)
                                .menu_with_icon("Add Conversation", IconName::Plus, Box::new(Add))
                                .menu_with_icon(
                                    "Add Folder",
                                    IconName::Plus,
                                    Box::new(AddShift),
                                )
                                .menu_with_icon("Delete", IconName::Delete, Box::new(Delete))
                        }),
                ),
        )
}
