use std::rc::Rc;

use crate::{foundation::assets::IconName, state::temporary::TemporaryConversationNode};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
    v_flex,
};

type OnSelect = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement, Clone)]
pub(super) struct TemporaryConversationListItem {
    node: Rc<TemporaryConversationNode>,
    is_selected: bool,
}

impl TemporaryConversationListItem {
    fn new(node: Rc<TemporaryConversationNode>) -> Self {
        Self {
            node,
            is_selected: false,
        }
    }
}

impl Selectable for TemporaryConversationListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl RenderOnce for TemporaryConversationListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let node = self.node;

        h_flex()
            .id(format!("temporary-conversation-row-{}", node.id))
            .w_full()
            .min_h(px(40.))
            .items_center()
            .px_3()
            .py_2()
            .cursor_pointer()
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(
                Label::new(node.title.clone())
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .truncate(),
            )
    }
}

pub(super) struct TemporaryConversationListDelegate {
    ix: Option<IndexPath>,
    items: Vec<Rc<TemporaryConversationNode>>,
    empty_label: SharedString,
    no_results_label: SharedString,
    error_label: Option<SharedString>,
    has_query: bool,
    on_select: OnSelect,
}

impl TemporaryConversationListDelegate {
    pub(super) fn new(
        items: Vec<TemporaryConversationNode>,
        has_query: bool,
        empty_label: SharedString,
        no_results_label: SharedString,
        error_label: Option<SharedString>,
        on_select: OnSelect,
    ) -> Self {
        Self {
            ix: None,
            items: items.into_iter().map(Rc::new).collect(),
            empty_label,
            no_results_label,
            error_label,
            has_query,
            on_select,
        }
    }
}

impl ListDelegate for TemporaryConversationListDelegate {
    type Item = TemporaryConversationListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.items
            .get(ix.row)
            .cloned()
            .map(TemporaryConversationListItem::new)
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let (icon, label) = if let Some(error) = &self.error_label {
            (IconName::CircleAlert, error.clone())
        } else if self.has_query {
            (IconName::Search, self.no_results_label.clone())
        } else {
            (IconName::SquarePen, self.empty_label.clone())
        };

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .px_4()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(icon).size_5())
            .child(Label::new(label).text_sm().text_center())
            .into_any_element()
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.ix = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        if let Some(ix) = self.ix
            && self.items.get(ix.row).is_some()
        {
            let on_select = self.on_select.clone();
            // `confirm` runs while `ListState` is locked, and selecting a
            // conversation synchronizes the same list from its owner.
            window.defer(cx, move |window, cx| {
                on_select(ix.row, window, cx);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TemporaryConversationListDelegate;
    use crate::state::workspace::SidebarConversationNode;
    use gpui::{
        App, AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, div,
    };
    use gpui_component::{
        IndexPath,
        list::{ListDelegate, ListState},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn test_node() -> SidebarConversationNode {
        SidebarConversationNode {
            id: "conversation".to_string(),
            project_id: String::new(),
            title: "Conversation".into(),
            updated_at: 0,
            pinned: false,
        }
    }

    struct TestRoot;

    impl Render for TestRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn confirm_callback_runs_after_list_update_finishes(cx: &mut TestAppContext) {
        let list_slot = Rc::new(RefCell::new(
            None::<Entity<ListState<TemporaryConversationListDelegate>>>,
        ));
        let callback_ran = Rc::new(Cell::new(false));
        let on_select = Rc::new({
            let list_slot = list_slot.clone();
            let callback_ran = callback_ran.clone();
            move |_index: usize, _window: &mut Window, cx: &mut App| {
                let list = list_slot
                    .borrow()
                    .as_ref()
                    .cloned()
                    .expect("temporary conversation list should be initialized");
                list.update(cx, |_, _| callback_ran.set(true));
            }
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let list = cx.new(|cx| {
                let mut list = ListState::new(
                    TemporaryConversationListDelegate::new(
                        vec![test_node()],
                        false,
                        "Empty".into(),
                        "No results".into(),
                        None,
                        on_select,
                    ),
                    window,
                    cx,
                );
                list.delegate_mut()
                    .set_selected_index(Some(IndexPath::default()), window, cx);
                list
            });
            *list_slot.borrow_mut() = Some(list);
            TestRoot
        });
        let list = list_slot
            .borrow()
            .as_ref()
            .cloned()
            .expect("temporary conversation list should be initialized");

        cx.update(|window, cx| {
            list.update(cx, |list, cx| {
                list.delegate_mut().confirm(false, window, cx);
            });
        });
        cx.run_until_parked();

        assert!(callback_ran.get());
    }
}
