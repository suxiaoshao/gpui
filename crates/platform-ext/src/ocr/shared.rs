use crate::capture::ImageFrame;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecognizedLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
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

pub(crate) fn compare_lines(left: &RecognizedLine, right: &RecognizedLine) -> std::cmp::Ordering {
    left.y
        .partial_cmp(&right.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            left.x
                .partial_cmp(&right.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

pub(crate) fn collapse_lines(mut lines: Vec<RecognizedLine>) -> String {
    lines.sort_by(compare_lines);
    lines
        .into_iter()
        .filter_map(|line| {
            let text = line.text.trim().to_string();
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{RecognizedLine, collapse_lines};

    #[test]
    fn flattens_lines_into_text() {
        let text = collapse_lines(vec![
            RecognizedLine {
                text: "hello".into(),
                x: 0.0,
                y: 0.0,
            },
            RecognizedLine {
                text: "world".into(),
                x: 10.0,
                y: 8.0,
            },
        ]);

        assert_eq!(text, "hello\nworld");
    }
}
