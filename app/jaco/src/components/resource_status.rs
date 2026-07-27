use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, button::Button, h_flex, label::Label,
    spinner::Spinner, v_flex,
};
use gpui_operation::refresh::Phase;

use crate::foundation::I18n;

#[derive(IntoElement)]
pub(crate) struct ResourceRefreshButton {
    id: &'static str,
    running: bool,
    refresh: fn(&mut App),
}

impl RenderOnce for ResourceRefreshButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        Button::new(self.id)
            .label(cx.global::<I18n>().t("resource-status-refresh"))
            .loading(self.running)
            .disabled(self.running)
            .on_click(move |_, _, cx| (self.refresh)(cx))
    }
}

#[derive(IntoElement)]
pub(crate) struct ResourceLoadingView {
    id: &'static str,
    running: bool,
    refresh: fn(&mut App),
}

impl RenderOnce for ResourceLoadingView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        resource_status_container(cx, false)
            .child(resource_status_header(
                cx.global::<I18n>().t("resource-status-loading"),
                true,
                ResourceRefreshButton {
                    id: self.id,
                    running: self.running,
                    refresh: self.refresh,
                },
            ))
            .into_any_element()
    }
}

#[derive(IntoElement)]
pub(crate) struct ResourceProblemView {
    id: &'static str,
    problem: String,
    running: bool,
    refresh: fn(&mut App),
}

impl RenderOnce for ResourceProblemView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        resource_status_container(cx, true)
            .child(resource_status_header(
                cx.global::<I18n>().t("resource-status-unavailable"),
                self.running,
                ResourceRefreshButton {
                    id: self.id,
                    running: self.running,
                    refresh: self.refresh,
                },
            ))
            .child(
                Label::new(self.problem)
                    .text_sm()
                    .text_color(cx.theme().warning),
            )
            .into_any_element()
    }
}

#[derive(IntoElement)]
pub(crate) struct ResourceStaleNotice {
    id: &'static str,
    problem: Option<String>,
    running: bool,
    refresh: fn(&mut App),
}

impl RenderOnce for ResourceStaleNotice {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        resource_status_container(cx, self.problem.is_some())
            .child(resource_status_header(
                cx.global::<I18n>().t("resource-status-stale"),
                self.running,
                ResourceRefreshButton {
                    id: self.id,
                    running: self.running,
                    refresh: self.refresh,
                },
            ))
            .children(
                self.problem
                    .map(|problem| Label::new(problem).text_sm().text_color(cx.theme().warning)),
            )
            .into_any_element()
    }
}

pub(crate) fn refresh_status(
    id: &'static str,
    phase: Phase,
    problem: Option<String>,
    refresh: fn(&mut App),
    _cx: &App,
) -> Option<AnyElement> {
    match phase {
        Phase::Ready => None,
        Phase::Idle | Phase::Loading | Phase::Retrying => Some(
            ResourceLoadingView {
                id,
                running: matches!(phase, Phase::Loading | Phase::Retrying),
                refresh,
            }
            .into_any_element(),
        ),
        Phase::Unavailable => Some(
            ResourceProblemView {
                id,
                problem: problem.unwrap_or_default(),
                running: false,
                refresh,
            }
            .into_any_element(),
        ),
        Phase::Refreshing | Phase::Degraded | Phase::RefreshingDegraded => Some(
            ResourceStaleNotice {
                id,
                problem,
                running: matches!(phase, Phase::Refreshing | Phase::RefreshingDegraded),
                refresh,
            }
            .into_any_element(),
        ),
    }
}

fn resource_status_container(cx: &App, warning: bool) -> Div {
    v_flex()
        .w_full()
        .gap_2()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(if warning {
            cx.theme().warning
        } else {
            cx.theme().border
        })
        .bg(cx.theme().secondary.opacity(0.45))
        .p_3()
}

fn resource_status_header(
    title: impl Into<SharedString>,
    running: bool,
    button: ResourceRefreshButton,
) -> Div {
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
                .child(Label::new(title).font_medium()),
        )
        .child(button)
}
