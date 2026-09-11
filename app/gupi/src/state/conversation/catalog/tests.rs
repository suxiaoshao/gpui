use super::*;
use std::cell::Cell;
use std::rc::Rc;

struct Task(Rc<Cell<usize>>);
impl Drop for Task {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn start(state: &mut CatalogState<Task>, id: u64, dropped: &Rc<Cell<usize>>) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    state.transition(Message::Start(ScanWork::new(
        id,
        Task(dropped.clone()),
        cancel.clone(),
    )));
    cancel
}
#[test]
fn phase_transfer_preserves_task_and_rejects_invalid_or_stale_progress() {
    let dropped = Rc::new(Cell::new(0));
    let mut state = CatalogState::Idle;
    let cancel = start(&mut state, 1, &dropped);
    state.transition(Message::Progress {
        id: 1,
        value: ScanProgress::Reading {
            completed: 0,
            total: 2,
        },
    });
    assert_eq!(dropped.get(), 0);
    assert!(!cancel.load(Ordering::Relaxed));
    state.transition(Message::Progress {
        id: 1,
        value: ScanProgress::Reading {
            completed: 1,
            total: 2,
        },
    });
    for (id, completed, total) in [(1, 0, 2), (1, 3, 2), (1, 2, 3), (0, 2, 2)] {
        assert_eq!(
            state.transition(Message::Progress {
                id,
                value: ScanProgress::Reading { completed, total }
            }),
            Update::Ignored
        );
    }
    assert_eq!(
        state.transition(Message::Finish {
            id: 1,
            result: Ok(Catalog::default())
        }),
        Update::Ignored
    );
    state.transition(Message::Cancel);
    assert!(cancel.load(Ordering::Relaxed));
    assert_eq!(dropped.get(), 1);
    start(&mut state, 2, &dropped);
    assert_eq!(
        state.transition(Message::Finish {
            id: 1,
            result: Err("old".into())
        }),
        Update::Ignored
    );
    assert!(state.running());
}
#[test]
fn failed_refresh_preserves_only_whole_previous_catalog_and_does_not_retry() {
    let dropped = Rc::new(Cell::new(0));
    let mut old = Catalog::default();
    old.directories.insert("old".into());
    let mut state = CatalogState::Ready(old);
    start(&mut state, 1, &dropped);
    state.transition(Message::QueueRefresh);
    assert_eq!(
        state.transition(Message::Finish {
            id: 1,
            result: Err("unreadable".into())
        }),
        Update::Finished { rescan: false }
    );
    assert_eq!(state.error(), Some("unreadable"));
    assert!(
        state
            .data()
            .unwrap()
            .directories
            .contains(std::path::Path::new("old"))
    );
    start(&mut state, 2, &dropped);
    assert_eq!(state.error(), None);
    state.transition(Message::Cancel);
    assert!(matches!(state, CatalogState::Ready(_)));
    start(&mut state, 3, &dropped);
    state.transition(Message::QueueRefresh);
    state.transition(Message::QueueRefresh);
    state.transition(Message::Progress {
        id: 3,
        value: ScanProgress::Reading {
            completed: 0,
            total: 0,
        },
    });
    assert_eq!(
        state.transition(Message::Finish {
            id: 3,
            result: Ok(Catalog::default())
        }),
        Update::Finished { rescan: true }
    );
    assert!(state.data().unwrap().directories.is_empty());
    assert_eq!(dropped.get(), 3);
}
