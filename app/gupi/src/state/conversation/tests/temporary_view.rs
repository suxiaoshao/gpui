use super::super::{ConversationState, Session, SessionInfo};
use crate::{
    features::temporary::TemporaryView,
    state::{config::AppLanguage, pi},
};
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};

#[gpui_kit::test]
fn temporary_page_tab_search_and_recreation_preserve_the_draft(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        pi::init(cx);
        crate::app::temporary::init(cx);
        cx.set_global(crate::state::layout::LayoutState::default());
    });
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, _| {
        let mut session = Session::new(
            SessionInfo {
                path: Default::default(),
                id: "first".into(),
                cwd: "/tmp".into(),
                name: Some("Alpha".into()),
                first_message: String::new(),
                activity: "1".into(),
                parent_session: None,
            },
            "draft".into(),
        );
        session.state = Some(
            serde_json::from_value(serde_json::json!({"sessionId":"fixture", "isStreaming":false,"isCompacting":false}))
                .unwrap(),
        );
        state.sessions.insert("first".into(), session);
        state.selected = Some("first".into());
    });
    for suffix in [" one", " two"] {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| TemporaryView::new(state.clone(), window, cx));
            view.update(cx, |view, cx| view.focus_search(window, cx));
            Root::new(view, window, cx)
        });
        visual.simulate_resize(size(px(960.), px(620.)));
        visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        visual.simulate_input("no-match");
        visual.update(|_, cx| assert_eq!(state.read(cx).selected.as_deref(), Some("first")));
        visual.simulate_keystrokes("tab");
        visual.dispatch_action(gpui_kit::component::input::MoveToEnd);
        visual.simulate_input(suffix);
        visual.run_until_parked();
        visual.update(|_, cx| {
            assert!(
                state.read(cx).current().unwrap().draft.ends_with(suffix),
                "actual draft: {:?}",
                state.read(cx).current().unwrap().draft
            )
        });
        visual.simulate_keystrokes("tab");
        visual.simulate_input(" still-searching");
        visual.update(|window, cx| {
            assert!(
                !state
                    .read(cx)
                    .current()
                    .unwrap()
                    .draft
                    .contains("searching")
            );
            window.remove_window();
        });
        visual.run_until_parked();
    }
    state.read_with(cx, |state, _| {
        assert_eq!(state.current().unwrap().draft, "draft one two");
        assert!(state.current().unwrap().instance.is_none());
    });
}

fn init_interactions(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        pi::init(cx);
        crate::app::temporary::init(cx);
        crate::app::temporary::make_visible_for_test(cx);
        crate::state::keybindings::apply(&Default::default(), cx);
        cx.set_global(crate::state::layout::LayoutState::default());
    });
}
fn fixture_session(name: &str) -> Session {
    let mut session = Session::new(
        SessionInfo {
            path: Default::default(),
            id: name.into(),
            cwd: "/tmp".into(),
            name: Some(name.into()),
            first_message: String::new(),
            activity: String::new(),
            parent_session: None,
        },
        String::new(),
    );
    // A detached in-memory fixture must never launch a real Pi process.
    session.binding = 1;
    session.state = Some(
        serde_json::from_value(
            serde_json::json!({"sessionId":name,"isStreaming":false,"isCompacting":false}),
        )
        .unwrap(),
    );
    session
}
fn answer(session: &mut Session, text: &str) {
    session.live.push(crate::state::history::DisplayMessage {
        id: "answer".into(), entry: None, completed_at: None, final_answer_part: None,
        value: serde_json::json!({"role":"assistant","stopReason":"stop","content":[{"type":"thinking","thinking":"private reasoning"},{"type":"text","text":text}]}),
    });
    session.content_revision += 1;
}

#[gpui_kit::test]
fn temporary_new_reuses_unsent_draft_and_navigation_handles_more_than_nine(
    cx: &mut TestAppContext,
) {
    init_interactions(cx);
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, cx| {
        for i in 0..12 {
            let key = format!("session-{i:02}");
            let mut session = fixture_session(&key);
            if i == 0 {
                session.draft = "preserve this draft".into();
                // A healthy draft still preparing its first connection can be
                // reused. Keep preparation pending without launching Pi.
                session.binding = 0;
                session.core_read = super::super::CoreRead::CheckingFile {
                    _task: cx.spawn(async |_, _| std::future::pending().await),
                };
            } else {
                answer(&mut session, "complete");
            }
            state.sessions.insert(key, session);
        }
        state.selected = Some("session-01".into());
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| TemporaryView::new(state.clone(), window, cx));
        view.update(cx, |view, cx| view.focus_search(window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(960.), px(620.)));
    visual.run_until_parked();
    visual.simulate_keystrokes("cmd-9");
    visual.update(|_, cx| assert_eq!(state.read(cx).selected.as_deref(), Some("session-11")));
    visual.simulate_keystrokes("cmd-2");
    visual.update(|_, cx| assert_eq!(state.read(cx).selected.as_deref(), Some("session-01")));
    visual.simulate_keystrokes("cmd-n");
    visual.run_until_parked();
    visual.update(|_, cx| {
        let state = state.read(cx);
        assert_eq!(state.selected.as_deref(), Some("session-00"));
        assert_eq!(state.current().unwrap().draft, "preserve this draft");
        assert_eq!(state.sessions.len(), 12);
    });
    visual.simulate_keystrokes("cmd-n");
    visual.update(|_, cx| assert_eq!(state.read(cx).sessions.len(), 12));
}

#[gpui_kit::test]
fn temporary_new_skips_failed_and_exited_drafts(cx: &mut TestAppContext) {
    init_interactions(cx);
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, cx| {
        let mut failed = fixture_session("failed");
        failed.binding = 0;
        failed.error = Some("Pi failed to start".into());
        failed.draft = "keep failed draft".into();
        state.sessions.insert("failed".into(), failed);
        state
            .sessions
            .insert("exited".into(), fixture_session("exited"));
        let mut read_failed = fixture_session("read-failed");
        read_failed.binding = 0;
        read_failed.core_read = super::super::CoreRead::Failed("Cannot prepare workspace".into());
        state.sessions.insert("read-failed".into(), read_failed);

        let mut healthy = fixture_session("healthy");
        healthy.binding = 0;
        healthy.draft = "keep healthy draft".into();
        healthy.core_read = super::super::CoreRead::CheckingFile {
            _task: cx.spawn(async |_, _| std::future::pending().await),
        };
        state.sessions.insert("healthy".into(), healthy);

        for selected in ["failed", "exited", "read-failed"] {
            state.selected = Some(selected.into());
            state.new_or_reuse(None, cx);
            assert_eq!(state.selected.as_deref(), Some("healthy"));
            assert_eq!(state.sessions.len(), 4);
            assert_eq!(state.current().unwrap().draft, "keep healthy draft");
        }

        state.sessions.remove("healthy");
        state.selected = Some("exited".into());
        state.new_or_reuse(None, cx);
        let replacement = state.selected.clone().unwrap();
        assert!(replacement.starts_with("draft-"));
        assert_eq!(state.sessions.len(), 4);
        assert_eq!(state.sessions["failed"].draft, "keep failed draft");
        assert!(state.sessions[&replacement].core_read.running());
        // Cancel preparation before it touches disk or starts a process.
        state.sessions.get_mut(&replacement).unwrap().reset_reads();
    });
}

#[gpui_kit::test]
fn temporary_actions_filter_copy_and_restore_focus_without_sending(cx: &mut TestAppContext) {
    use gpui_kit::ClipboardItem;
    init_interactions(cx);
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, _| {
        let mut session = fixture_session("first");
        answer(&mut session, "final answer");
        state.sessions.insert("first".into(), session);
        state.selected = Some("first".into());
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| TemporaryView::new(state.clone(), window, cx));
        view.update(cx, |view, cx| view.focus_search(window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(960.), px(620.)));
    visual.run_until_parked();
    visual.simulate_keystrokes("cmd-k");
    visual.run_until_parked();
    visual.simulate_input("copy");
    visual.run_until_parked();
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("final answer")
        )
    });
    visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    visual.simulate_keystrokes("cmd-k");
    visual.simulate_keystrokes("cmd-enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("final answer")
        )
    });
    // Escape must dismiss the action panel, not stop/hide the window.
    visual.simulate_keystrokes("cmd-k");
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.simulate_keystrokes("tab");
    visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    visual.simulate_keystrokes("shift-enter");
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        )
    });
    // Whitespace-only composer: Enter copies the final answer; no external
    // target exists in this fixture, so the UI stays open with a fallback notice.
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("final answer")
        );
        assert!(state.read(cx).current().unwrap().instance.is_none());
    });
    visual.simulate_input("follow up");
    visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    visual.simulate_keystrokes("enter");
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        )
    });
}

#[test]
fn temporary_return_requires_successful_final_text_and_no_pending_input() {
    let mut session = fixture_session("first");
    answer(&mut session, "answer");
    assert_eq!(session.completed_answer().as_deref(), Some("answer"));
    session.live[0].value["stopReason"] = "toolUse".into();
    assert!(session.completed_answer().is_none());
    session.live[0].value["stopReason"] = "aborted".into();
    assert!(session.completed_answer().is_none());
    session.live[0].value["stopReason"] = "stop".into();
    session.live[0].value["content"] = serde_json::json!([{"type":"text","text":"progress"},{"type":"thinking","thinking":"private"},{"type":"text","text":"final"}]);
    session.live[0].final_answer_part = Some(2);
    assert_eq!(session.completed_answer().as_deref(), Some("final"));
    session.pending_count = 1;
    assert!(session.completed_answer().is_none());
    session.pending_count = 0;
    session.draft = "  \n".into();
    assert!(session.composer_empty());
    session.attachments_read = Some(gpui_kit::Task::ready(()));
    assert!(!session.composer_empty());
}

#[gpui_kit::test]
fn temporary_composing_text_and_attachments_never_trigger_return(cx: &mut TestAppContext) {
    use crate::features::home::HomeView;
    use gpui_kit::{ClipboardItem, EntityInputHandler};
    init_interactions(cx);
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, _| {
        let mut session = fixture_session("first");
        answer(&mut session, "final answer");
        state.sessions.insert("first".into(), session);
        state.selected = Some("first".into());
    });
    let mut home = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| HomeView::with_state(state.clone(), window, cx));
        home = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    let home = home.unwrap();
    visual.update(|window, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
        let input = home.read(cx).input.clone();
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, " ", Some(0..1), window, cx)
        });
        home.update(cx, |home, cx| home.submit_or_paste(false, window, cx));
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        );
        input.update(cx, |input, cx| {
            input.unmark_text(window, cx);
            input.set_value("", window, cx);
        });
        state.update(cx, |state, _| {
            state.sessions.get_mut("first").unwrap().attachments.push(
                crate::foundation::attachments::Attachment::file("/tmp/fixture.txt".into(), 0),
            )
        });
        home.update(cx, |home, cx| home.submit_or_paste(false, window, cx));
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        );
    });
}

#[gpui_kit::test]
fn temporary_panel_switches_stop_hide_and_honors_rebound_shortcuts(cx: &mut TestAppContext) {
    use crate::{features::home::actions::Kind, state::keybindings};
    use gpui_kit::ClipboardItem;
    init_interactions(cx);
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, _| {
        let mut session = fixture_session("first");
        answer(&mut session, "final answer");
        state.sessions.insert("first".into(), session);
        state.selected = Some("first".into());
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| TemporaryView::new(state.clone(), window, cx));
        view.update(cx, |view, cx| view.focus_search(window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(960.), px(620.)));
    visual.simulate_keystrokes("cmd-k");
    visual.run_until_parked();
    assert!(visual.debug_bounds("temporary-action-hide").is_some());
    assert!(visual.debug_bounds("temporary-action-stop").is_none());
    visual.update(|_, cx| {
        state.update(cx, |state, cx| {
            state.sessions.get_mut("first").unwrap().pending_count = 1;
            cx.notify();
        })
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("temporary-action-stop").is_some());
    assert!(visual.debug_bounds("temporary-action-hide").is_none());
    visual.simulate_keystrokes("escape");
    visual.update(|_, cx| {
        state.update(cx, |state, cx| {
            state.sessions.get_mut("first").unwrap().pending_count = 0;
            cx.notify();
        });
        let overrides = keybindings::Overrides::from([
            ("temporary_paste".into(), "ctrl-alt-v".into()),
            ("temporary_copy".into(), "ctrl-alt-c".into()),
            ("temporary_trash".into(), "ctrl-alt-d".into()),
            ("stop".into(), "ctrl-alt-s".into()),
        ]);
        for (id, value) in &overrides {
            assert!(
                keybindings::validate(id, value, &overrides, cx).is_ok(),
                "{id}"
            );
        }
        keybindings::apply(&overrides, cx);
        assert!(!keybindings::uses_enter(Kind::PasteAnswer, cx));
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
    });
    visual.simulate_keystrokes("enter");
    visual.simulate_keystrokes("cmd-enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        )
    });
    visual.simulate_keystrokes("tab");
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        )
    });
    visual.simulate_keystrokes("ctrl-alt-v");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("final answer")
        );
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
    });
    visual.simulate_keystrokes("cmd-k");
    visual.simulate_keystrokes("ctrl-alt-c");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("final answer")
        );
        keybindings::apply(
            &keybindings::Overrides::from([
                ("temporary_paste".into(), String::new()),
                ("temporary_copy".into(), String::new()),
            ]),
            cx,
        );
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
    });
    visual.simulate_keystrokes("ctrl-alt-v");
    visual.simulate_keystrokes("ctrl-alt-c");
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("sentinel")
        )
    });
}

#[gpui_kit::test]
fn composer_attachments_stay_inside_group_and_remove_without_preview(cx: &mut TestAppContext) {
    use crate::foundation::attachments::Attachment;
    use gpui_kit::{SharedString, component::WindowExt, test::TestWindowExt};

    init_interactions(cx);
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1200, 800)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut image = Attachment::from_image("界面.png".into(), png.get_ref()).unwrap();
    image.id = "image".into();
    let mut file = Attachment::file("/tmp/README.md".into(), 128_000);
    file.id = "file".into();
    let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
    state.update(cx, |state, _| {
        let mut session = fixture_session("attachments");
        session.draft = "keep draft".into();
        session.attachments = vec![image, file];
        state.sessions.insert("attachments".into(), session);
        state.selected = Some("attachments".into());
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| TemporaryView::new(state.clone(), window, cx));
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(960.), px(620.)));
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let group = window.find("conversation-composer").bounds();
        let attachments = window
            .within("conversation-composer")
            .find("attachments")
            .bounds();
        let footer = window
            .within("conversation-composer")
            .find("footer")
            .bounds();
        assert!(group.contains(&attachments.origin));
        assert!(attachments.bottom() <= footer.top());
        window.click(SharedString::from("preview-image"), cx);
        assert!(window.has_active_dialog(cx));
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let preview = window.find("image-preview").bounds();
        assert!(preview.size.width > window.viewport_size().width * 0.9);
        assert!(preview.size.height > window.viewport_size().height * 0.9);
        assert!(preview.right() <= window.viewport_size().width);
        assert!(preview.bottom() <= window.viewport_size().height);
        let bitmap = window.find("image-preview-bitmap").bounds();
        let image_area = window.find("image-preview-scroll").bounds();
        assert!(bitmap.size.width <= image_area.size.width);
        assert!(bitmap.size.height <= image_area.size.height);
        window.click("image-preview-zoom-in", cx);
        assert!(window.find("image-preview-bitmap").bounds().size.width > bitmap.size.width);
        let enlarged = window.find("image-preview-bitmap").bounds();
        window.scroll(
            "image-preview-scroll",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-24.), px(-24.))),
            cx,
        );
        let scrolled = window.find("image-preview-bitmap").bounds();
        assert_eq!(
            scrolled.size, enlarged.size,
            "plain scrolling must not zoom"
        );
        assert!(scrolled.origin.y < enlarged.origin.y);
        window.click("image-preview-bitmap", cx);
        assert!(
            window.has_active_dialog(cx),
            "clicking the image must not close it"
        );
        window.click("image-preview-zoom-out", cx);
        assert_eq!(
            window.find("image-preview-bitmap").bounds().size,
            bitmap.size
        );
        window.click("image-preview-close", cx);
        assert!(
            !window.has_active_dialog(cx),
            "preview must open only one dialog"
        );
        window.render_frame(cx);
        window.click(SharedString::from("preview-image"), cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.press("escape", cx);
        assert!(!window.has_active_dialog(cx), "Escape closes the preview");
        window.click(SharedString::from("preview-image"), cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.click_at("image-preview", gpui_kit::point(px(2.), px(100.)), cx);
        assert!(
            !window.has_active_dialog(cx),
            "clicking the blank margin closes the preview"
        );
        window.click(SharedString::from("remove-image"), cx);
        assert!(
            !window.has_active_dialog(cx),
            "remove must not open image preview"
        );
        assert_eq!(state.read(cx).current().unwrap().attachments.len(), 1);
        assert_eq!(state.read(cx).current().unwrap().draft, "keep draft");
        window.click(SharedString::from("remove-file"), cx);
        assert!(state.read(cx).current().unwrap().attachments.is_empty());
        assert!(
            window
                .within("conversation-composer")
                .try_find("attachments")
                .is_none()
        );
        assert!(
            window
                .within("conversation-composer")
                .try_find("footer")
                .is_some()
        );
    });
}
