use super::{StartupScreen, StartupView};
use crate::{
    app::temporary,
    state::config::{AppConfig, ConfigContents, ConfigData},
};
use gpui_kit::{AppContext, Task, TestAppContext, component::Root};
use gpui_operation::{Cancel, Retry, Settle, Transition};

#[gpui_kit::test]
fn configured_windows_and_sessions_do_not_wait_for_a_probe(cx: &mut TestAppContext) {
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
    let mut startup = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| StartupView::new(false, window, cx));
        // Replace the pending filesystem read before it runs; this test never
        // reads or writes the user's configuration.
        let store = view.read(cx).config.read(cx).store.clone();
        store.update(cx, |op| {
            op.transition(Cancel);
            op.transition(Settle(Ok(ConfigData {
                path: "unused-config.toml".into(),
                contents: ConfigContents::Configured(AppConfig {
                    pi_command: Some("missing-pi".into()),
                    ..Default::default()
                }),
                backup: None,
            })));
        });
        startup = Some(view.clone());
        Root::new(view, window, cx)
    });
    let startup = startup.unwrap();
    startup.update(visual, |view, cx| {
        assert!(matches!(view.screen(cx), StartupScreen::Home(command) if command == std::path::Path::new("missing-pi")));
        assert!(!view.applied_pi.read(cx).operation.is_running());
        assert!(view.applied_pi.read(cx).operation.data().is_none());
        assert!(temporary::state(cx).is_some(), "template actions can create their session immediately");
        view.applied_pi.update(cx, |pi, _| {
            pi.operation.transition(Settle(Err(pi_rpc::probe::ProbeFailure::InvalidVersion)));
        });
        assert!(matches!(view.screen(cx), StartupScreen::Home(_)));
        view.applied_pi.update(cx, |pi, _| {
            pi.operation.transition(Retry(Task::ready(())));
        });
        assert!(matches!(view.screen(cx), StartupScreen::Home(_)));
        view.applied_pi.update(cx, |pi, _| pi.stop());
    });
}
