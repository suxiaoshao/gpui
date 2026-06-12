use std::rc::Rc;

use crate::{
    foundation::{I18n, assets::IconName, conversation_format},
    state::temporary::TemporaryConversationNode,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
    v_flex,
};
use time::OffsetDateTime;

type OnSelect = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement, Clone)]
pub(super) struct TemporaryConversationListItem {
    node: Rc<TemporaryConversationNode>,
    row_ix: usize,
    is_selected: bool,
    on_select: OnSelect,
}

impl TemporaryConversationListItem {
    fn new(node: Rc<TemporaryConversationNode>, row_ix: usize, on_select: OnSelect) -> Self {
        Self {
            node,
            row_ix,
            is_selected: false,
            on_select,
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
        let row_ix = self.row_ix;
        let on_select = self.on_select;
        let updated_at = updated_at_label(node.updated_at, cx.global::<I18n>());

        h_flex()
            .id(format!("temporary-conversation-row-{}", node.id))
            .w_full()
            .min_h(px(52.))
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .cursor_pointer()
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .on_click(move |_, window, cx| {
                on_select(row_ix, window, cx);
            })
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
                    .child(
                        Icon::new(IconName::MessageSquare)
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(Label::new(node.title.clone()).text_sm().truncate())
                    .child(
                        Label::new(updated_at)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
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
            .map(|node| TemporaryConversationListItem::new(node, ix.row, self.on_select.clone()))
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
            (self.on_select)(ix.row, window, cx);
        }
    }
}

fn updated_at_label(updated_at: i128, i18n: &I18n) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(updated_at)
        .map(|time| conversation_format::timestamp_label(time, i18n))
        .unwrap_or_default()
}
