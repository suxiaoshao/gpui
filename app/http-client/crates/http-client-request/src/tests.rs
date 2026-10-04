use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use gpui_kit::TestAppContext;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use super::*;
use crate::localization::init_i18n;
use crate::{
    response::{
        BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
        StoredBody,
    },
    runtime::{HttpRunMessage, RequestPhase},
};

fn initialize(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
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
        ResponseTiming::new(Duration::from_millis(1), Duration::from_millis(2)),
        CompletedBody::new(
            StoredBody::Memory(Bytes::from_static(bytes)),
            BodyDecoding::Identity,
            ResponseSizes::new(
                Some(bytes.len() as u64),
                bytes.len() as u64,
                bytes.len() as u64,
            ),
        ),
    ))
}

#[gpui_kit::test]
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

#[gpui_kit::test]
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

#[gpui_kit::test]
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

#[gpui_kit::test]
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

#[gpui_kit::test]
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
            view.start_request(window, cx);
            assert_eq!(view.runtime.phase(), RequestPhase::Sending);
            assert!(view.runtime.response().is_none());
            view.cancel_request(cx);
        });
    });
}

#[gpui_kit::test]
fn response_save_disables_send_until_the_save_task_finishes(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);

    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            assert!(!view.send_is_disabled());

            view.set_save_activity(true, cx);
            assert!(view.send_is_disabled());

            view.set_save_activity(false, cx);
            assert!(!view.send_is_disabled());
        });
    });
}
