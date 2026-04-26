use super::{
    ConversationDraft, WorkspaceState,
    persistence::{PersistedTab, PersistedTabKey},
};
use crate::{
    database::Conversation,
    features::home::ConversationPanelView,
    state::{ChatData, ChatDataInner},
};
use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabKind {
    Conversation(i32),
}

#[derive(Clone)]
pub(super) enum TabPanel {
    Conversation(Entity<ConversationPanelView>),
}

#[derive(Clone)]
pub(super) struct AppTab {
    pub(super) kind: TabKind,
    pub(super) icon: SharedString,
    pub(super) name: SharedString,
    pub(super) panel: TabPanel,
}

impl TabKind {
    pub(super) fn key(self) -> i32 {
        match self {
            TabKind::Conversation(id) => id,
        }
    }

    pub(super) fn persisted_key(self) -> PersistedTabKey {
        match self {
            Self::Conversation(id) => PersistedTabKey::Conversation { id },
        }
    }
}

impl TabPanel {
    pub(super) fn into_any_element(self) -> AnyElement {
        match self {
            Self::Conversation(panel) => panel.into_any_element(),
        }
    }
}

impl WorkspaceState {
    pub(super) fn restore_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let restored = self.persisted.tabs.clone();
        for tab in restored {
            match tab {
                PersistedTab::Conversation { id, draft } => {
                    let _ = self.try_add_restored_conversation_tab(id, draft, window, cx);
                }
                PersistedTab::TemplateList | PersistedTab::TemplateDetail { .. } => {}
            }
        }

        if self.tabs.is_empty()
            && let Some(conversation) = cx
                .global::<ChatData>()
                .read(cx)
                .as_ref()
                .ok()
                .and_then(ChatDataInner::first_conversation)
                .cloned()
        {
            self.push_conversation_tab(conversation, None, window, cx);
        }

        self.active_tab = self
            .persisted
            .active_tab
            .as_ref()
            .and_then(|active| {
                self.tabs
                    .iter()
                    .find(|tab| &tab.kind.persisted_key() == active)
            })
            .map(|tab| tab.kind)
            .or_else(|| self.tabs.first().map(|tab| tab.kind));
        self.sync_persisted_tabs();
    }

    pub(super) fn try_add_restored_conversation_tab(
        &mut self,
        conversation_id: i32,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(conversation) = cx
            .global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .and_then(|data| data.conversation(conversation_id))
            .cloned()
        else {
            return false;
        };
        self.push_conversation_tab(conversation, draft, window, cx);
        true
    }

    fn conversation_tab(
        conversation: Conversation,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut App,
    ) -> AppTab {
        let panel = cx.new(|cx| ConversationPanelView::new(&conversation, window, cx));
        if let Some(draft) = draft.clone() {
            panel.update(cx, |panel, cx| {
                panel.restore_draft(draft, window, cx);
            });
        }
        AppTab {
            kind: TabKind::Conversation(conversation.id),
            icon: SharedString::from(conversation.icon),
            name: SharedString::from(conversation.title),
            panel: TabPanel::Conversation(panel),
        }
    }

    pub(super) fn push_conversation_tab(
        &mut self,
        conversation: Conversation,
        draft: Option<ConversationDraft>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let resolved_draft = self.resolve_initial_conversation_draft(conversation.id, draft);
        self.upsert_persisted_conversation_draft(conversation.id, resolved_draft.clone());
        self.tabs.push(Self::conversation_tab(
            conversation,
            resolved_draft,
            window,
            cx,
        ));
    }
}
