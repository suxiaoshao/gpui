use crate::{
    errors::AiChatResult,
    store::{ChatData, ChatDataInner},
};
use gpui::*;
use gpui_component::{
    IconName, Side, WindowExt,
    form::field,
    h_flex,
    input::{Input, InputState},
    list::ListItem,
    menu::ContextMenuExt,
    tree::{TreeState, tree},
};

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
    tree_state: Entity<TreeState>,
    folder_input: Entity<InputState>,
}

impl SidebarView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().clone();
        let tree_state = Self::get_tree_state(&chat_data, cx);
        let folder_input = cx.new(|cx| InputState::new(window, cx));
        Self {
            tree_state,
            folder_input,
        }
    }

    fn get_tree_state(chat_data: &ChatData, cx: &mut Context<Self>) -> Entity<TreeState> {
        let data = chat_data.read(cx);
        match data {
            Ok(ChatDataInner {
                conversations,
                folders,
            }) => {
                let mut folder_items: Vec<_> = folders.iter().map(From::from).collect();
                let conversation_items = conversations.iter().map(From::from);
                folder_items.extend(conversation_items);
                cx.new(|cx| TreeState::new(cx).items(folder_items))
            }
            Err(_) => cx.new(|cx| TreeState::new(cx)),
        }
    }
    fn add_conversation(
        &mut self,
        _: &AddConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("Add Conversation")
                .child("This is a dialog dialog.")
        });
    }
    fn add_folder(&mut self, _: &AddFolder, window: &mut Window, cx: &mut Context<Self>) {
        let folder_input = self.folder_input.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            dialog
                .title("Add Folder")
                .child(field().label("Name").child(Input::new(&folder_input)))
        });
    }
}

impl Render for SidebarView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::add_conversation))
            .on_action(cx.listener(Self::add_folder))
            .size_full()
            .child(tree(&self.tree_state, |ix, entry, selected, window, cx| {
                let icon = if !entry.is_folder() {
                    IconName::File
                } else if entry.is_expanded() {
                    IconName::FolderOpen
                } else {
                    IconName::Folder
                };
                ListItem::new(ix).child(
                    h_flex()
                        .gap_2()
                        .child(icon)
                        .child(entry.item().label.clone()),
                )
            }))
            .context_menu(|this, window, cx| {
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
