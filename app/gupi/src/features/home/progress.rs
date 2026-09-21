//! Timers belong to the small status views; they never invalidate conversation data.
use super::*;
use crate::foundation::i18n::t_with_args;
use crate::state::conversation::execution::RetryProgress;
use fluent_bundle::FluentArgs;
use gpui_kit::prelude::FluentBuilder as _;
use std::time::Duration;

pub(super) struct ProcessClock {
    started_at: Option<i64>,
    tick: Option<Task<()>>,
}
impl ProcessClock {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            started_at: None,
            tick: None,
        }
    }
    pub fn sync(&mut self, started_at: Option<i64>, cx: &mut Context<Self>) {
        if self.started_at == started_at {
            return;
        }
        self.started_at = started_at;
        self.tick = started_at.map(|_| {
            cx.spawn(async |owner, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if owner.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });
        cx.notify();
    }
}
impl Render for ProcessClock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The disclosure already has a single-line title; ticking cannot change row height.
        div()
            .min_w_0()
            .truncate()
            .child(messages::metadata::process_title(
                &[],
                true,
                self.started_at,
                cx,
            ))
    }
}

pub(super) struct RetryView {
    retry: Option<RetryProgress>,
    summary: Option<RetryProgress>,
    tick: Option<Task<()>>,
}
impl RetryView {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            retry: None,
            summary: None,
            tick: None,
        }
    }
    pub fn sync(
        &mut self,
        retry: Option<RetryProgress>,
        summary: Option<RetryProgress>,
        cx: &mut Context<Self>,
    ) {
        if self.retry == retry && self.summary == summary {
            return;
        }
        self.retry = retry;
        self.summary = summary;
        self.tick = None;
        if self.retry.is_some() || self.summary.is_some() {
            self.tick = Some(cx.spawn(async |owner, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let keep = owner
                        .update(cx, |this, cx| {
                            cx.notify();
                            this.retry
                                .iter()
                                .chain(this.summary.iter())
                                .any(|p| p.remaining() > 0)
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            }));
        }
        cx.notify();
    }
}
impl Render for RetryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_1().children(
            self.retry
                .iter()
                .map(|p| (p, false))
                .chain(self.summary.iter().map(|p| (p, true)))
                .map(|(progress, summary)| {
                    let mut args = FluentArgs::new();
                    args.set("attempt", progress.attempt as i64);
                    args.set("total", progress.max_attempts as i64);
                    args.set("seconds", progress.remaining() as i64);
                    let key = match (summary, progress.remaining() == 0) {
                        (false, false) => "conversation-retry-countdown",
                        (false, true) => "conversation-retry-waiting",
                        (true, false) => "conversation-summary-retry-countdown",
                        (true, true) => "conversation-summary-retry-waiting",
                    };
                    v_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t_with_args(cx, key, &args))
                        .when(!progress.reason.is_empty(), |view| {
                            view.child(div().truncate().child(progress.reason.clone()))
                        })
                }),
        )
    }
}
