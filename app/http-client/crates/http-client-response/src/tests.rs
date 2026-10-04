use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use gpui_kit::TestAppContext;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use super::*;
use crate::localization::init_i18n;
use crate::{
    BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
    StoredBody,
};
use gpui_kit::component::select::SelectEvent;

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
fn clear_response_invalidates_the_current_preview(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(ResponseView::new);
    let response = completed_response(b"body");

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: response.clone(),
            };
            let stale_preview =
                view.response_pane
                    .begin_preview(response, ViewerMode::Pdf, window, cx);

            view.set_state(RequestRuntime::Idle, window, cx);

            assert!(matches!(view.runtime, RequestRuntime::Idle));
            assert!(!view.response_pane.is_current_preview(&stale_preview));
        });
    });
}

#[gpui_kit::test]
fn response_save_picker_cancel_is_silent_and_releases_its_task(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(ResponseView::new);
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

#[gpui_kit::test]
fn response_view_mode_select_invalidates_only_the_previous_viewer_projection(
    cx: &mut TestAppContext,
) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(ResponseView::new);
    let response = completed_response(b"body");
    let stale_preview = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.runtime = RequestRuntime::Ready {
                response: response.clone(),
            };
            view.response_pane
                .begin_preview(response, ViewerMode::Hex, window, cx)
        })
    });
    let mode_state = cx.update(|_, cx| view.read(cx).response_pane.mode_state.clone());
    cx.update(|_, cx| {
        mode_state.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some(ViewerMode::Pdf)));
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).response_pane.mode(), ViewerMode::Pdf);
        assert!(
            !view
                .read(cx)
                .response_pane
                .is_current_preview(&stale_preview)
        );
        assert!(matches!(
            view.read(cx).runtime,
            RequestRuntime::Ready { .. }
        ));
    });
}

#[gpui_kit::test]
fn new_request_invalidates_preview_without_cancelling_an_independent_save(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(ResponseView::new);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let previous = completed_response(b"previous");
            view.runtime = RequestRuntime::Ready {
                response: previous.clone(),
            };
            let stale = view
                .response_pane
                .begin_preview(previous, ViewerMode::Hex, window, cx);
            let save = cx.spawn(async |_, _| std::future::pending::<()>().await);
            view.response_pane.install_save_task(save);
            view.set_state(RequestRuntime::Sending, window, cx);
            assert!(view.runtime.response().is_none());
            assert!(!view.response_pane.is_current_preview(&stale));
            assert_eq!(view.response_pane.mode(), ViewerMode::Auto);
            assert!(view.save_is_running());
        })
    });
}
