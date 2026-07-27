use std::collections::HashMap;

use gpui::{App, BorrowAppContext, Global, Subscription, Task, Window, WindowId};

struct TaskOwners {
    application: Vec<Task<()>>,
    windows: HashMap<WindowId, Vec<Task<()>>>,
    _window_closed_subscription: Subscription,
}

impl Global for TaskOwners {}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<TaskOwners>() {
        return;
    }
    let window_closed_subscription = cx.on_window_closed(|cx, window_id| {
        cx.update_global::<TaskOwners, _>(|owners, _cx| {
            owners.windows.remove(&window_id);
        });
    });
    cx.set_global(TaskOwners {
        application: Vec::new(),
        windows: HashMap::new(),
        _window_closed_subscription: window_closed_subscription,
    });
}

pub(crate) fn retain_application(task: Task<()>, cx: &mut App) {
    init(cx);
    cx.update_global::<TaskOwners, _>(|owners, _cx| {
        owners.application.retain(|task| !task.is_ready());
        owners.application.push(task);
    });
}

pub(crate) fn retain_window(window: &Window, task: Task<()>, cx: &mut App) {
    init(cx);
    let window_id = window.window_handle().window_id();
    cx.update_global::<TaskOwners, _>(|owners, _cx| {
        let tasks = owners.windows.entry(window_id).or_default();
        tasks.retain(|task| !task.is_ready());
        tasks.push(task);
    });
}
