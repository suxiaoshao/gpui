use crate::{
    OcrError,
    capture::ScreenRect,
    ocr::{OcrLanguage, OcrLine, OcrRequest, OcrResult, OcrWord, compare_lines},
};
use windows::{
    Globalization::Language,
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    core::{HSTRING, Interface},
};

use crate::capture::windows::{
    async_support::{wait_async_operation, wait_async_operation_with_progress},
    image_frame::software_bitmap_from_image_frame,
};

const AI_LANGUAGE_SELECTION_UNAVAILABLE: &str =
    "windows ai text recognizer does not support explicit language selection";
const AI_BACKEND_UNAVAILABLE: &str = "windows ai text recognizer is unavailable on this system";
const LEGACY_BACKEND_UNAVAILABLE: &str = "windows media ocr is unavailable on this system";
const LEGACY_LANGUAGE_UNAVAILABLE: &str =
    "none of the requested OCR languages are supported on this system";

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::all
)]
mod ai_bindings {
    include!(concat!(env!("OUT_DIR"), "/windows_ai_bindings.rs"));
}

enum BackendError {
    Unavailable(&'static str),
    Failure(String),
}

pub(super) fn recognize_text(request: OcrRequest) -> Result<OcrResult, OcrError> {
    let bitmap =
        software_bitmap_from_image_frame(&request.image).map_err(OcrError::SystemFailure)?;
    resolve_backend_result(recognize_text_ai(&bitmap, &request), || {
        recognize_text_legacy(&bitmap, &request)
    })
}

fn recognize_text_ai(bitmap: &SoftwareBitmap, request: &OcrRequest) -> Result<OcrResult, BackendError> {
    if !request.languages.is_empty() {
        return Err(BackendError::Unavailable(AI_LANGUAGE_SELECTION_UNAVAILABLE));
    }

    ensure_ai_ready()?;
    let recognizer =
        wait_async_operation(ai_bindings::TextRecognizer::CreateAsync().map_err(ai_failure)?)
            .map_err(ai_failure)?;
    let ai_bitmap: ai_bindings::SoftwareBitmap = bitmap.cast().map_err(ai_failure)?;
    let image_buffer =
        ai_bindings::ImageBuffer::CreateForSoftwareBitmap(&ai_bitmap).map_err(ai_failure)?;
    let recognized = recognizer
        .RecognizeTextFromImage(&image_buffer)
        .map_err(ai_failure)?;

    let mut lines = recognized
        .Lines()
        .map_err(ai_failure)?
        .iter()
        .flatten()
        .map(|line| {
            let words = if request.include_word_boxes {
                line.Words()
                    .map_err(ai_failure)?
                    .iter()
                    .flatten()
                    .map(|word| {
                        Ok(OcrWord {
                            text: word.Text().map_err(ai_failure)?.to_string(),
                            bounds: ai_bounding_box_to_rect(word.BoundingBox().map_err(ai_failure)?),
                            confidence: Some(word.MatchConfidence().map_err(ai_failure)?),
                        })
                    })
                    .collect::<Result<Vec<_>, BackendError>>()?
            } else {
                Vec::new()
            };
            Ok(OcrLine {
                text: line.Text().map_err(ai_failure)?.to_string(),
                bounds: ai_bounding_box_to_rect(line.BoundingBox().map_err(ai_failure)?),
                words,
            })
        })
        .collect::<Result<Vec<_>, BackendError>>()?;
    lines.sort_by(compare_lines);
    Ok(OcrResult::from_lines(lines))
}

fn ensure_ai_ready() -> Result<(), BackendError> {
    match ai_bindings::TextRecognizer::GetReadyState().map_err(ai_failure)? {
        ai_bindings::AIFeatureReadyState::Ready => Ok(()),
        ai_bindings::AIFeatureReadyState::NotSupportedOnCurrentSystem
        | ai_bindings::AIFeatureReadyState::DisabledByUser => {
            Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE))
        }
        ai_bindings::AIFeatureReadyState::NotReady => {
            let ready_result = wait_async_operation_with_progress(
                ai_bindings::TextRecognizer::EnsureReadyAsync().map_err(ai_failure)?,
            )
            .map_err(ai_failure)?;
            match ready_result.Status().map_err(ai_failure)? {
                ai_bindings::AIFeatureReadyResultState::Success => Ok(()),
                ai_bindings::AIFeatureReadyResultState::InProgress
                | ai_bindings::AIFeatureReadyResultState::Failure => {
                    Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE))
                }
                _ => Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE)),
            }
        }
        _ => Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE)),
    }
}

fn recognize_text_legacy(bitmap: &SoftwareBitmap, request: &OcrRequest) -> Result<OcrResult, BackendError> {
    let bitmap = SoftwareBitmap::ConvertWithAlpha(bitmap, BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore)
        .map_err(legacy_failure)?;
    let engine = select_legacy_engine(&request.languages)?;
    let result =
        wait_async_operation(engine.RecognizeAsync(&bitmap).map_err(legacy_failure)?).map_err(legacy_failure)?;

    let mut lines = Vec::new();
    let lines_view = result.Lines().map_err(legacy_failure)?;
    for index in 0..lines_view.Size().map_err(legacy_failure)? {
        let line = lines_view.GetAt(index).map_err(legacy_failure)?;
        let words = line
            .Words()
            .map_err(legacy_failure)?
            .into_iter()
            .map(|word| {
                Ok(OcrWord {
                    text: word.Text().map_err(legacy_failure)?.to_string(),
                    bounds: foundation_rect_to_screen_rect(word.BoundingRect().map_err(legacy_failure)?),
                    confidence: None,
                })
            })
            .collect::<Result<Vec<_>, BackendError>>()?;
        let bounds = union_word_bounds(&words);
        lines.push(OcrLine {
            text: line.Text().map_err(legacy_failure)?.to_string(),
            bounds,
            words: if request.include_word_boxes {
                words
            } else {
                Vec::new()
            },
        });
    }

    lines.sort_by(compare_lines);
    Ok(OcrResult::from_lines(lines))
}

fn select_legacy_engine(languages: &[OcrLanguage]) -> Result<OcrEngine, BackendError> {
    if languages.is_empty() {
        return OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| BackendError::Unavailable(LEGACY_BACKEND_UNAVAILABLE));
    }

    for language in languages {
        let tag = HSTRING::from(language_tag(*language));
        let language = Language::CreateLanguage(&tag).map_err(legacy_failure)?;
        if OcrEngine::IsLanguageSupported(&language).map_err(legacy_failure)? {
            return OcrEngine::TryCreateFromLanguage(&language)
                .map_err(|_| BackendError::Unavailable(LEGACY_BACKEND_UNAVAILABLE));
        }
    }

    Err(BackendError::Unavailable(LEGACY_LANGUAGE_UNAVAILABLE))
}

fn ai_bounding_box_to_rect(bounds: ai_bindings::RecognizedTextBoundingBox) -> ScreenRect {
    let xs = [
        f64::from(bounds.TopLeft.X),
        f64::from(bounds.TopRight.X),
        f64::from(bounds.BottomLeft.X),
        f64::from(bounds.BottomRight.X),
    ];
    let ys = [
        f64::from(bounds.TopLeft.Y),
        f64::from(bounds.TopRight.Y),
        f64::from(bounds.BottomLeft.Y),
        f64::from(bounds.BottomRight.Y),
    ];
    let min_x = xs.into_iter().fold(f64::INFINITY, f64::min);
    let max_x = xs.into_iter().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.into_iter().fold(f64::INFINITY, f64::min);
    let max_y = ys.into_iter().fold(f64::NEG_INFINITY, f64::max);
    ScreenRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn foundation_rect_to_screen_rect(rect: windows::Foundation::Rect) -> ScreenRect {
    ScreenRect::new(
        f64::from(rect.X),
        f64::from(rect.Y),
        f64::from(rect.Width),
        f64::from(rect.Height),
    )
}

fn union_word_bounds(words: &[OcrWord]) -> ScreenRect {
    if words.is_empty() {
        return ScreenRect::default();
    }

    let min_x = words.iter().map(|word| word.bounds.x).fold(f64::INFINITY, f64::min);
    let min_y = words.iter().map(|word| word.bounds.y).fold(f64::INFINITY, f64::min);
    let max_x = words
        .iter()
        .map(|word| word.bounds.x + word.bounds.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = words
        .iter()
        .map(|word| word.bounds.y + word.bounds.height)
        .fold(f64::NEG_INFINITY, f64::max);
    ScreenRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn language_tag(language: OcrLanguage) -> &'static str {
    match language {
        OcrLanguage::English => "en-US",
        OcrLanguage::SimplifiedChinese => "zh-Hans",
        OcrLanguage::TraditionalChinese => "zh-Hant",
        OcrLanguage::Japanese => "ja-JP",
        OcrLanguage::Korean => "ko-KR",
    }
}

fn resolve_backend_result(
    primary: Result<OcrResult, BackendError>,
    fallback: impl FnOnce() -> Result<OcrResult, BackendError>,
) -> Result<OcrResult, OcrError> {
    match primary {
        Ok(result) => Ok(result),
        Err(BackendError::Unavailable(_)) => match fallback() {
            Ok(result) => Ok(result),
            Err(BackendError::Unavailable(reason)) => Err(OcrError::BackendUnavailable(reason)),
            Err(BackendError::Failure(message)) => Err(OcrError::SystemFailure(message)),
        },
        Err(BackendError::Failure(message)) => Err(OcrError::SystemFailure(message)),
    }
}

fn ai_failure(error: windows_core::Error) -> BackendError {
    BackendError::Failure(error.to_string())
}

fn legacy_failure(error: windows_core::Error) -> BackendError {
    BackendError::Failure(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        BackendError, ai_bindings, ai_bounding_box_to_rect, foundation_rect_to_screen_rect,
        language_tag, resolve_backend_result, union_word_bounds,
    };
    use crate::{
        capture::ScreenRect,
        ocr::{OcrLanguage, OcrResult, OcrWord},
    };
    use std::cell::Cell;

    #[test]
    fn language_tags_match_windows_expectations() {
        assert_eq!(language_tag(OcrLanguage::English), "en-US");
        assert_eq!(language_tag(OcrLanguage::SimplifiedChinese), "zh-Hans");
    }

    #[test]
    fn union_word_bounds_uses_extents() {
        let bounds = union_word_bounds(&[
            OcrWord {
                text: "left".into(),
                bounds: ScreenRect::new(10.0, 8.0, 12.0, 6.0),
                confidence: None,
            },
            OcrWord {
                text: "right".into(),
                bounds: ScreenRect::new(28.0, 9.0, 7.0, 5.0),
                confidence: None,
            },
        ]);

        assert_eq!(bounds, ScreenRect::new(10.0, 8.0, 25.0, 6.0));
    }

    #[test]
    fn foundation_rect_maps_directly() {
        let rect = foundation_rect_to_screen_rect(windows::Foundation::Rect {
            X: 4.0,
            Y: 6.0,
            Width: 10.0,
            Height: 12.0,
        });

        assert_eq!(rect, ScreenRect::new(4.0, 6.0, 10.0, 12.0));
    }

    #[test]
    fn ai_bounding_box_maps_quad_to_axis_aligned_rect() {
        let rect = ai_bounding_box_to_rect(ai_bindings::RecognizedTextBoundingBox {
            TopLeft: ai_bindings::Point { X: 12.0, Y: 18.0 },
            TopRight: ai_bindings::Point { X: 22.0, Y: 16.0 },
            BottomLeft: ai_bindings::Point { X: 11.0, Y: 29.0 },
            BottomRight: ai_bindings::Point { X: 24.0, Y: 31.0 },
        });

        assert_eq!(rect, ScreenRect::new(11.0, 16.0, 13.0, 15.0));
    }

    #[test]
    fn resolve_backend_result_falls_back_only_for_unavailable_primary() {
        let fallback_called = Cell::new(false);
        let result = resolve_backend_result(Err(BackendError::Unavailable("ai unavailable")), || {
            fallback_called.set(true);
            Ok(OcrResult::from_lines(Vec::new()))
        })
        .expect("fallback should succeed");

        assert!(fallback_called.get());
        assert_eq!(result.text, "");
    }

    #[test]
    fn resolve_backend_result_does_not_fallback_for_primary_failure() {
        let fallback_called = Cell::new(false);
        let result = resolve_backend_result(Err(BackendError::Failure("boom".into())), || {
            fallback_called.set(true);
            Ok(OcrResult::from_lines(Vec::new()))
        });

        assert!(!fallback_called.get());
        assert_eq!(result, Err(crate::OcrError::SystemFailure("boom".into())));
    }

    #[test]
    fn resolve_backend_result_uses_fallback_backend_unavailable_reason() {
        let result = resolve_backend_result(Err(BackendError::Unavailable("ai unavailable")), || {
            Err(BackendError::Unavailable("legacy unavailable"))
        });

        assert_eq!(result, Err(crate::OcrError::BackendUnavailable("legacy unavailable")));
    }
}
