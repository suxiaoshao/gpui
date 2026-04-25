use crate::{
    errors::AiChatResult,
    features::settings::OpenSetting,
    foundation::assets::IconName,
    foundation::i18n::I18n,
    state::{ChatData, ChatDataInner, WorkspaceState, WorkspaceStore},
};
use gpui::*;
use gpui_component::{
    Collapsible, Side,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use std::ops::Deref;
use tracing::{Level, event};

use super::{AddConversation, AddFolder, search::OpenConversationSearch};

mod conversation_item;
mod conversation_tree;
mod folder_item;
pub(crate) use conversation_tree::DragConversationTreeItem;

const CONTEXT: &str = "sidebar_view";

#[derive(Clone)]
enum SidebarSection {
    Tree(SidebarGroup<conversation_tree::ConversationTree>),
    Menu(SidebarGroup<SidebarMenu>),
}

impl Collapsible for SidebarSection {
    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Tree(group) => Self::Tree(group.collapsed(collapsed)),
            Self::Menu(group) => Self::Menu(group.collapsed(collapsed)),
        }
    }

    fn is_collapsed(&self) -> bool {
        match self {
            Self::Tree(group) => group.is_collapsed(),
            Self::Menu(group) => group.is_collapsed(),
        }
    }
}

impl SidebarItem for SidebarSection {
    fn render(
        self,
        _id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Tree(group) => group.render("sidebar-tree", window, cx).into_any_element(),
            Self::Menu(group) => group.render("sidebar-menu", window, cx).into_any_element(),
        }
    }
}

pub fn init(_cx: &mut App) {
    event!(Level::INFO, "init sidebar_view");
}

pub(crate) struct SidebarView {
    chat_data: WeakEntity<AiChatResult<ChatDataInner>>,
    workspace: WeakEntity<WorkspaceState>,
    focus_handle: FocusHandle,
}

impl SidebarView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_data = cx.global::<ChatData>().downgrade();
        let workspace = cx.global::<WorkspaceStore>().deref().downgrade();
        let focus_handle = cx.focus_handle();
        Self {
            chat_data,
            workspace,
            focus_handle,
        }
    }
}

impl Render for SidebarView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let (
            app_title,
            conversation_tree_title,
            actions_title,
            settings_label,
            search_label,
            template_list_label,
            add_conversation_label,
            add_folder_label,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("sidebar-app-title"),
                i18n.t("sidebar-conversation-tree"),
                i18n.t("sidebar-actions"),
                i18n.t("sidebar-settings"),
                i18n.t("sidebar-search-conversation"),
                i18n.t("sidebar-template-list"),
                i18n.t("sidebar-add-conversation"),
                i18n.t("sidebar-add-folder"),
            )
        };
        let root_label = cx.global::<I18n>().t("sidebar-root");
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .child(
                Sidebar::new("sidebar")
                    .side(Side::Left)
                    .w_full()
                    .border_r_0()
                    .collapsible(false)
                    .collapsed(false)
                    .header(SidebarHeader::new().child(app_title))
                    .child(SidebarSection::Tree(
                        SidebarGroup::new(conversation_tree_title).child(
                            self.chat_data
                                .upgrade()
                                .and_then(|x| x.read(cx).as_ref().ok())
                                .map(|data| {
                                    let active_conversation_id = self
                                        .workspace
                                        .upgrade()
                                        .and_then(|workspace| workspace.read(cx).active_tab_key())
                                        .filter(|id| *id > 0);
                                    let open_folder_ids = self
                                        .workspace
                                        .upgrade()
                                        .map(|workspace| workspace.read(cx).open_folder_ids())
                                        .unwrap_or_default();
                                    conversation_tree::ConversationTree::new(
                                        data,
                                        active_conversation_id,
                                        open_folder_ids,
                                        root_label.clone().into(),
                                    )
                                })
                                .unwrap_or_else(|| {
                                    conversation_tree::ConversationTree::empty_with_label(
                                        root_label.clone().into(),
                                    )
                                }),
                        ),
                    ))
                    .child(SidebarSection::Menu(
                        SidebarGroup::new(actions_title).child(
                            SidebarMenu::new()
                                .child(
                                    SidebarMenuItem::new(settings_label)
                                        .icon(IconName::Settings)
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            window.dispatch_action(OpenSetting.boxed_clone(), cx);
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new(search_label)
                                        .icon(IconName::Search)
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            window.dispatch_action(
                                                OpenConversationSearch.boxed_clone(),
                                                cx,
                                            );
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new(template_list_label)
                                        .icon(IconName::LayoutTemplate)
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            cx.global::<WorkspaceStore>().deref().clone().update(
                                                cx,
                                                |workspace, cx| {
                                                    workspace.open_template_list_tab(window, cx);
                                                },
                                            );
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new(add_conversation_label.clone())
                                        .icon(IconName::Plus)
                                        .on_click(cx.listener(|_this, _evnet, window, cx| {
                                            window
                                                .dispatch_action(AddConversation.boxed_clone(), cx);
                                        })),
                                )
                                .child(
                                    SidebarMenuItem::new(add_folder_label.clone())
                                        .icon(IconName::Plus)
                                        .on_click(cx.listener(|_this, _evnet, window, cx| {
                                            window.dispatch_action(AddFolder.boxed_clone(), cx);
                                        })),
                                ),
                        ),
                    )),
            )
    }
}
