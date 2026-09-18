//! Configuration recovery before the popup can create a conversation.
use super::TemporaryView;
use crate::{
    app::{
        menus,
        temporary::{self, Temporary},
    },
    components::recovery::recovery,
    features::home::actions::{Kind, Run},
    foundation::i18n::t,
    state::config::ConfigContents,
};
use gpui_kit::{
    component::{ActiveTheme, Root, button::Button, h_flex, spinner::Spinner, v_flex},
    *,
};

pub(crate) struct TemporaryStartup {
    content: Option<Entity<TemporaryView>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl TemporaryStartup {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![
            cx.observe_global_in::<Temporary>(window, |this, window, cx| {
                this.sync(window, cx);
                cx.notify();
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if this.content.is_none() {
                    if window.is_window_active() {
                        this.focus.focus(window, cx);
                    } else {
                        temporary::on_deactivate(window, cx);
                    }
                }
            }),
        ];
        if let Some(config) = cx.global::<Temporary>().config.clone() {
            let store = config.read(cx).store.clone();
            subscriptions.push(store.observe_in(cx, window, |_, _, _, cx| cx.notify()));
        }
        let mut this = Self {
            content: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.sync(window, cx);
        this
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Existing sessions remain available during configuration reloads.
        if self.content.is_none()
            && let Some(state) = temporary::state(cx)
        {
            let view = cx.new(|cx| TemporaryView::new(state, window, cx));
            if self.focus.is_focused(window) {
                view.update(cx, |view, cx| view.focus_search(window, cx));
            }
            self.content = Some(view);
        }
    }

    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(content) = &self.content {
            content.update(cx, |view, cx| view.focus_search(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        if let Some(config) = cx.global::<Temporary>().config.clone() {
            config.update(cx, |config, cx| config.reload(cx));
        }
    }

    fn problem(&self, cx: &App) -> Option<(&'static str, &'static str)> {
        let config = cx.global::<Temporary>().config.as_ref()?;
        config
            .read(cx)
            .store
            .read(cx, |op| match op.data().map(|data| &data.contents) {
                Some(ConfigContents::Configured(_)) => None,
                Some(ConfigContents::Missing) => {
                    Some(("startup-welcome", "temporary-setup-required"))
                }
                None if !op.is_running() => op
                    .problem()
                    .map(|problem| ("recovery-config-title", problem.key())),
                _ => None,
            })
    }
}

impl Render for TemporaryStartup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(content) = &self.content {
            return content.clone().into_any_element();
        }
        window.set_window_title(&t(cx, "temporary-title"));
        let problem = self.problem(cx);
        let body = match problem {
            Some((title, description)) => {
                recovery(t(cx, title), t(cx, description), cx).into_any_element()
            }
            None => h_flex()
                .gap_2()
                .child(Spinner::new())
                .child(t(cx, "startup-checking"))
                .into_any_element(),
        };
        let mut actions = h_flex().gap_2();
        if problem.is_some() {
            actions = actions.child(
                Button::new("temporary-startup-retry")
                    .label(t(cx, "action-retry"))
                    .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
            );
        }
        actions = actions.child(
            Button::new("temporary-startup-settings")
                .label(t(cx, "menu-settings"))
                .on_click(|_, _, cx| cx.dispatch_action(&menus::ShowSettings)),
        );
        v_flex()
            .debug_selector(|| "temporary-startup".into())
            .key_context("Gupi GupiTemporary GupiApplication")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, action: &Run, window, cx| {
                if action.0 == Kind::Stop {
                    temporary::hide(window, cx);
                }
            }))
            .size_full()
            .items_center()
            .justify_center()
            .p_6()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(body)
            .child(actions)
            .children(Root::render_notification_layer(window, cx))
            .into_any_element()
    }
}
