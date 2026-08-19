use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
};

use crate::foundation::assets::IconName;

pub(super) type OnCopy = Rc<dyn Fn(String, &mut Window, &mut App) -> bool + 'static>;

const COPIED_STATE_DURATION: Duration = Duration::from_secs(2);

struct CopyButtonState {
    copied_at: Option<Instant>,
}

#[derive(IntoElement)]
pub(super) struct CopyButton {
    state: Entity<CopyButtonState>,
    id: String,
    copy_text: String,
    on_copy: OnCopy,
    copy_tooltip: String,
    copied_tooltip: String,
}

impl CopyButton {
    pub(super) fn new(
        id: String,
        copy_text: String,
        on_copy: OnCopy,
        copy_tooltip: String,
        copied_tooltip: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let state_key = format!("{id}-copied-state");
        let state = window.use_keyed_state(state_key, cx, CopyButtonState::new);
        Self {
            state,
            id,
            copy_text,
            on_copy,
            copy_tooltip,
            copied_tooltip,
        }
    }
}

impl CopyButtonState {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { copied_at: None }
    }

    fn is_copied(&self) -> bool {
        self.copied_at
            .is_some_and(|copied_at| copied_at.elapsed() < COPIED_STATE_DURATION)
    }

    fn mark_copied(&mut self) {
        self.copied_at = Some(Instant::now());
    }
}

impl View for CopyButton {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_copied = self.state.read(cx).is_copied();
        let icon = if is_copied {
            Icon::new(IconName::Check).text_color(cx.theme().success)
        } else {
            Icon::new(IconName::Copy)
        };
        let tooltip = if is_copied {
            self.copied_tooltip
        } else {
            self.copy_tooltip
        };
        let state = self.state;
        let copy_text = self.copy_text;
        let on_copy = self.on_copy;

        Button::new(self.id)
            .ghost()
            .xsmall()
            .icon(icon)
            .tooltip(tooltip)
            .disabled(is_copied)
            .on_click(move |_, window, cx| {
                if !on_copy(copy_text.clone(), window, cx) {
                    return;
                }

                state.update(cx, |state, cx| {
                    state.mark_copied();
                    cx.notify();
                });

                let state_id = state.entity_id();
                let timer = cx.spawn(async move |cx| {
                    cx.background_executor().timer(COPIED_STATE_DURATION).await;
                    cx.update(|cx| {
                        cx.notify(state_id);
                    })
                });
                crate::app::tasks::retain_window(window, timer, cx);
            })
    }
}
