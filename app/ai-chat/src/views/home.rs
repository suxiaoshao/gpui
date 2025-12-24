use crate::{
    errors::AiChatResult,
    store::{Conversation, Db, Folder},
    views::home::sidebar::SidebarView,
};
use gpui::*;
use gpui_component::{
    Root, TitleBar,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

mod sidebar;

pub fn init(cx: &mut App) {
    sidebar::init(cx);
}

struct ChatData {
    conversations: Vec<Conversation>,
    folders: Vec<Folder>,
}

impl ChatData {
    fn new(cx: &mut Context<AiChatResult<Self>>) -> AiChatResult<Self> {
        let conn = &mut cx.global::<Db>().get()?;
        let conversations = Conversation::query_without_folder(conn)?;
        let folders = Folder::query(conn)?;
        Ok(Self {
            conversations,
            folders,
        })
    }
}

pub(crate) struct HomeView {
    sidebar: Entity<SidebarView>,
    chat_data: Entity<AiChatResult<ChatData>>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_data = cx.new(|cx| ChatData::new(cx));
        let sidebar = cx.new(|cx| SidebarView::new(chat_data.clone(), window, cx));
        Self { sidebar, chat_data }
    }
}

impl Render for HomeView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        v_flex()
            .size_full()
            .child(TitleBar::new())
            .child(
                h_resizable("vertical-layout")
                    .child(resizable_panel().size(px(300.)).child(self.sidebar.clone()))
                    .child(div().child("Bottom Panel").into_any_element()),
            )
            .children(dialog_layer)
    }
}
