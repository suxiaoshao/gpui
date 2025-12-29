use crate::{
    errors::AiChatResult,
    store::{ChatData, ChatDataEvent, ChatDataInner},
};
use gpui::*;
use gpui_component::{
    IconName, Side, WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
    menu::ContextMenuExt,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu},
    v_flex,
};
use std::ops::Deref;
use tracing::{Level, event};

mod conversation_item;
mod folder_item;

actions!(
    sidebar_view,
    [
        AddConversation,
        AddFolder,
        DeleteConversation,
        EditConversation
    ]
);

const CONTEXT: &str = "sidebar_view";

pub fn init(cx: &mut App) {
    event!(Level::INFO, "init sidebar_view");
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-n", AddConversation, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-n", AddConversation, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-n", AddFolder, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("cmd-shift-n", AddFolder, Some(CONTEXT)),
    ])
}

pub(crate) struct SidebarView {
    chat_data: Entity<AiChatResult<ChatDataInner>>,
    focus_handle: FocusHandle,
}

impl SidebarView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().deref().clone();
        let focus_handle = cx.focus_handle();
        Self {
            chat_data,
            focus_handle,
        }
    }

    fn add_conversation(
        &mut self,
        _: &AddConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let span = tracing::info_span!("add_conversation action");
        let _enter = span.enter();
        event!(Level::INFO, "add_conversation action");
        let name_input = cx.new(|cx| InputState::new(window, cx));
        let info_input = cx.new(|cx| InputState::new(window, cx));
        let template_input = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("Add Conversation")
                .child("This is a dialog dialog.")
        });
    }
    fn add_folder(&mut self, _: &AddFolder, window: &mut Window, cx: &mut Context<Self>) {
        let span = tracing::info_span!("add_folder action");
        let _enter = span.enter();
        event!(Level::INFO, "add_folder action");
        let folder_input = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("Add Folder")
                .child(v_form().child(field().label("Name").child(Input::new(&folder_input))))
                .footer({
                    let folder_input = folder_input.clone();
                    move |_this, _state, _window, _cx| {
                        vec![
                            Button::new("ok").primary().label("Submit").on_click({
                                let folder_input = folder_input.clone();
                                move |_, window, cx| {
                                    let name = folder_input.read(cx).value().to_string();
                                    if !name.is_empty() {
                                        let chat_data = cx.global::<ChatData>().deref().clone();
                                        chat_data.update(cx, |_this, cx| {
                                            cx.emit(ChatDataEvent::AddFolder {
                                                name,
                                                parent_id: None,
                                            });
                                        });
                                    }
                                    window.close_dialog(cx);
                                }
                            }),
                            Button::new("cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        ]
                    }
                })
        });
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
                    .child(SidebarGroup::new("Conversation Tree").child(
                        SidebarMenu::new().children(match self.chat_data.read(cx) {
                            Ok(data) => data.sidebar_items(),
                            Err(_) => vec![],
                        }),
                    )),
            )
            .context_menu(|this, _window, _cx| {
                this.check_side(Side::Left)
                    .external_link_icon(false)
                    .menu_with_icon(
                        "Add Conversation",
                        IconName::Plus,
                        Box::new(AddConversation),
                    )
                    .menu_with_icon("Add Folder", IconName::Plus, Box::new(AddFolder))
            })
    }
}
