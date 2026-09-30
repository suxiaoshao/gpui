use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use super::{
    MAX_PDF_IMAGE_BYTES, MAX_PDF_PAGE_COUNT, MAX_PDF_PAGE_DIMENSION, PdfProblemKind, PdfViewport,
    worker::{contain_dimensions, reserve_image_budget, rgba_to_bgra, validate_page_count},
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
