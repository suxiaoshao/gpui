use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use gpui::{RenderImage, Task, TestAppContext};
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use super::{
    MAX_PDF_IMAGE_BYTES, MAX_PDF_PAGE_COUNT, MAX_PDF_PAGE_DIMENSION, PdfProblemKind, PdfViewport,
    worker::{
        PdfWorkerEvent, PdfWorkerHandle, contain_dimensions, reserve_image_budget, rgba_to_bgra,
        validate_page_count,
    },
};
use crate::features::request::response::{
    BodyDecoding, CompletedBody, PreviewToken, ResponseData, ResponseHead, ResponseSizes,
    ResponseTiming, StoredBody, ViewerMode,
};

#[test]
fn contain_dimensions_preserves_the_source_aspect_ratio_within_the_viewport() {
    assert_eq!(
        contain_dimensions(2_000.0, 1_000.0, PdfViewport::new(800, 600)).unwrap(),
        (800, 400)
    );
    assert_eq!(
        contain_dimensions(1_000.0, 2_000.0, PdfViewport::new(800, 600)).unwrap(),
        (300, 600)
    );
}

#[test]
fn contain_dimensions_rejects_invalid_and_unrepresentable_source_dimensions() {
    assert_eq!(
        contain_dimensions(f32::NAN, 10.0, PdfViewport::new(100, 100))
            .unwrap_err()
            .kind(),
        PdfProblemKind::Budget
    );
    assert_eq!(
        contain_dimensions(10.0, 10.0, PdfViewport::new(0, 100))
            .unwrap_err()
            .kind(),
        PdfProblemKind::Budget
    );
}

#[test]
fn image_budget_checks_checked_product_and_limits() {
    assert!(reserve_image_budget(MAX_PDF_PAGE_DIMENSION, MAX_PDF_PAGE_DIMENSION).is_ok());
    assert_eq!(
        reserve_image_budget(MAX_PDF_PAGE_DIMENSION + 1, 1)
            .unwrap_err()
            .kind(),
        PdfProblemKind::Budget
    );
    assert_eq!(MAX_PDF_IMAGE_BYTES, 64 * 1024 * 1024);
}

#[test]
fn rgba_to_bgra_preserves_premultiplied_alpha_without_allocating() {
    let mut pixels = [1, 2, 3, 4, 50, 60, 70, 80];
    rgba_to_bgra(&mut pixels);
    assert_eq!(pixels, [3, 2, 1, 4, 70, 60, 50, 80]);
}

#[tokio::test]
async fn worker_parses_renders_and_navigates_a_generated_two_page_document() {
    let token = token(1);
    let mut worker = PdfWorkerHandle::new(generated_pdf(2, false).into()).unwrap();
    let events = worker.take_event_receiver().unwrap();
    worker
        .load(token.clone(), 0, PdfViewport::new(320, 240))
        .unwrap();

    let first = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    let PdfWorkerEvent::Loaded {
        page_count,
        page,
        image,
        ..
    } = first
    else {
        panic!("generated PDF did not load")
    };
    assert_eq!(page_count, 2);
    assert_eq!(page, 0);
    assert_eq!(image.frame_count(), 1);

    worker
        .render(token, 1, 1, PdfViewport::new(320, 240))
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        second,
        PdfWorkerEvent::Rendered {
            page_generation: 1,
            page: 1,
            ..
        }
    ));
}

#[tokio::test]
async fn worker_eventually_processes_the_latest_page_request_after_stale_renders() {
    let token = token(3);
    let mut worker = PdfWorkerHandle::new(generated_pdf(4, false).into()).unwrap();
    let events = worker.take_event_receiver().unwrap();
    let viewport = PdfViewport::new(320, 240);
    worker.load(token.clone(), 0, viewport).unwrap();
    let initial = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(initial, PdfWorkerEvent::Loaded { page: 0, .. }));

    // A render may already be in progress when newer requests replace the
    // pending mailbox entry. Its completion is allowed to arrive first; the
    // response owner filters that stale generation. The worker must still
    // eventually process the latest request.
    worker.render(token.clone(), 1, 1, viewport).unwrap();
    worker.render(token.clone(), 2, 2, viewport).unwrap();
    worker.render(token, 3, 3, viewport).unwrap();

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            match event {
                PdfWorkerEvent::Rendered {
                    page_generation: 3,
                    page: 3,
                    ..
                } => break,
                PdfWorkerEvent::Rendered {
                    page_generation,
                    page,
                    ..
                } => {
                    assert!(page_generation < 3);
                    assert_eq!(page, page_generation as usize);
                }
                PdfWorkerEvent::Loaded { .. } => {
                    panic!("the initial load event should have been consumed")
                }
                PdfWorkerEvent::Failed { .. } => panic!("page render unexpectedly failed"),
            }
        }
    })
    .await
    .unwrap();
}

#[test]
fn worker_event_receiver_has_exactly_one_owner() {
    let mut worker = PdfWorkerHandle::new(generated_pdf(1, false).into()).unwrap();
    let _events = worker.take_event_receiver().unwrap();

    assert_eq!(
        worker.take_event_receiver().unwrap_err().kind(),
        PdfProblemKind::Internal
    );
}

#[tokio::test]
async fn worker_reports_invalid_and_encrypted_documents_without_exposing_parser_details() {
    for (bytes, expected) in [
        (b"not a PDF".to_vec(), PdfProblemKind::Parse),
        (generated_pdf(1, true), PdfProblemKind::Encrypted),
    ] {
        let token = token(2);
        let mut worker = PdfWorkerHandle::new(bytes.into()).unwrap();
        let events = worker.take_event_receiver().unwrap();
        worker.load(token, 0, PdfViewport::new(320, 240)).unwrap();
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        let PdfWorkerEvent::Failed { problem, .. } = event else {
            panic!("invalid PDF unexpectedly rendered")
        };
        assert_eq!(problem.kind(), expected);
        assert!(!format!("{problem:?}").contains("not a PDF"));
    }
}

#[test]
fn page_count_budget_rejects_empty_and_excessive_documents() {
    assert_eq!(
        validate_page_count(0).unwrap_err().kind(),
        PdfProblemKind::Budget
    );
    assert!(validate_page_count(MAX_PDF_PAGE_COUNT).is_ok());
    assert_eq!(
        validate_page_count(MAX_PDF_PAGE_COUNT + 1)
            .unwrap_err()
            .kind(),
        PdfProblemKind::Budget
    );
}

#[test]
fn reading_the_complete_response_is_a_loading_preview_phase() {
    let mut preview = super::PdfPreview::new();
    assert!(!preview.is_loading());

    preview.begin_read(token(1));

    assert!(preview.is_loading());
}

#[gpui::test]
fn stale_token_and_page_generation_cannot_replace_the_latest_pdf_page(cx: &mut TestAppContext) {
    let preview_token = token(1);
    let stale_token = token(2);
    let mut preview = super::PdfPreview::new();
    preview.begin_read(preview_token.clone());

    let worker = PdfWorkerHandle::new(generated_pdf(2, false).into()).unwrap();
    preview.load(
        preview_token.clone(),
        worker,
        owner_task(cx, Arc::new(AtomicBool::new(false))),
        PdfViewport::new(320, 240),
    );
    preview.handle_event(PdfWorkerEvent::Loaded {
        token: preview_token.clone(),
        page_generation: 0,
        page_count: 2,
        page: 0,
        image: image(),
    });
    assert_eq!(preview.current_page(), Some(0));

    preview.render_page(1, PdfViewport::new(320, 240));
    preview.render_page(0, PdfViewport::new(640, 480));
    assert_eq!(preview.current_page(), Some(0));
    assert_eq!(preview.viewport(), Some(PdfViewport::new(640, 480)));

    preview.handle_event(PdfWorkerEvent::Rendered {
        token: stale_token,
        page_generation: 2,
        page: 0,
        image: image(),
    });
    preview.handle_event(PdfWorkerEvent::Rendered {
        token: preview_token.clone(),
        page_generation: 1,
        page: 1,
        image: image(),
    });
    assert_eq!(preview.current_page(), Some(0));
    assert!(preview.is_loading());

    preview.handle_event(PdfWorkerEvent::Rendered {
        token: preview_token,
        page_generation: 2,
        page: 0,
        image: image(),
    });
    assert!(!preview.is_loading());
    assert_eq!(preview.current_page(), Some(0));
    assert_eq!(preview.viewport(), Some(PdfViewport::new(640, 480)));
}

#[gpui::test]
fn stop_and_owner_drop_cancel_the_route_and_stabilize_the_pdf_worker(cx: &mut TestAppContext) {
    for stop_explicitly in [true, false] {
        let route_dropped = Arc::new(AtomicBool::new(false));
        let mut preview = loading_preview(cx, Arc::clone(&route_dropped));
        let stopped = match &preview.runtime {
            super::PdfRuntime::Loading(state) => state.worker.stop_probe(),
            _ => panic!("PDF test setup must install the loading runtime"),
        };

        if stop_explicitly {
            preview.stop();
            assert_eq!(preview.runtime.phase(), super::PdfPhase::Idle);
        } else {
            drop(preview);
        }

        cx.run_until_parked();
        assert!(route_dropped.load(Ordering::Acquire));
        assert!(stopped.load(Ordering::Acquire));
    }
}

fn loading_preview(cx: &mut TestAppContext, route_dropped: Arc<AtomicBool>) -> super::PdfPreview {
    let preview_token = token(1);
    let mut preview = super::PdfPreview::new();
    preview.begin_read(preview_token.clone());
    preview.load(
        preview_token,
        PdfWorkerHandle::new(generated_pdf(1, false).into()).unwrap(),
        owner_task(cx, route_dropped),
        PdfViewport::new(320, 240),
    );
    preview
}

fn owner_task(cx: &mut TestAppContext, route_dropped: Arc<AtomicBool>) -> Task<()> {
    let route_drop = RouteDrop(route_dropped);
    cx.update(|cx| {
        cx.spawn(async move |_| {
            let _route_dropped = route_drop;
            std::future::pending::<()>().await;
        })
    })
}

struct RouteDrop(Arc<AtomicBool>);

impl Drop for RouteDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_raw(1, 1, vec![0, 0, 0, 255]).unwrap(),
    )]))
}

fn token(generation: u64) -> PreviewToken {
    let response = Arc::new(ResponseData::new(
        ResponseHead::new(
            StatusCode::OK,
            Version::HTTP_11,
            Url::parse("https://example.test/private?token=value").unwrap(),
            HeaderMap::new(),
        ),
        ResponseTiming {
            head_after: Duration::ZERO,
            completed_after: Duration::ZERO,
        },
        CompletedBody {
            body: StoredBody::Memory(Bytes::new()),
            body_decoding: BodyDecoding::Identity,
            sizes: ResponseSizes {
                declared_encoded_bytes: Some(0),
                received_encoded_bytes: 0,
                stored_body_bytes: 0,
            },
        },
    ));
    PreviewToken::new(response, ViewerMode::Pdf, ViewerMode::Pdf, generation)
}

fn generated_pdf(page_count: usize, encrypted: bool) -> Vec<u8> {
    let mut objects = Vec::new();
    let page_objects = (0..page_count)
        .map(|index| 3 + index * 2)
        .collect::<Vec<_>>();
    let kids = page_objects
        .iter()
        .map(|object| format!("{object} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    objects.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"
    ));
    for (index, page_object) in page_objects.into_iter().enumerate() {
        let content_object = page_object + 1;
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << >> /Contents {content_object} 0 R >>"
        ));
        let red = (index % 10) as f32 / 10.0;
        let stream = format!("q {red:.1} 0.2 0.8 rg 10 10 50 50 re f Q");
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len()
        ));
    }

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    let encryption = if encrypted {
        " /Encrypt << /Filter /Standard /V 1 /R 2 /Length 40 /O (owner) /U (user) /P -4 >> /ID [(fixture)(fixture)]"
    } else {
        ""
    };
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R{encryption} >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}
