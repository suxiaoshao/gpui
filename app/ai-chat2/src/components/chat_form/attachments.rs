pub(super) const STRIP_GAP: f32 = 8.;
pub(super) const STRIP_BOTTOM_MARGIN: f32 = 8.;
pub(super) const IMAGE_THUMBNAIL_SIZE: f32 = 80.;
pub(super) const FILE_CARD_WIDTH: f32 = 220.;
pub(super) const FILE_CARD_HEIGHT: f32 = 56.;
pub(super) const CARD_RADIUS: f32 = 8.;
pub(super) const REMOVE_BUTTON_SIZE: f32 = 20.;

pub(super) fn format_file_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "Unknown size".to_string();
    };
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let size = size as f64;
    if size < KB {
        format!("{} B", size as u64)
    } else if size < MB {
        format!("{:.1} KB", size / KB)
    } else if size < GB {
        format!("{:.1} MB", size / MB)
    } else {
        format!("{:.1} GB", size / GB)
    }
}

#[cfg(test)]
mod tests {
    use super::format_file_size;

    #[test]
    fn formats_file_size() {
        assert_eq!(format_file_size(Some(42)), "42 B");
        assert_eq!(format_file_size(Some(1536)), "1.5 KB");
        assert_eq!(format_file_size(None), "Unknown size");
    }
}
