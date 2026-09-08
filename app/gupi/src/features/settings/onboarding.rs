use super::*;
use gpui_kit::component::{
    ElementExt, Sizable,
    animation::ease_in_out_cubic,
    stepper::{Stepper, StepperItem},
};
use gpui_kit::prelude::FluentBuilder;
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Clone, Copy)]
pub(super) struct PageTransition {
    from: usize,
    to: usize,
    serial: u64,
}

impl SettingsView {
    fn navigate(&mut self, step: usize, window: &mut Window, cx: &mut Context<Self>) {
        if step > 3
            || step == self.step
            || self.transition.is_some()
            || self.controller.read(cx).busy(cx)
        {
            return;
        }
        self.focus_handle.focus(window, cx);
        self.transition_serial += 1;
        self.transition = (!cx.reduce_motion()).then_some(PageTransition {
            from: self.step,
            to: step,
            serial: self.transition_serial,
        });
        self.step = step;
        cx.notify();
    }

    pub(super) fn render_onboarding(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let busy = self.controller.read(cx).busy(cx) || self.transition.is_some();
        let mut viewport = div()
            .relative()
            .h_full()
            .w(px(800.))
            .min_w_0()
            .min_h_0()
            .overflow_hidden();
        if let Some(transition) = self.transition {
            let forward = transition.to > transition.from;
            let pages = if forward {
                [transition.from, transition.to]
            } else {
                [transition.to, transition.from]
            };
            let progress = Rc::new(Cell::new(0.));
            let completion = progress.clone();
            let owner = cx.entity().downgrade();
            let strip = h_flex()
                .relative()
                .w(relative(2.))
                .h_full()
                .items_stretch()
                .children(pages.map(|step| {
                    div()
                        .id(("setup-page", step))
                        .w(relative(0.5))
                        .h_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .child(self.render_onboarding_page(step, window, cx))
                }))
                .on_prepaint(move |_, _, cx| {
                    // Complete on the final rendered frame, using GPUI's animation clock.
                    if completion.get() >= 1. {
                        cx.defer(move |cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if this
                                    .transition
                                    .is_some_and(|active| active.serial == transition.serial)
                                {
                                    this.transition = None;
                                    cx.notify();
                                }
                            });
                        });
                    }
                })
                .with_animation(
                    ("setup-slide", transition.serial),
                    Animation::new(Duration::from_millis(240)).with_easing(ease_in_out_cubic),
                    move |strip, value| {
                        progress.set(value);
                        strip.left(relative(if forward { -value } else { value - 1. }))
                    },
                );
            viewport = viewport
                .child(strip)
                .child(div().absolute().top_0().left_0().size_full().occlude());
        } else {
            viewport = viewport.child(
                div()
                    .id(("setup-page", self.step))
                    .size_full()
                    .child(self.render_onboarding_page(self.step, window, cx)),
            );
        }
        let back = Button::new("setup-back")
            .size(px(40.))
            .outline()
            .icon(IconName::ArrowLeft)
            .tooltip(t(cx, "setup-back"))
            .accessibility_label(t(cx, "setup-back"))
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
                this.navigate(this.step.saturating_sub(1), window, cx);
            }));
        let next = Button::new("setup-next")
            .size(px(40.))
            .outline()
            .icon(IconName::ArrowRight)
            .tooltip(t(cx, "setup-next"))
            .accessibility_label(t(cx, "setup-next"))
            .disabled(busy || self.step == 3)
            .on_click(cx.listener(|this, _, window, cx| {
                this.navigate(this.step + 1, window, cx);
            }));
        h_flex()
            .size_full()
            .min_h_0()
            .child(navigation_gutter((self.step != 0).then_some(back)))
            .child(viewport)
            .child(navigation_gutter((self.step != 0).then_some(next)))
            .into_any_element()
    }

    fn render_onboarding_page(
        &self,
        step: usize,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let busy = self.controller.read(cx).busy(cx);
        let logo =
            || img(SharedString::from(crate::foundation::assets::app_logo(cx))).size(px(80.));
        if step == 0 {
            return v_flex()
                .w_full()
                .h_full()
                .min_h_0()
                .items_center()
                .justify_center()
                .gap_6()
                .child(logo())
                .child(
                    v_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t(cx, "startup-welcome")),
                        )
                        .child(
                            div()
                                .text_lg()
                                .text_color(cx.theme().muted_foreground)
                                .child(t(cx, "setup-tagline")),
                        ),
                )
                .child(
                    div()
                        .max_w(px(480.))
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "setup-intro")),
                )
                .child(
                    Button::new("setup-start")
                        .icon(IconName::ArrowRight)
                        .primary()
                        .large()
                        .label(t(cx, "setup-start"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.navigate(1, window, cx);
                        })),
                )
                .into_any_element();
        }
        let title = match step {
            1 => "setup-language-title",
            2 => "setup-appearance-title",
            _ => "setup-pi-title",
        };
        let mut view = v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .gap_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        img(SharedString::from(crate::foundation::assets::app_logo(cx)))
                            .size(px(28.)),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Gupi")),
            )
            .child(
                Stepper::new("setup-steps")
                    .selected_index(step - 1)
                    .small()
                    .items(
                        ["settings-language", "settings-theme", "setup-pi-step"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, key)| {
                                StepperItem::new()
                                    .disabled(busy || index + 1 > step)
                                    .child(t(cx, key))
                            }),
                    )
                    .on_click(cx.listener(|this, index, window, cx| {
                        if !this.controller.read(cx).busy(cx) && *index < this.step {
                            this.navigate(index + 1, window, cx);
                        }
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(cx, title)),
                    )
                    .when(step == 3, |view| {
                        view.child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(t(cx, "setup-pi-help")),
                        )
                    }),
            )
            .child(
                div()
                    .id("setup-page-body")
                    .track_scroll(&self.page_scroll[step])
                    .flex_1()
                    .min_h_0()
                    .when(step == 2, |view| view.overflow_hidden())
                    .when(step != 2, |view| view.overflow_y_scroll())
                    .child(match step {
                        1 => self.render_language(cx),
                        2 => self.render_appearance(window, cx),
                        _ => self.render_pi(cx),
                    }),
            );
        let store = self.controller.read(cx).store.clone();
        let problem = store.read(cx, |op| {
            op.problem().map(|problem| {
                (
                    problem.key,
                    problem.conflict,
                    problem.reconcile,
                    problem.pending.is_some(),
                )
            })
        });
        if let Some(key) = self.error.as_deref().or(problem.map(|p| p.0)) {
            view = view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(t(cx, key)),
            );
        }
        // Preserve explicit recovery choices if a file appeared or saving failed during setup.
        if let Some((_, conflict, reconcile, retry)) = problem {
            view =
                view.child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("setup-reload")
                                .icon(IconName::RotateCw)
                                .label(t(cx, "settings-reload"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request(ConfigRepair::Reload, cx)
                                })),
                        )
                        .when(conflict && !reconcile, |view| {
                            view.child(
                                Button::new("setup-overwrite")
                                    .label(t(cx, "action-overwrite"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(ConfigRepair::BackupAndWrite, cx)
                                    })),
                            )
                        })
                        .when(retry && !conflict && !reconcile, |view| {
                            view.child(
                                Button::new("setup-retry")
                                    .icon(IconName::RotateCw)
                                    .label(t(cx, "action-retry"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(ConfigRepair::RetryWrite, cx)
                                    })),
                            )
                        }),
                );
        }
        if self.confirmation.is_some() {
            view = view.child(
                v_flex().gap_2().child(t(cx, "recovery-confirm")).child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("setup-confirm")
                                .label(t(cx, "action-confirm"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(action) = this.confirmation.take() {
                                        this.controller
                                            .update(cx, |owner, cx| owner.repair(action, cx));
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("setup-cancel")
                                .label(t(cx, "action-cancel"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirmation = None;
                                    cx.notify();
                                })),
                        ),
                ),
            );
        }
        if step == 3 {
            view = view.child(
                Button::new("setup-finish")
                    .icon(IconName::Check)
                    .primary()
                    .label(t(cx, if busy { "setup-saving" } else { "setup-finish" }))
                    .disabled(busy || !self.probe_ready(cx) || problem.is_some_and(|p| p.1 || p.2))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.controller.read(cx).busy(cx) && this.probe_ready(cx) {
                            this.submit(window, cx);
                        }
                    })),
            );
        }
        view.into_any_element()
    }
}

fn navigation_gutter(button: Option<Button>) -> Div {
    h_flex()
        .flex_1()
        .min_w(px(48.))
        .h_full()
        .items_center()
        .justify_center()
        .children(button)
}
