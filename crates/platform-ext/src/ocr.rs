use crate::{
    capture::{ImageFrame, ScreenRect},
    error::OcrError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrLanguage {
    English,
    SimplifiedChinese,
    TraditionalChinese,
    Japanese,
    Korean,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrRequest {
    pub image: ImageFrame,
    pub languages: Vec<OcrLanguage>,
    pub include_word_boxes: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub bounds: ScreenRect,
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub bounds: ScreenRect,
    pub words: Vec<OcrWord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
}

impl OcrResult {
    #[must_use]
    pub fn from_lines(lines: Vec<OcrLine>) -> Self {
        let text = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Self { text, lines }
    }
}

pub fn recognize_text(request: OcrRequest) -> Result<OcrResult, OcrError> {
    validate_image_frame(&request.image)?;
    recognize_text_impl(request)
}

fn validate_image_frame(image: &ImageFrame) -> Result<(), OcrError> {
    if image.width == 0 || image.height == 0 {
        return Err(OcrError::InvalidInput(
            "image dimensions must be greater than zero",
        ));
    }

    let width = usize::try_from(image.width)
        .map_err(|_| OcrError::InvalidInput("image width is too large"))?;
    let height = usize::try_from(image.height)
        .map_err(|_| OcrError::InvalidInput("image height is too large"))?;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(OcrError::InvalidInput("image dimensions overflow"))?;

    if image.bytes_rgba8.len() != expected_len {
        return Err(OcrError::InvalidInput(
            "image must contain width * height * 4 RGBA bytes",
        ));
    }

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn recognize_text_impl(_: OcrRequest) -> Result<OcrResult, OcrError> {
    Err(OcrError::BackendUnavailable(
        "platform OCR backend is not implemented yet",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn recognize_text_impl(_: OcrRequest) -> Result<OcrResult, OcrError> {
    Err(OcrError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::{OcrLine, OcrResult, OcrWord};
    use crate::capture::ScreenRect;

    #[test]
    fn flattens_lines_into_text() {
        let result = OcrResult::from_lines(vec![
            OcrLine {
                text: "hello".into(),
                bounds: ScreenRect::default(),
                words: vec![OcrWord {
                    text: "hello".into(),
                    bounds: ScreenRect::default(),
                    confidence: Some(0.9),
                }],
            },
            OcrLine {
                text: "world".into(),
                bounds: ScreenRect::default(),
                words: vec![],
            },
        ]);

        assert_eq!(result.text, "hello\nworld");
    }
}
