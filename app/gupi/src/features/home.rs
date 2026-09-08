use crate::foundation::i18n::t;
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::*;
use pi_rpc::probe::PiProbeData;
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
        .child(t(cx, "home-description"))
}
