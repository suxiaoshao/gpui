use crate::capture::{ImageFrame, ScreenRect};

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

pub(crate) fn validate_image_frame(image: &ImageFrame) -> Result<(), crate::OcrError> {
    if image.width == 0 || image.height == 0 {
        return Err(crate::OcrError::InvalidInput(
            "image dimensions must be greater than zero",
        ));
    }

    let width = usize::try_from(image.width)
        .map_err(|_| crate::OcrError::InvalidInput("image width is too large"))?;
    let height = usize::try_from(image.height)
        .map_err(|_| crate::OcrError::InvalidInput("image height is too large"))?;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(crate::OcrError::InvalidInput("image dimensions overflow"))?;

    if image.bytes_rgba8.len() != expected_len {
        return Err(crate::OcrError::InvalidInput(
            "image must contain width * height * 4 RGBA bytes",
        ));
    }

    Ok(())
}

pub(crate) fn compare_lines(left: &OcrLine, right: &OcrLine) -> std::cmp::Ordering {
    left.bounds
        .y
        .partial_cmp(&right.bounds.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            left.bounds
                .x
                .partial_cmp(&right.bounds.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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
