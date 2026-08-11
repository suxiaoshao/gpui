use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use gpui::TestAppContext;
use gpui_component::select::SelectEvent;
use gpui_operation::Transition as _;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use super::*;
use crate::features::request::{
    response::{
        BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
        StoredBody,
    },
    runtime::{HttpRunMessage, RequestPhase},
};
use crate::foundation::i18n::init_i18n;

fn initialize(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init_i18n(cx);
        gpui_tokio::init(cx);
    });
}

fn completed_response(bytes: &'static [u8]) -> Arc<ResponseData> {
    Arc::new(ResponseData::new(
        ResponseHead::new(
            StatusCode::OK,
            Version::HTTP_11,
            Url::parse("https://example.test/response").unwrap(),
            HeaderMap::new(),
        ),
        ResponseTiming {
            head_after: Duration::from_millis(1),
            completed_after: Duration::from_millis(2),
        },
        CompletedBody {
            body: StoredBody::Memory(Bytes::from_static(bytes)),
            body_decoding: BodyDecoding::Identity,
            sizes: ResponseSizes {
                declared_encoded_bytes: Some(bytes.len() as u64),
                received_encoded_bytes: bytes.len() as u64,
                stored_body_bytes: bytes.len() as u64,
            },
        },
    ))
}

#[gpui::test]
fn page_prepare_uses_the_form_snapshot_without_rewriting_the_editor(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());
    let original = " https://example.test/items?q=one#local-fragment ".to_owned();

    cx.update(|_, cx| RequestDraft::URL.set(&form, original.clone(), cx));
    let prepared = cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.prepare_request(cx)
                .expect("valid request must prepare")
        })
    });

    assert_eq!(prepared.url.as_str(), "https://example.test/items?q=one");
    cx.update(|_, cx| {
        assert_eq!(RequestDraft::URL.get(&form, cx), original);
    });
}

#[gpui::test]
fn page_prepare_reports_submit_errors_on_the_precise_url_path(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());

    assert!(
        cx.update(|_, cx| view.update(cx, |view, cx| view.prepare_request(cx)))
            .is_err()
    );
    cx.update(|_, cx| {
        let issues = RequestDraft::URL.errors(&form, cx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code(), "required");
    });
}

#[gpui::test]
fn running_send_is_rejected_before_submit_validation_or_a_second_task(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let task = cx.spawn(async |_, _| std::future::pending::<()>().await);
            (&mut view.runtime).transition(HttpRunMessage::Start {
                task,
                started_at: std::time::Instant::now(),
            });
            view.start_request(window, cx);
        });
    });

    cx.update(|_, cx| {
        assert_eq!(view.read(cx).runtime.phase(), RequestPhase::Sending);
        assert!(RequestDraft::URL.errors(&form, cx).is_empty());
        view.update(cx, |view, cx| view.cancel_request(cx));
    });
}

#[gpui::test]
fn prepare_failure_preserves_the_current_terminal_response(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let response = completed_response(b"previous");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: response.clone(),
            };
            view.start_request(window, cx);
            assert!(
                view.runtime
                    .response()
                    .is_some_and(|current| Arc::ptr_eq(current, &response))
            );
        });
    });
}

#[gpui::test]
fn accepted_send_clears_the_previous_response_before_worker_poll(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());
    cx.update(|_, cx| RequestDraft::URL.set(&form, "http://127.0.0.1:9".into(), cx));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let previous = completed_response(b"old");
            view.runtime = RequestRuntime::Ready {
                response: previous.clone(),
            };
            let stale_preview = view.response_pane.begin_preview(
                previous,
                crate::features::request::response::ViewerMode::Audio,
                window,
                cx,
            );
            let save = cx.spawn(async |_, _| std::future::pending::<()>().await);
            view.response_pane.install_save_task(save);
            view.start_request(window, cx);
            assert_eq!(view.runtime.phase(), RequestPhase::Sending);
            assert!(view.runtime.response().is_none());
            assert!(!view.response_pane.is_current_preview(&stale_preview));
            assert_eq!(
                view.response_pane.mode(),
                crate::features::request::response::ViewerMode::Auto
            );
            assert!(view.response_pane.save_is_running());
            view.cancel_request(cx);
        });
    });
}

#[gpui::test]
fn clear_response_invalidates_the_current_preview(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let response = completed_response(b"body");

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: response.clone(),
            };
            let stale_preview = view.response_pane.begin_preview(
                response,
                crate::features::request::response::ViewerMode::Pdf,
                window,
                cx,
            );

            view.clear_response(cx);

            assert_eq!(view.runtime.phase(), RequestPhase::Idle);
            assert!(!view.response_pane.is_current_preview(&stale_preview));
        });
    });
}

#[gpui::test]
fn response_save_picker_cancel_is_silent_and_releases_its_task(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: completed_response(b"body"),
            };
            view.start_response_save(window, cx);
            assert!(view.response_pane.save_is_running());
        });
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(!view.read(cx).response_pane.save_is_running());
    });
}

#[gpui::test]
fn response_view_mode_select_invalidates_only_the_previous_viewer_projection(
    cx: &mut TestAppContext,
) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let response = completed_response(b"body");
    let stale_preview = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: response.clone(),
            };
            view.response_pane.begin_preview(
                response,
                crate::features::request::response::ViewerMode::Hex,
                window,
                cx,
            )
        })
    });
    let mode_state = cx.update(|_, cx| view.read(cx).response_pane.mode_state.clone());
    cx.update(|_, cx| {
        mode_state.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some(
                crate::features::request::response::ViewerMode::Pdf,
            )));
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).response_pane.mode(),
            crate::features::request::response::ViewerMode::Pdf
        );
        assert!(
            !view
                .read(cx)
                .response_pane
                .is_current_preview(&stale_preview)
        );
        assert_eq!(view.read(cx).runtime.phase(), RequestPhase::Ready);
    });
}
