#[derive(Clone, Debug, PartialEq)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub bytes_rgba8: Vec<u8>,
}
