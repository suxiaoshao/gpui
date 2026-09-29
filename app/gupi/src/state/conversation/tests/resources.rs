use super::super::{ConversationState, ReadState};
use super::loading::{begin, close, count};
use crate::features::home::{
    HomeView,
    actions::{Kind, Run},
};
use crate::foundation::attachments::Attachment;
use gpui_kit::{
    AppContext, Entity, SharedString, TestAppContext,
    component::{Root, input},
    px, size,
    test::TestWindowExt,
};
use pi_rpc::protocol::SlashCommand;

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::state::keybindings::apply(&Default::default(), cx);
        cx.set_global(crate::state::layout::LayoutState::default());
    });
}
fn resource(name: &str, source: &str, path: &std::path::Path) -> SlashCommand {
    serde_json::from_value(
        serde_json::json!({"name":name,"source":source,"sourceInfo":{"path":path}}),
    )
    .unwrap()
}
fn file_draft(prefix: &str, path: &std::path::Path, suffix: &str) -> input::InputContent {
    let token = crate::foundation::composer_resources::file_token(path, false);
    input::InputContent::new(format!("{prefix}{}{suffix}", token.text()))
        .with_token(prefix.len()..prefix.len() + token.text().len(), token)
        .unwrap()
}

fn request(
    owner: &Entity<ConversationState>,
    key: &str,
    raw: serde_json::Value,
    cx: &mut TestAppContext,
) {
    owner.update(cx, |state, cx| {
        state.on_event(
            &crate::state::pi::PiEvent {
                instance: state.sessions[key].instance.unwrap(),
                event: pi_rpc::protocol::Event::ExtensionUi {
                    request: serde_json::from_value(raw.clone()).unwrap(),
                    raw,
                },
            },
            cx,
        );
    });
}

#[gpui_kit::test]
async fn resource_selection_is_atomic_and_never_sends_from_the_panel(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| {
        state.can_submit(&key, cx) && !state.sessions[&key].commands.running()
    })
    .await;
    init(cx);
    owner.update(cx, |state, cx| {
        state.sessions.get_mut(&key).unwrap().commands = ReadState::Ready(vec![
            resource("skill:review", "skill", &dir.path().join("SKILL.md")),
            resource("summary", "prompt", &dir.path().join("summary.md")),
        ]);
        state.set_draft(&key, "检查中文 🦀".into(), cx);
        state
            .sessions
            .get_mut(&key)
            .unwrap()
            .attachments
            .push(Attachment::file(dir.path().join("keep.txt"), 1));
    });
    let mut home = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| HomeView::with_state(owner.clone(), window, cx));
        home = Some(view.clone());
        Root::new(view, window, cx)
    });
    let home = home.unwrap();
    visual.simulate_resize(size(px(1200.), px(800.)));
    visual.update(|window, cx| {
        home.update(cx, |home, cx| {
            home.run_action(&Run(Kind::Palette), window, cx)
        })
    });
    visual.run_until_parked();
    visual.simulate_input("/skill:review");
    visual.run_until_parked();
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        owner.read_with(cx, |state, _| {
            let draft = &state.sessions[&key].draft;
            assert_eq!(draft.text().as_ref(), "/skill:review 检查中文 🦀");
            assert_eq!(draft.tokens().len(), 1);
            assert_eq!(state.sessions[&key].attachments.len(), 1);
        })
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    visual.dispatch_action(input::Undo);
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert_eq!(state.sessions[&key].draft.text().as_ref(), "检查中文 🦀")
    });
    visual.dispatch_action(input::Redo);
    visual.run_until_parked();
    // The caret is after the atomic command and its Pi separator. Backspace
    // deletes the whole command; undo restores its metadata, not just its text.
    visual.simulate_keystrokes("backspace");
    visual.run_until_parked();
    visual.update(|_, cx| {
        owner.read_with(cx, |state, _| {
            assert_eq!(state.sessions[&key].draft.text().as_ref(), "检查中文 🦀");
            assert!(state.sessions[&key].draft.tokens().is_empty());
        })
    });
    visual.dispatch_action(input::Undo);
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            owner.read_with(cx, |state, _| state.sessions[&key].draft.tokens().len()),
            1
        )
    });
    visual.update(|window, cx| {
        home.update(cx, |home, cx| {
            home.run_action(&Run(Kind::Palette), window, cx)
        })
    });
    visual.simulate_input("/summary");
    visual.run_until_parked();
    visual.simulate_keystrokes("tab");
    visual.simulate_input("search query");
    visual.run_until_parked();
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, cx| {
        owner.read_with(cx, |state, _| {
            assert_eq!(
                state.sessions[&key].draft.text().as_ref(),
                "/summary 检查中文 🦀"
            );
            assert_eq!(
                state.sessions[&key].draft.tokens()[0].token().id().as_ref(),
                "prompt:summary"
            );
        })
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    visual.update(|window, _| window.remove_window());
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn template_files_validate_positions_and_preserve_failed_draft(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| {
        state.can_submit(&key, cx) && !state.sessions[&key].commands.running()
    })
    .await;
    let template = dir.path().join("summary.md");
    let file = dir.path().join("a 'quoted' \"name\" 文件.txt");
    std::fs::write(&template, "Only $1").unwrap();
    owner.update(cx, |state, cx| {
        state.sessions.get_mut(&key).unwrap().commands =
            ReadState::Ready(vec![resource("summary", "prompt", &template)]);
        state.set_draft_content(&key, file_draft("/summary existing ", &file, "tail"), cx);
    });
    let reply = owner.update(cx, |state, cx| state.send_draft(&key, cx).unwrap());
    assert!(!reply.await.unwrap_or(false));
    owner.read_with(cx, |state, _| {
        assert_eq!(
            state.sessions[&key].draft.text().as_ref(),
            file_draft("/summary existing ", &file, "tail")
                .text()
                .as_ref()
        );
        assert_eq!(state.sessions[&key].draft.tokens().len(), 1);
        assert!(state.sessions[&key].attachments.is_empty());
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    std::fs::write(&template, "$ARGUMENTS").unwrap();
    owner.update(cx, |state, cx| {
        state.set_draft_content(&key, file_draft("/summary \"unclosed ", &file, "tail"), cx)
    });
    let reply = owner.update(cx, |state, cx| state.send_draft(&key, cx).unwrap());
    assert!(!reply.await.unwrap_or(false));
    assert_eq!(count(dir.path(), "prompt"), 0);
    owner.update(cx, |state, cx| {
        state.set_draft_content(&key, file_draft("/summary existing ", &file, "tail"), cx)
    });
    let reply = owner.update(cx, |state, cx| state.send_draft(&key, cx).unwrap());
    assert!(reply.await.unwrap());
    let lines = std::fs::read_to_string(dir.path().join("inputs.jsonl")).unwrap();
    let sent: serde_json::Value = serde_json::from_str(lines.lines().next().unwrap()).unwrap();
    assert_eq!(
        sent["message"],
        file_draft("/summary existing ", &file, "tail")
            .text()
            .as_ref()
    );
    assert_eq!(count(dir.path(), "prompt"), 1);
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn questionnaire_selection_survives_window_recreation_and_only_submit_replies(
    cx: &mut TestAppContext,
) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| state.can_submit(&key, cx))
        .await;
    init(cx);
    request(
        &owner,
        &key,
        serde_json::json!({"id":"choose","method":"select","title":"Choose","options":["Same","Same","Third", "A long answer with a description that should wrap inside the option card. ".repeat(8)]}),
        cx,
    );
    let selection = owner.read_with(cx, |state, _| {
        state.sessions[&key].pending_ui[0]
            .selection
            .clone()
            .unwrap()
    });
    for rebuild in [false, true] {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| HomeView::with_state(owner.clone(), window, cx));
            Root::new(view, window, cx)
        });
        visual.simulate_resize(size(px(1200.), px(800.)));
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.render_frame(cx);
            let viewport = window.find("extension-choices").bounds();
            let first = window
                .find(SharedString::from(format!(
                    "questionnaire-{}-choice-answer-0",
                    selection.entity_id()
                )))
                .bounds();
            // The component paints its focus ring 3px outside the card. The
            // scroll viewport must leave room on each exposed edge.
            assert!(first.origin.x - viewport.origin.x >= px(3.));
            assert!(first.origin.y - viewport.origin.y >= px(3.));
            assert!(viewport.right() - first.right() >= px(3.));
            let short = window
                .find(SharedString::from(format!(
                    "questionnaire-{}-choice-answer-1",
                    selection.entity_id()
                )))
                .bounds();
            let long = window
                .find(SharedString::from(format!(
                    "questionnaire-{}-choice-answer-3",
                    selection.entity_id()
                )))
                .bounds();
            assert!(
                long.size.height > short.size.height,
                "long labels must wrap within their card"
            );
            let submit =
                SharedString::from(format!("questionnaire-{}-Submit", selection.entity_id()));
            assert!(window.find(submit).visible());
            if !rebuild {
                window.click(
                    SharedString::from(format!(
                        "questionnaire-{}-choice-answer-1",
                        selection.entity_id()
                    )),
                    cx,
                );
            } else {
                assert_eq!(
                    selection.read(cx).answer("answer").unwrap().choices(),
                    &[SharedString::from("1")]
                );
                window.click(
                    SharedString::from(format!("questionnaire-{}-Submit", selection.entity_id())),
                    cx,
                );
            }
        });
        visual.run_until_parked();
        if !rebuild {
            assert_eq!(count(dir.path(), "extension_ui_response"), 0);
            visual.update(|_, cx| {
                assert_eq!(
                    owner.read_with(cx, |state, _| state.sessions[&key].pending_ui.len()),
                    1
                )
            });
        }
        visual.update(|window, _| window.remove_window());
    }
    cx.condition(&owner, |state, _| {
        state.sessions[&key].pending_ui.is_empty()
    })
    .await;
    // Wait for the write to the actual fixture process, independently of the
    // local request removal.
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    client
        .request_raw(
            serde_json::json!({"type":"wait_for","command":"extension_ui_response","count":"1"}),
        )
        .await
        .unwrap();
    let response: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(dir.path().join("ui-responses.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(response["value"], "Same");
    assert_eq!(response["id"], "choose");
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn extension_text_inputs_preserve_empty_spaces_and_multiline_values(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| state.can_submit(&key, cx))
        .await;
    init(cx);
    for (id, method, typed, expected) in [
        ("empty", "input", "", ""),
        ("spaces", "input", "   ", "   "),
        ("filled", "input", "  中文 answer  ", "  中文 answer  "),
        ("editor", "editor", "第二行", "预填\n第二行"),
    ] {
        request(
            &owner,
            &key,
            serde_json::json!({"id":id,"method":method,"title":id,"prefill":"预填"}),
            cx,
        );
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| HomeView::with_state(owner.clone(), window, cx));
            Root::new(view, window, cx)
        });
        visual.simulate_resize(size(px(1200.), px(800.)));
        visual.run_until_parked();
        if method == "editor" {
            visual.dispatch_action(input::MoveToEnd);
            visual.simulate_keystrokes("shift-enter");
        }
        visual.simulate_input(typed);
        visual.run_until_parked();
        visual.update(|window, cx| {
            assert_eq!(owner.read(cx).sessions[&key].pending_ui[0].text, expected);
            window.render_frame(cx);
            if id == "filled" {
                window.press("enter", cx);
            } else {
                window.click("extension-submit", cx);
            }
        });
        visual.run_until_parked();
        visual.update(|window, _| window.remove_window());
        cx.condition(&owner, |state, _| {
            state.sessions[&key].pending_ui.is_empty()
        })
        .await;
    }
    let client = owner.read_with(cx, |state, cx| state.client(&key, cx).unwrap());
    client
        .request_raw(
            serde_json::json!({"type":"wait_for","command":"extension_ui_response","count":"4"}),
        )
        .await
        .unwrap();
    let data = std::fs::read_to_string(dir.path().join("ui-responses.jsonl")).unwrap();
    let replies: Vec<serde_json::Value> = data
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for (reply, expected) in replies
        .iter()
        .zip(["", "   ", "  中文 answer  ", "预填\n第二行"])
    {
        assert_eq!(reply["value"], expected);
        assert!(reply.get("cancelled").is_none());
    }
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn file_picker_inserts_in_place_with_atomic_undo_and_preserves_cancelled_text(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestAppContextExt;
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| state.can_submit(&key, cx))
        .await;
    init(cx);
    std::fs::write(dir.path().join("report.txt"), "file content").unwrap();
    owner.update(cx, |state, cx| {
        state.set_draft(&key, "before  after".into(), cx)
    });
    let mut home = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| HomeView::with_state(owner.clone(), window, cx));
        home = Some(view.clone());
        Root::new(view, window, cx)
    });
    let home = home.unwrap();
    visual.simulate_resize(size(px(1200.), px(800.)));
    visual.update(|window, cx| {
        let input = home.read(cx).input.clone();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.set_selected_range(7..7, cx);
        });
    });
    visual.simulate_input("@");
    visual.run_until_parked();
    visual.simulate_input("report.txt");
    let handle = visual.update(|window, _| window.window_handle());
    visual
        .wait_for(handle, std::time::Duration::from_secs(3), |window, _| {
            window
                .try_find("command-send")
                .is_some_and(|button| button.label() == Some("Insert into message"))
        })
        .await;
    visual.simulate_keystrokes("enter");
    visual
        .condition(&owner, |state, _| {
            state.sessions[&key].draft.tokens().len() == 1
        })
        .await;
    let expected = file_draft("before ", &dir.path().join("report.txt"), "after");
    owner.read_with(visual, |state, _| {
        assert_eq!(state.sessions[&key].draft, expected);
        assert!(state.sessions[&key].attachments.is_empty());
    });
    visual.dispatch_action(input::Undo);
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert_eq!(state.sessions[&key].draft.text().as_ref(), "before @ after");
        assert!(state.sessions[&key].draft.tokens().is_empty());
    });
    visual.dispatch_action(input::Redo);
    visual.run_until_parked();
    visual.simulate_keystrokes("backspace");
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert_eq!(state.sessions[&key].draft.text().as_ref(), "before after");
    });
    visual.dispatch_action(input::Undo);
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert_eq!(state.sessions[&key].draft, expected)
    });
    visual.simulate_input("@");
    visual.run_until_parked();
    visual.simulate_input("cancelled-query");
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert_eq!(
            state.sessions[&key].draft.text().as_ref(),
            format!("{}@after", expected.text().strip_suffix("after").unwrap())
        );
    });
    assert_eq!(count(dir.path(), "prompt"), 0);
    visual.update(|window, _| window.remove_window());
    close(&owner, cx).await;
}

#[gpui_kit::test]
async fn image_reference_and_pasted_files_use_distinct_send_paths(cx: &mut TestAppContext) {
    let (dir, owner, key) = begin(cx, &[]);
    cx.condition(&owner, |state, cx| state.can_submit(&key, cx))
        .await;
    init(cx);
    let picture = dir.path().join("image.png");
    image::DynamicImage::new_rgb8(2, 3).save(&picture).unwrap();
    let first = dir.path().join("first 文件.txt");
    let second = dir.path().join("second.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    owner.update(cx, |state, cx| {
        state.set_draft(&key, "before  after".into(), cx)
    });
    let mut home = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| HomeView::with_state(owner.clone(), window, cx));
        home = Some(view.clone());
        Root::new(view, window, cx)
    });
    let home = home.unwrap();
    visual.simulate_resize(size(px(1200.), px(800.)));
    visual.update(|window, cx| {
        home.read(cx).input.clone().update(cx, |input, cx| {
            input.focus(window, cx);
            input.set_selected_range(7..7, cx);
        });
        // This is the same confirmation used by the @ project picker.
        home.update(cx, |home, cx| home.select_file(picture.clone(), window, cx));
    });
    visual.run_until_parked();
    owner.read_with(visual, |state, _| {
        assert!(state.sessions[&key].attachments.is_empty());
        assert_eq!(
            state.sessions[&key].draft,
            file_draft("before ", &picture, "after")
        );
    });
    visual.update(|_, cx| {
        cx.write_to_clipboard(gpui_kit::ClipboardItem {
            entries: vec![gpui_kit::ClipboardEntry::ExternalPaths(
                gpui_kit::ExternalPaths(
                    vec![first.clone(), picture.clone(), second.clone()].into(),
                ),
            )],
        });
    });
    visual.dispatch_action(input::Paste);
    visual
        .condition(&owner, |state, _| {
            state.sessions[&key].attachments.len() == 1
        })
        .await;
    let expected = owner.read_with(visual, |state, _| {
        let session = &state.sessions[&key];
        assert_eq!(session.draft.tokens().len(), 3);
        let paths: Vec<_> = session
            .draft
            .tokens()
            .iter()
            .map(|span| {
                crate::foundation::composer_resources::file_path(span.token())
                    .unwrap()
                    .to_path_buf()
            })
            .collect();
        assert_eq!(paths, vec![picture.clone(), first.clone(), second.clone()]);
        assert_eq!(session.attachments[0].name, "image.png");
        session.draft.text().to_string()
    });
    assert!(expected.ends_with(" after"));
    let reply = owner.update(visual, |state, cx| state.send_draft(&key, cx).unwrap());
    assert!(reply.await.unwrap());
    let lines = std::fs::read_to_string(dir.path().join("inputs.jsonl")).unwrap();
    let sent: serde_json::Value = serde_json::from_str(lines.lines().last().unwrap()).unwrap();
    assert_eq!(sent["message"], expected);
    assert_eq!(sent["images"].as_array().unwrap().len(), 1);
    visual.update(|window, _| window.remove_window());
    close(&owner, cx).await;
}
