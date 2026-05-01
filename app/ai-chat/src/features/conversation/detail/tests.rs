use super::{
    InitialMessageReveal, MessageListSyncOperation, MessageRevisionExt, RunningTask,
    first_revision_diff, message_list_sync_operation,
};
use gpui::Task;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TestRevision {
    id: usize,
    content_version: usize,
}

impl MessageRevisionExt for TestRevision {
    type Id = usize;

    fn message_id(&self) -> Self::Id {
        self.id
    }
}

fn revision(id: usize, content_version: usize) -> TestRevision {
    TestRevision {
        id,
        content_version,
    }
}

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
        message_list_sync_operation(
            2,
            &[revision(1, 1), revision(2, 1)],
            &[revision(1, 1), revision(2, 2)]
        ),
        MessageListSyncOperation::Remeasure { range: 1..2 }
    );
    assert_eq!(
        message_list_sync_operation(
            2,
            &[revision(1, 1), revision(2, 1)],
            &[revision(1, 1), revision(2, 1), revision(3, 1)]
        ),
        MessageListSyncOperation::Splice {
            old_range: 2..2,
            count: 1,
        }
    );
    assert_eq!(
        message_list_sync_operation(
            1,
            &[revision(1, 1), revision(2, 1)],
            &[revision(1, 1), revision(2, 2)]
        ),
        MessageListSyncOperation::Reset { count: 2 }
    );
    assert_eq!(
        message_list_sync_operation(
            2,
            &[revision(1, 1), revision(2, 1)],
            &[revision(1, 1), revision(2, 1)]
        ),
        MessageListSyncOperation::None
    );
}

#[test]
fn message_list_sync_splices_equal_length_identity_changes() {
    assert_eq!(
        message_list_sync_operation(
            2,
            &[revision(1, 1), revision(2, 1)],
            &[revision(1, 1), revision(3, 1)]
        ),
        MessageListSyncOperation::Splice {
            old_range: 1..2,
            count: 1,
        }
    );
    assert_eq!(
        message_list_sync_operation(
            3,
            &[revision(1, 1), revision(2, 1), revision(3, 1)],
            &[revision(1, 1), revision(3, 1), revision(2, 1)]
        ),
        MessageListSyncOperation::Splice {
            old_range: 1..3,
            count: 2,
        }
    );
}

#[test]
fn message_list_sync_splices_from_first_diff_when_identity_change_follows_content_change() {
    assert_eq!(
        message_list_sync_operation(
            3,
            &[revision(1, 1), revision(2, 1), revision(3, 1)],
            &[revision(1, 2), revision(2, 1), revision(4, 1)]
        ),
        MessageListSyncOperation::Splice {
            old_range: 0..3,
            count: 3,
        }
    );
}

#[test]
fn first_revision_diff_finds_content_and_length_changes() {
    assert_eq!(first_revision_diff(&[1, 2, 3], &[1, 9, 3]), Some(1));
    assert_eq!(first_revision_diff(&[1, 2], &[1, 2, 3]), Some(2));
    assert_eq!(first_revision_diff(&[1, 2, 3], &[1, 2]), Some(2));
    assert_eq!(first_revision_diff(&[1, 2], &[1, 2]), None);
}

#[test]
fn initial_message_reveal_waits_for_first_non_empty_message_list() {
    let mut reveal = InitialMessageReveal::new(true);

    assert!(!reveal.take_if_ready(0));
    assert!(reveal.take_if_ready(1));
    assert!(!reveal.take_if_ready(1));
}

#[test]
fn initial_message_reveal_is_rearmed_only_by_reset() {
    let mut reveal = InitialMessageReveal::new(true);
    assert!(reveal.take_if_ready(2));

    reveal.record_sync_operation(&MessageListSyncOperation::Remeasure { range: 1..2 });
    assert!(!reveal.take_if_ready(2));

    reveal.record_sync_operation(&MessageListSyncOperation::Splice {
        old_range: 2..2,
        count: 1,
    });
    assert!(!reveal.take_if_ready(3));

    reveal.record_sync_operation(&MessageListSyncOperation::Reset { count: 3 });
    assert!(reveal.take_if_ready(3));
}

#[test]
fn initial_message_reveal_stays_disabled_when_not_configured() {
    let mut reveal = InitialMessageReveal::new(false);

    reveal.record_sync_operation(&MessageListSyncOperation::Reset { count: 2 });
    assert!(!reveal.take_if_ready(2));
}
