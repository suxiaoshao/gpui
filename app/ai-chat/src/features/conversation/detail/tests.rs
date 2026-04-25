use super::{
    MessageListChange, PreservedScrollOffset, RunningTask, latest_revision_changed, list_is_at_end,
    message_list_change, preserved_tail_item_scroll_offset, should_measure_all_message_list,
    should_reveal_latest_message,
};
use gpui::{ListAlignment, ListState, Task, px};

#[test]
fn running_task_binds_and_matches_messages() {
    let task = Task::ready(());
    let mut running_task = RunningTask::new(task);
    running_task.bind_messages(Some(1usize), Some(2usize));

    assert!(running_task.contains_message(1));
    assert!(running_task.contains_message(2));
    assert!(!running_task.contains_message(3));
    assert_eq!(running_task.message_ids(), [Some(1), Some(2)]);
}

#[test]
fn auto_scrolling_lists_use_full_measurement() {
    assert!(should_measure_all_message_list(true));
    assert!(!should_measure_all_message_list(false));
}

#[test]
fn bottom_aligned_list_is_at_end_only_at_bottom_offset() {
    let state = ListState::new(3, ListAlignment::Bottom, px(100.));
    assert!(list_is_at_end(&state, ListAlignment::Bottom));

    state.scroll_to_reveal_item(0);
    assert!(!list_is_at_end(&state, ListAlignment::Bottom));
    assert!(list_is_at_end(&state, ListAlignment::Top));
}

#[test]
fn latest_revision_change_only_tracks_last_message() {
    assert!(latest_revision_changed(Some(&2), Some(&3)));
    assert!(!latest_revision_changed(Some(&2), Some(&2)));
    assert!(!latest_revision_changed::<i32>(None, None));
}

#[test]
fn message_list_change_tracks_new_items_and_tail_updates() {
    assert_eq!(
        message_list_change(&[1], &[1, 2]),
        MessageListChange {
            item_count_increased: true,
            latest_revision_changed: true,
        }
    );
    assert_eq!(
        message_list_change(&[1, 2], &[9, 2]),
        MessageListChange {
            item_count_increased: false,
            latest_revision_changed: false,
        }
    );
}

#[test]
fn preserves_scroll_offset_for_last_item_only_when_viewport_is_inside_it() {
    let state = ListState::new(2, ListAlignment::Top, px(100.));
    state.scroll_to(gpui::ListOffset {
        item_ix: 1,
        offset_in_item: px(42.),
    });

    assert_eq!(
        preserved_tail_item_scroll_offset(&state, 2, 2, 1),
        Some(PreservedScrollOffset {
            item_ix: 1,
            offset_in_item: px(42.),
        })
    );
    assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 2, 0), None);
    assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 3, 1), None);

    state.scroll_to(gpui::ListOffset {
        item_ix: 1,
        offset_in_item: px(0.),
    });
    assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 2, 1), None);
}

#[test]
fn reveal_latest_message_for_new_message_or_tail_chunk_at_end() {
    assert!(should_reveal_latest_message(
        true,
        false,
        MessageListChange {
            item_count_increased: true,
            latest_revision_changed: true,
        },
        2,
    ));
    assert!(should_reveal_latest_message(
        true,
        true,
        MessageListChange {
            item_count_increased: false,
            latest_revision_changed: true,
        },
        2,
    ));
    assert!(!should_reveal_latest_message(
        true,
        false,
        MessageListChange {
            item_count_increased: false,
            latest_revision_changed: true,
        },
        2,
    ));
    assert!(!should_reveal_latest_message(
        true,
        true,
        MessageListChange::default(),
        2,
    ));
    assert!(!should_reveal_latest_message(
        false,
        true,
        MessageListChange {
            item_count_increased: true,
            latest_revision_changed: true,
        },
        2,
    ));
}
