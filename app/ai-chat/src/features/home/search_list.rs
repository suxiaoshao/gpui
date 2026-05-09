use gpui::{App, Context, ScrollStrategy, Window};
use gpui_component::{
    IndexPath,
    list::{ListDelegate, ListState},
};

pub(crate) fn select_first_if_any<D>(
    state: &mut ListState<D>,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let first = IndexPath::default();
    let has_items = state.delegate().items_count(0, cx) > 0;
    state.set_selected_index(has_items.then_some(first), window, cx);
    if has_items {
        state.scroll_to_item(first, ScrollStrategy::Top, window, cx);
    }
}

pub(crate) fn move_selected<D>(
    state: &mut ListState<D>,
    delta: isize,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let count = state.delegate().items_count(0, cx);
    if count == 0 {
        state.set_selected_index(None, window, cx);
        return;
    }

    let current = state.selected_index().map(|ix| ix.row).unwrap_or(0);
    let next = if delta < 0 {
        if current == 0 { count - 1 } else { current - 1 }
    } else if current + 1 >= count {
        0
    } else {
        current + 1
    };
    let next_ix = IndexPath::default().row(next);
    state.set_selected_index(Some(next_ix), window, cx);
    state.scroll_to_item(next_ix, ScrollStrategy::Top, window, cx);
}

pub(crate) fn confirm_selected<D>(
    state: &mut ListState<D>,
    secondary: bool,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let selected = state.selected_index();
    state
        .delegate_mut()
        .set_selected_index(selected, window, cx);
    state.delegate_mut().confirm(secondary, window, cx);
}

pub(crate) fn item_count<D>(state: &ListState<D>, cx: &App) -> usize
where
    D: ListDelegate + 'static,
{
    state.delegate().items_count(0, cx)
}
