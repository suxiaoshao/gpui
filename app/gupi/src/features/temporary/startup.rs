//! The popup can open before the shared Pi probe has finished.
use super::TemporaryView;
use crate::{
    app::{
        menus,
        temporary::{self, Temporary},
    },
    components::recovery::recovery,
    features::home::actions::{Kind, Run},
    foundation::i18n::t,
    pi::ProbeFailureKey,
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
        if let Some((config, probe)) = cx.global::<Temporary>().environment.clone() {
            subscriptions.push(cx.observe(&probe, |_, _, cx| cx.notify()));
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
        // Once connected, existing sessions remain available during later probes.
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
        let Some((config, probe)) = cx.global::<Temporary>().environment.clone() else {
            return;
        };
        let applied = config
            .read(cx)
            .store
            .read(cx, |op| op.data().and_then(|d| d.configured()).cloned());
        if let Some(config) = applied {
            probe.update(cx, |pi, cx| pi.request(config.pi_command, true, cx));
        } else {
            config.update(cx, |config, cx| config.reload(cx));
        }
    }

    fn problem(&self, cx: &App) -> Option<(&'static str, &'static str)> {
        let (config, probe) = cx.global::<Temporary>().environment.as_ref()?;
        config
            .read(cx)
            .store
            .read(cx, |op| match op.data().map(|data| &data.contents) {
                Some(ConfigContents::Configured(_)) => {
                    let pi = probe.read(cx);
                    (!pi.operation.is_running())
                        .then(|| pi.operation.problem())
                        .flatten()
                        .map(|problem| ("recovery-pi-title", problem.key()))
                }
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

#[cfg(test)]
mod tests {
    use super::TemporaryStartup;
    use crate::app::temporary::{self, Temporary};
    use crate::state::config::ConfigContents;
    use crate::{
        pi::PiProbeController,
        state::{
            config::{AppConfig, ConfigController, ConfigData},
            conversation::ConversationState,
        },
    };
    use gpui_form::Form;
    use gpui_kit::{AppContext, TestAppContext, component::Root};
    use gpui_operation::{Settle, Transition};

    #[gpui_kit::test]
    fn startup_recovers_in_place_and_keeps_existing_sessions(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            gpui_tokio::init(cx);
            app_theme::init(cx);
            crate::state::theme::init(cx);
            crate::foundation::i18n::apply(Default::default(), cx);
            crate::state::pi::init(cx);
            temporary::init(cx);
            cx.set_global(crate::state::layout::LayoutState::default());
        });
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let config = cx.new(|cx| ConfigController::new(&form, cx));
        let probe = cx.new(|_| PiProbeController::new());
        cx.update(|cx| {
            let store = config.read(cx).store.clone();
            store.update(cx, |op| {
                op.transition(Settle(Ok(ConfigData {
                    path: "unused-config.toml".into(),
                    contents: ConfigContents::Configured(AppConfig {
                        pi_command: Some("unused-pi".into()),
                        ..Default::default()
                    }),
                    backup: None,
                })))
            });
            cx.global_mut::<Temporary>().environment = Some((config.clone(), probe.clone()));
        });
        let mut shell = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| TemporaryStartup::new(window, cx));
            shell = Some(view.clone());
            Root::new(view, window, cx)
        });
        let shell = shell.unwrap();
        visual.run_until_parked();
        assert!(visual.debug_bounds("temporary-startup").is_some());
        visual.update(|_, cx| assert!(cx.global::<Temporary>().state.is_none()));

        probe.update(visual, |pi, cx| {
            pi.operation
                .transition(Settle(Err(pi_rpc::probe::ProbeFailure::InvalidVersion)));
            cx.notify();
        });
        visual.run_until_parked();
        shell.read_with(visual, |view, cx| {
            assert_eq!(
                view.problem(cx),
                Some(("recovery-pi-title", "error-pi-version"))
            )
        });
        shell.update(visual, |view, cx| {
            view.retry(cx);
            assert!(probe.read(cx).operation.is_running());
            probe.update(cx, |pi, _| pi.stop());
        });

        // Publish a ready conversation without launching a real Pi process.
        let state = visual.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
        state.update(visual, |state, cx| {
            state.new_draft(None, cx);
            let session = state
                .sessions
                .get_mut(state.selected.as_ref().unwrap())
                .unwrap();
            // Cancel workspace preparation before any filesystem or process work.
            session.core_read.finish(None);
            session.binding = 1;
            session.draft = "retained draft".into();
        });
        visual.update(|_, cx| {
            cx.global_mut::<Temporary>().state = Some(state.clone());
            temporary::set_command("unused-pi".into(), cx);
        });
        visual.run_until_parked();
        let content = shell.read_with(visual, |view, _| view.content.clone().unwrap());
        assert!(visual.debug_bounds("temporary-startup").is_none());
        visual.update(|_, cx| cx.global_mut::<Temporary>().command = None);
        visual.run_until_parked();
        shell.read_with(visual, |view, _| {
            assert_eq!(view.content.as_ref(), Some(&content))
        });
        state.read_with(visual, |state, _| {
            assert_eq!(state.current().unwrap().draft, "retained draft")
        });
    }
}
