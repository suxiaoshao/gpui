#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let x = self.x.min(self.x + self.width);
        let y = self.y.min(self.y + self.height);
        Self {
            x,
            y,
            width: self.width.abs(),
            height: self.height.abs(),
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        let rect = self.normalized();
        rect.width <= f64::EPSILON || rect.height <= f64::EPSILON
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub bytes_rgba8: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::ScreenRect;

    #[test]
    fn normalizes_negative_dimensions() {
        let rect = ScreenRect::new(16.0, 12.0, -6.0, -4.0).normalized();

        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 8.0);
        assert_eq!(rect.width, 6.0);
        assert_eq!(rect.height, 4.0);
    }

    #[test]
    fn empty_rect_detects_zero_dimensions() {
        assert!(ScreenRect::new(0.0, 0.0, 0.0, 5.0).is_empty());
    }
}
