use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::*;
pub(crate) fn recovery(title: String, description: String, cx: &App) -> Div {
    v_flex()
        .gap_3()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}
