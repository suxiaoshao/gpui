use crate::{foundation::i18n::t, pi::PiProbeData};
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::*;
pub(crate) fn home(data: &PiProbeData, cx: &App) -> Div {
    v_flex()
        .gap_4()
        .child(div().text_3xl().child("Gupi"))
        .child(t(cx, "home-ready"))
        .child(div().text_color(cx.theme().muted_foreground).child(format!(
            "Pi {} · {}",
            data.version,
            data.command.display()
        )))
        .child(div().text_sm().child(format!(
            "{}: {}",
            t(cx, "home-checked"),
            data.checked_at.elapsed().unwrap_or_default().as_secs()
        )))
        .child(t(cx, "home-description"))
}
