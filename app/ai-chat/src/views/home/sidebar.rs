use crate::{
    components::{add_conversation::add_conversation_dialog, add_folder::add_folder_dialog},
    errors::AiChatResult,
    store::{ChatData, ChatDataEvent, ChatDataInner},
    views::settings::OpenSetting,
};
use gpui::*;
use gpui_component::{
    IconName, Side,
    menu::ContextMenuExt,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use std::ops::Deref;
use tracing::{Level, event};

mod conversation_item;
mod folder_item;

actions!(sidebar_view, [Add, AddShift, Delete, Edit]);

const CONTEXT: &str = "sidebar_view";

pub fn init(cx: &mut App) {
    event!(Level::INFO, "init sidebar_view");
    cx.bind_keys([
        KeyBinding::new("secondary-n", Add, None),
        KeyBinding::new("secondary-shift-n", AddShift, None),
        KeyBinding::new("backspace", Delete, None),
    ])
}

pub(crate) struct SidebarView {
    chat_data: WeakEntity<AiChatResult<ChatDataInner>>,
    focus_handle: FocusHandle,
}

impl SidebarView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().downgrade();
        let focus_handle = cx.focus_handle();
        Self {
            chat_data,
            focus_handle,
        }
    }

    fn add_conversation(&mut self, _: &Add, window: &mut Window, cx: &mut Context<Self>) {
        add_conversation_dialog(None, window, cx);
    }
    fn add_folder(&mut self, _: &AddShift, window: &mut Window, cx: &mut Context<Self>) {
        add_folder_dialog(None, window, cx);
    }
}

impl Render for SidebarView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::add_conversation))
            .on_action(cx.listener(Self::add_folder))
            .size_full()
            .child(
                Sidebar::new(Side::Left)
                    .w_full()
                    .header(SidebarHeader::new().child("Ai Chat"))
                    .child(
                        SidebarGroup::new("Conversation Tree").child(
                            SidebarMenu::new().children(
                                match self
                                    .chat_data
                                    .upgrade()
                                    .and_then(|x| x.read(cx).as_ref().ok())
                                {
                                    Some(data) => data.sidebar_items(),
                                    None => vec![],
                                },
                            ),
                        ),
                    )
                    .child(
                        SidebarGroup::new("Actions").child(
                            SidebarMenu::new()
                                .child(
                                    SidebarMenuItem::new("Settings")
                                        .icon(IconName::Settings)
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            window.dispatch_action(OpenSetting.boxed_clone(), cx);
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new("Template List")
                                        .icon(IconName::Bot)
                                        .on_click(cx.listener(|_this, _event, _window, cx| {
                                            let chat_data = cx.global::<ChatData>().deref().clone();
                                            chat_data.update(cx, |_this, cx| {
                                                cx.emit(ChatDataEvent::OpenTemplateList);
                                            });
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new("Add Conversation")
                                        .icon(IconName::Plus)
                                        .on_click(cx.listener(|_this, _evnet, window, cx| {
                                            window.dispatch_action(Add.boxed_clone(), cx);
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new("Add Folder")
                                        .icon(IconName::Plus)
                                        .on_click(cx.listener(|_this, _evnet, window, cx| {
                                            window.dispatch_action(AddShift.boxed_clone(), cx);
                                        })),
                                ),
                        ),
                    ),
            )
            .context_menu(|this, _window, _cx| {
                this.check_side(Side::Left)
                    .external_link_icon(false)
                    .menu_with_icon("Add Conversation", IconName::Plus, Box::new(Add))
                    .menu_with_icon("Add Folder", IconName::Plus, Box::new(AddShift))
            })
    }
}
