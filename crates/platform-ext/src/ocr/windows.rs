use crate::{
    OcrError,
    capture::ImageFrame,
    ocr::{RecognizedLine, collapse_lines},
};
use windows::{
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, ERROR_MOD_NOT_FOUND, ERROR_PROC_NOT_FOUND, REGDB_E_CLASSNOTREG,
    },
    core::Interface,
};
use windows_core::HRESULT;

use crate::capture::windows::{
    async_support::{wait_async_operation, wait_async_operation_with_progress},
    image_frame::software_bitmap_from_image_frame,
};

const AI_BACKEND_UNAVAILABLE: &str = "windows ai text recognizer is unavailable on this system";
const LEGACY_BACKEND_UNAVAILABLE: &str = "windows media ocr is unavailable on this system";

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

pub(super) fn recognize_text(image: &ImageFrame) -> Result<String, OcrError> {
    let bitmap = software_bitmap_from_image_frame(image).map_err(OcrError::SystemFailure)?;
    resolve_backend_result(recognize_text_ai(&bitmap), || {
        recognize_text_legacy(&bitmap)
    })
}

fn recognize_text_ai(bitmap: &SoftwareBitmap) -> Result<String, BackendError> {
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

    let lines = recognized
        .Lines()
        .map_err(ai_failure)?
        .iter()
        .flatten()
        .map(|line| {
            let bounds = line.BoundingBox().map_err(ai_failure)?;
            Ok(RecognizedLine {
                text: line.Text().map_err(ai_failure)?.to_string(),
                x: quad_min_x(&bounds),
                y: quad_min_y(&bounds),
            })
        })
        .collect::<Result<Vec<_>, BackendError>>()?;
    Ok(collapse_lines(lines))
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
                _ => Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE)),
            }
        }
        _ => Err(BackendError::Unavailable(AI_BACKEND_UNAVAILABLE)),
    }
}

fn recognize_text_legacy(bitmap: &SoftwareBitmap) -> Result<String, BackendError> {
    let bitmap =
        SoftwareBitmap::ConvertWithAlpha(bitmap, BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore)
            .map_err(legacy_failure)?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|_| BackendError::Unavailable(LEGACY_BACKEND_UNAVAILABLE))?;
    let result = wait_async_operation(engine.RecognizeAsync(&bitmap).map_err(legacy_failure)?)
        .map_err(legacy_failure)?;

    let lines_view = result.Lines().map_err(legacy_failure)?;
    let mut lines = Vec::new();
    for index in 0..lines_view.Size().map_err(legacy_failure)? {
        let line = lines_view.GetAt(index).map_err(legacy_failure)?;
        let bounds = line.Words().map_err(legacy_failure)?;
        let (x, y) = if bounds.Size().map_err(legacy_failure)? == 0 {
            (0.0, 0.0)
        } else {
            let first = bounds.GetAt(0).map_err(legacy_failure)?;
            let rect = first.BoundingRect().map_err(legacy_failure)?;
            (f64::from(rect.X), f64::from(rect.Y))
        };
        lines.push(RecognizedLine {
            text: line.Text().map_err(legacy_failure)?.to_string(),
            x,
            y,
        });
    }

    Ok(collapse_lines(lines))
}

fn quad_min_x(bounds: &ai_bindings::RecognizedTextBoundingBox) -> f64 {
    [
        f64::from(bounds.TopLeft.X),
        f64::from(bounds.TopRight.X),
        f64::from(bounds.BottomLeft.X),
        f64::from(bounds.BottomRight.X),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

fn quad_min_y(bounds: &ai_bindings::RecognizedTextBoundingBox) -> f64 {
    [
        f64::from(bounds.TopLeft.Y),
        f64::from(bounds.TopRight.Y),
        f64::from(bounds.BottomLeft.Y),
        f64::from(bounds.BottomRight.Y),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

fn resolve_backend_result(
    primary: Result<String, BackendError>,
    fallback: impl FnOnce() -> Result<String, BackendError>,
) -> Result<String, OcrError> {
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
    if is_ai_backend_unavailable_hresult(error.code()) {
        BackendError::Unavailable(AI_BACKEND_UNAVAILABLE)
    } else {
        BackendError::Failure(error.to_string())
    }
}

fn legacy_failure(error: windows_core::Error) -> BackendError {
    BackendError::Failure(error.to_string())
}

fn is_ai_backend_unavailable_hresult(code: HRESULT) -> bool {
    code == REGDB_E_CLASSNOTREG
        || code == CLASS_E_CLASSNOTAVAILABLE
        || code == HRESULT::from_win32(ERROR_MOD_NOT_FOUND.0)
        || code == HRESULT::from_win32(ERROR_PROC_NOT_FOUND.0)
}

#[cfg(test)]
mod tests {
    use super::{
        BackendError, ai_failure, is_ai_backend_unavailable_hresult, quad_min_x, quad_min_y,
    };
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, ERROR_MOD_NOT_FOUND, ERROR_PROC_NOT_FOUND, REGDB_E_CLASSNOTREG,
    };
    use windows_core::{Error, HRESULT};

    #[test]
    fn ai_bounding_box_maps_to_minimum_origin() {
        let bounds = super::ai_bindings::RecognizedTextBoundingBox {
            TopLeft: super::ai_bindings::Point { X: 12.0, Y: 18.0 },
            TopRight: super::ai_bindings::Point { X: 22.0, Y: 16.0 },
            BottomLeft: super::ai_bindings::Point { X: 11.0, Y: 29.0 },
            BottomRight: super::ai_bindings::Point { X: 24.0, Y: 31.0 },
        };

        assert_eq!(quad_min_x(&bounds), 11.0);
        assert_eq!(quad_min_y(&bounds), 16.0);
    }

    #[test]
    fn detects_availability_related_ai_hresults() {
        assert!(is_ai_backend_unavailable_hresult(REGDB_E_CLASSNOTREG));
        assert!(is_ai_backend_unavailable_hresult(CLASS_E_CLASSNOTAVAILABLE));
        assert!(is_ai_backend_unavailable_hresult(HRESULT::from_win32(
            ERROR_MOD_NOT_FOUND.0
        )));
        assert!(is_ai_backend_unavailable_hresult(HRESULT::from_win32(
            ERROR_PROC_NOT_FOUND.0
        )));
        assert!(!is_ai_backend_unavailable_hresult(HRESULT(
            0x80004005_u32 as _
        )));
    }

    #[test]
    fn maps_availability_errors_to_backend_unavailable() {
        let error = ai_failure(Error::from(REGDB_E_CLASSNOTREG));
        assert!(matches!(error, BackendError::Unavailable(_)));

        let error = ai_failure(Error::from(HRESULT::from_win32(ERROR_MOD_NOT_FOUND.0)));
        assert!(matches!(error, BackendError::Unavailable(_)));

        let error = ai_failure(Error::from(HRESULT(0x80004005_u32 as _)));
        assert!(matches!(error, BackendError::Failure(_)));
    }
}
