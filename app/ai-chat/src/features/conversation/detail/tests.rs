use super::{
    MessageListSyncOperation, RunningTask, first_revision_diff, message_list_sync_operation,
};
use gpui::Task;

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
fn message_list_sync_remeasures_existing_items_instead_of_splicing_them() {
    assert_eq!(
        message_list_sync_operation(2, &[1, 2], &[1, 3]),
        MessageListSyncOperation::Remeasure { range: 1..2 }
    );
    assert_eq!(
        message_list_sync_operation(2, &[1, 2], &[1, 2, 3]),
        MessageListSyncOperation::Splice {
            old_range: 2..2,
            count: 1,
        }
    );
    assert_eq!(
        message_list_sync_operation(1, &[1, 2], &[1, 3]),
        MessageListSyncOperation::Reset { count: 2 }
    );
    assert_eq!(
        message_list_sync_operation(2, &[1, 2], &[1, 2]),
        MessageListSyncOperation::None
    );
}

#[test]
fn first_revision_diff_finds_content_and_length_changes() {
    assert_eq!(first_revision_diff(&[1, 2, 3], &[1, 9, 3]), Some(1));
    assert_eq!(first_revision_diff(&[1, 2], &[1, 2, 3]), Some(2));
    assert_eq!(first_revision_diff(&[1, 2, 3], &[1, 2]), Some(2));
    assert_eq!(first_revision_diff(&[1, 2], &[1, 2]), None);
}
