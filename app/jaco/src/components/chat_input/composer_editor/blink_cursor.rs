use std::time::Duration;

use gpui::{Context, Pixels, Task, px};

const BLINK_INTERVAL: Duration = Duration::from_millis(500);
const RESUME_DELAY: Duration = Duration::from_millis(300);

#[cfg(target_os = "macos")]
pub(super) const CURSOR_WIDTH: Pixels = px(1.5);
#[cfg(not(target_os = "macos"))]
pub(super) const CURSOR_WIDTH: Pixels = px(2.);

pub(super) struct BlinkCursor {
    visible: bool,
    paused: bool,
    running: bool,
    epoch: usize,
    _task: Task<()>,
}

impl BlinkCursor {
    pub(super) fn new() -> Self {
        Self {
            visible: false,
            paused: false,
            running: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    pub(super) fn start(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }

        self.running = true;
        self.paused = false;
        self.visible = false;
        self.blink(self.epoch, cx);
    }

    pub(super) fn stop(&mut self, cx: &mut Context<Self>) {
        if !self.running && !self.visible {
            return;
        }

        self.running = false;
        self.paused = false;
        self.visible = false;
        self.next_epoch();
        cx.notify();
    }

    pub(super) fn pause(&mut self, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }

        self.paused = true;
        self.visible = true;
        cx.notify();

        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESUME_DELAY).await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    if this.running && this.epoch == epoch {
                        this.paused = false;
                        this.blink(epoch, cx);
                    }
                });
            }
        });
    }

    pub(super) fn visible(&self) -> bool {
        self.paused || self.visible
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if !self.running || self.paused || epoch != self.epoch {
            return;
        }

        self.visible = !self.visible;
        cx.notify();

        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BLINK_INTERVAL).await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.blink(epoch, cx));
            }
        });
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }
}
