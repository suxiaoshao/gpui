use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, button::Button, h_flex, label::Label,
    spinner::Spinner, v_flex,
};
use gpui_operation::refresh::Phase;

use crate::foundation::I18n;

pub(crate) fn refresh_status(
    id: &'static str,
    phase: Phase,
    problem: Option<String>,
    refresh: fn(&mut App),
    cx: &App,
) -> Option<AnyElement> {
    if phase == Phase::Ready {
        return None;
    }
    let running = matches!(
        phase,
        Phase::Loading | Phase::Refreshing | Phase::Retrying | Phase::RefreshingDegraded
    );
    let stale = matches!(
        phase,
        Phase::Refreshing | Phase::Degraded | Phase::RefreshingDegraded
    );
    let title_key = if stale {
        "resource-status-stale"
    } else if running {
        "resource-status-loading"
    } else {
        "resource-status-unavailable"
    };

    Some(
        v_flex()
            .w_full()
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if problem.is_some() {
                cx.theme().warning
            } else {
                cx.theme().border
            })
            .bg(cx.theme().secondary.opacity(0.45))
            .p_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .children(running.then(|| Spinner::new().small()))
                            .child(Label::new(cx.global::<I18n>().t(title_key)).font_medium()),
                    )
                    .child(
                        Button::new(id)
                            .label(cx.global::<I18n>().t("resource-status-refresh"))
                            .loading(running)
                            .disabled(running)
                            .on_click(move |_, _, cx| refresh(cx)),
                    ),
            )
            .children(
                problem.map(|problem| Label::new(problem).text_sm().text_color(cx.theme().warning)),
            )
            .into_any_element(),
    )
}
