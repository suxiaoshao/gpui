use gpui_kit::App;

pub(crate) mod overlay;

pub(crate) fn init(cx: &mut App) {
    overlay::init(cx);
}
