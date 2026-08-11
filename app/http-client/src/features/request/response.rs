mod collector;
mod data;
mod decoding;
mod save;
mod viewer;

use std::{sync::Arc, time::Duration};

use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ObjectFit, ParentElement as _,
    SharedString, Styled as _, StyledImage as _, Subscription, Task, Window, div, img,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _,
    alert::Alert,
    button::Button,
    label::Label,
    progress::Progress,
    scroll::ScrollableElement as _,
    select::{Select, SelectEvent, SelectItem, SelectState},
    tab::{Tab, TabBar},
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};

use super::{
    RequestView,
    runtime::{
        BodySizeDimension, FailedAttempt, RequestProblemKind, RequestRuntime, ResponseReceipt,
    },
};
use crate::foundation::I18n;

pub(crate) use data::{
    BodyDecoding, CAPTURE_LIMIT_BYTES, CompletedBody, INLINE_PREVIEW_BYTES, ResponseData,
    ResponseHead, ResponseProgress, ResponseReadLease, ResponseReadProblem, ResponseTiming,
};
pub(crate) use decoding::{
    ContentKind, SourceLanguage, TextDecodingProblem, classify_content_type, collect_response_body,
    decode_text, escape_header_value, fenced_source,
};
pub(crate) use save::{
    ResponseSaveProblem, ResponseSaveProblemKind, initial_save_directory, save_response,
    suggested_response_name,
};
pub(crate) use viewer::{ResponseProjection, ResponseViewWarning, ViewerMode, project_response};

#[cfg(test)]
pub(crate) use data::{ActiveBodyStorage, ResponseSizes, StoredBody};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ResponseTab {
    #[default]
    Body,
    Headers,
}

impl ResponseTab {
    const ALL: [Self; 2] = [Self::Body, Self::Headers];

    const fn index(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Headers => 1,
        }
    }
}

#[derive(Clone)]
pub(super) struct ViewerModeItem {
    value: ViewerMode,
    title: SharedString,
}

impl SelectItem for ViewerModeItem {
    type Value = ViewerMode;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(super) type ViewerModeItems = Vec<ViewerModeItem>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ResponseSaveStatus {
    #[default]
    Idle,
    Succeeded,
    Failed(ResponseSaveProblemKind),
}

pub(super) struct ResponsePane {
    tab: ResponseTab,
    mode: ViewerMode,
    pub(super) mode_state: Entity<SelectState<ViewerModeItems>>,
    projection: Option<ResponseProjection>,
    preview_task: Option<Task<()>>,
    save_status: ResponseSaveStatus,
    save_task: Option<Task<()>>,
    _mode_subscription: Subscription,
}

impl ResponsePane {
    pub(super) fn new(window: &mut Window, cx: &mut Context<RequestView>) -> Self {
        let items = viewer_mode_items(cx);
        let mode_state = cx.new(|cx| {
            SelectState::new(
                items,
                Some(gpui_component::IndexPath::default().row(0)),
                window,
                cx,
            )
        });
        let mode_subscription = cx.subscribe_in(
            &mode_state,
            window,
            |this, _, event: &SelectEvent<ViewerModeItems>, window, cx| {
                let SelectEvent::Confirm(Some(mode)) = event else {
                    return;
                };
                if this.response_pane.mode == *mode {
                    return;
                }
                this.response_pane.mode = *mode;
                this.refresh_response_projection(window, cx);
            },
        );
        Self {
            tab: ResponseTab::Body,
            mode: ViewerMode::Auto,
            mode_state,
            projection: None,
            preview_task: None,
            save_status: ResponseSaveStatus::Idle,
            save_task: None,
            _mode_subscription: mode_subscription,
        }
    }

    pub(super) fn reset_for_send(&mut self, window: &mut Window, cx: &mut Context<RequestView>) {
        self.tab = ResponseTab::Body;
        self.mode = ViewerMode::Auto;
        self.projection = None;
        self.preview_task.take();
        self.mode_state.update(cx, |state, cx| {
            state.set_selected_value(&ViewerMode::Auto, window, cx)
        });
        if self.save_task.is_none() {
            self.save_status = ResponseSaveStatus::Idle;
        }
    }

    pub(super) fn clear_projection(&mut self) {
        self.projection = None;
        self.preview_task.take();
    }

    pub(super) fn install_preview_task(&mut self, task: Task<()>) {
        self.preview_task = Some(task);
    }

    pub(super) fn install_projection(&mut self, projection: ResponseProjection) {
        self.projection = Some(projection);
        self.preview_task.take();
    }

    pub(super) const fn mode(&self) -> ViewerMode {
        self.mode
    }

    pub(super) fn save_is_running(&self) -> bool {
        self.save_task.is_some()
    }

    pub(super) fn install_save_task(&mut self, task: Task<()>) {
        self.save_status = ResponseSaveStatus::Idle;
        self.save_task = Some(task);
    }

    pub(super) fn finish_save(&mut self, result: Result<(), ResponseSaveProblem>) {
        self.save_status = match result {
            Ok(()) => ResponseSaveStatus::Succeeded,
            Err(problem) => ResponseSaveStatus::Failed(problem.kind()),
        };
        self.save_task.take();
    }

    pub(super) fn cancel_save_prompt(&mut self) {
        self.save_status = ResponseSaveStatus::Idle;
        self.save_task.take();
    }

    pub(super) fn render(
        &self,
        runtime: &RequestRuntime,
        cx: &mut Context<RequestView>,
    ) -> AnyElement {
        let actions = self.render_actions(runtime, cx);
        let save_alert = match self.save_status {
            ResponseSaveStatus::Idle => None,
            ResponseSaveStatus::Succeeded => Some(
                Alert::success(
                    "response-save-success",
                    cx.global::<I18n>().t("response-save-complete"),
                )
                .into_any_element(),
            ),
            ResponseSaveStatus::Failed(_) => Some(
                Alert::error(
                    "response-save-failure",
                    cx.global::<I18n>().t("response-save-failed"),
                )
                .into_any_element(),
            ),
        };
        let content = match runtime {
            RequestRuntime::Idle => centered_status(cx.global::<I18n>().t("response-empty")),
            RequestRuntime::Sending { .. } => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(Label::new(cx.global::<I18n>().t("response-sending")))
                .child(Progress::new("response-sending-progress").loading(true))
                .into_any_element(),
            RequestRuntime::Receiving { receipt, .. } => self.render_receiving(receipt, cx),
            RequestRuntime::Ready { response } => self.render_ready(response, cx),
            RequestRuntime::Failed { attempt } => self.render_failed(attempt, cx),
        };

        v_flex()
            .size_full()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Label::new(cx.global::<I18n>().t("response-title")).font_semibold())
                    .child(actions),
            )
            .when_some(save_alert, |this, alert| this.child(alert))
            .child(content)
            .into_any_element()
    }

    fn render_actions(
        &self,
        runtime: &RequestRuntime,
        cx: &mut Context<RequestView>,
    ) -> AnyElement {
        let i18n = cx.global::<I18n>();
        div()
            .flex()
            .items_center()
            .gap_2()
            .when(runtime.response().is_some(), |this| {
                this.child(
                    Button::new("response-save")
                        .label(i18n.t("button-save-response"))
                        .disabled(self.save_is_running())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.start_response_save(window, cx)),
                        ),
                )
            })
            .when(
                matches!(
                    runtime,
                    RequestRuntime::Ready { .. } | RequestRuntime::Failed { .. }
                ),
                |this| {
                    this.child(
                        Button::new("response-clear")
                            .label(i18n.t("button-clear-response"))
                            .on_click(cx.listener(|this, _, _, cx| this.clear_response(cx))),
                    )
                },
            )
            .into_any_element()
    }

    fn render_receiving(
        &self,
        receipt: &ResponseReceipt,
        cx: &mut Context<RequestView>,
    ) -> AnyElement {
        let progress = receipt.progress;
        let message = receiving_message(progress, cx);
        let progress_element =
            if let Some(total) = progress.declared_encoded_bytes.filter(|v| *v > 0) {
                Progress::new("response-receiving-progress")
                    .value((progress.received_encoded_bytes as f32 / total as f32) * 100.)
            } else {
                Progress::new("response-receiving-progress").loading(true)
            };
        v_flex()
            .size_full()
            .min_h(px(0.))
            .child(self.render_summary(&receipt.head, Some(receipt.head_after), None, progress, cx))
            .child(self.render_tabs(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_2()
                    .p_2()
                    .when(self.tab == ResponseTab::Body, |this| {
                        this.child(Label::new(message)).child(progress_element)
                    })
                    .when(self.tab == ResponseTab::Headers, |this| {
                        this.child(render_headers(&receipt.head, cx))
                    }),
            )
            .into_any_element()
    }

    fn render_ready(
        &self,
        response: &Arc<ResponseData>,
        cx: &mut Context<RequestView>,
    ) -> AnyElement {
        let progress = response.progress();
        v_flex()
            .size_full()
            .min_h(px(0.))
            .child(self.render_summary(
                response.head(),
                Some(response.timing().head_after),
                Some(response.timing().completed_after),
                progress,
                cx,
            ))
            .child(self.render_tabs(cx))
            .child(match self.tab {
                ResponseTab::Body => self.render_body_projection(cx),
                ResponseTab::Headers => render_headers(response.head(), cx),
            })
            .into_any_element()
    }

    fn render_failed(&self, attempt: &FailedAttempt, cx: &mut Context<RequestView>) -> AnyElement {
        let problem = problem_message(attempt.problem.kind(), cx);
        let receipt = attempt.receipt.as_ref();
        v_flex()
            .size_full()
            .min_h(px(0.))
            .p_2()
            .gap_2()
            .child(Alert::error("request-failed", problem))
            .when_some(receipt, |this, receipt| {
                this.child(self.render_summary(
                    &receipt.head,
                    Some(receipt.head_after),
                    Some(attempt.failed_after),
                    receipt.progress,
                    cx,
                ))
                .child(render_headers(&receipt.head, cx))
            })
            .into_any_element()
    }

    fn render_tabs(&self, cx: &mut Context<RequestView>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let labels = [i18n.t("tab-response-body"), i18n.t("tab-response-headers")];
        TabBar::new("response-tabs")
            .selected_index(self.tab.index())
            .on_click(cx.listener(|this, index, _, cx| {
                if let Some(tab) = ResponseTab::ALL.get(*index).copied() {
                    this.response_pane.tab = tab;
                    cx.notify();
                }
            }))
            .children(labels.into_iter().map(|label| Tab::new().label(label)))
    }

    fn render_body_projection(&self, cx: &mut Context<RequestView>) -> AnyElement {
        let warning = self
            .projection
            .as_ref()
            .and_then(ResponseProjection::warning)
            .map(|warning| warning_message(warning, cx));
        let projection = match &self.projection {
            None => centered_status(cx.global::<I18n>().t("response-sending")),
            Some(ResponseProjection::Empty) => centered_status(SharedString::default()),
            Some(ResponseProjection::Text { markdown, .. }) => div()
                .flex_1()
                .min_h(px(0.))
                .child(
                    gpui_component::text::TextView::markdown(
                        "response-body-text",
                        markdown.clone(),
                    )
                    .selectable(true)
                    .scrollable(true),
                )
                .into_any_element(),
            Some(ResponseProjection::Image { image, .. }) => div()
                .flex_1()
                .min_h(px(0.))
                .p_2()
                .child(
                    img(image.clone())
                        .object_fit(ObjectFit::Contain)
                        .size_full(),
                )
                .into_any_element(),
            Some(ResponseProjection::Unavailable(_)) => centered_status(
                warning
                    .clone()
                    .unwrap_or_else(|| cx.global::<I18n>().t("response-viewer-mode-unavailable")),
            ),
        };

        v_flex()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .p_2()
            .gap_2()
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(180.))
                    .child(Select::new(&self.mode_state)),
            )
            .when_some(warning, |this, warning| {
                this.child(Alert::warning("response-view-warning", warning))
            })
            .child(projection)
            .into_any_element()
    }

    fn render_summary(
        &self,
        head: &ResponseHead,
        head_after: Option<Duration>,
        completed_after: Option<Duration>,
        progress: ResponseProgress,
        cx: &App,
    ) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .px_2()
            .py_1()
            .children([
                summary_value(i18n.t("response-status"), head.status.to_string()),
                summary_value(i18n.t("response-protocol"), protocol_label(head.version)),
                summary_value(
                    i18n.t("response-final-url"),
                    head.final_url.as_str().to_owned(),
                ),
                summary_value(
                    i18n.t("response-head-time"),
                    head_after.map_or_else(|| "—".into(), format_duration),
                ),
                summary_value(
                    i18n.t("response-total-time"),
                    completed_after.map_or_else(|| "—".into(), format_duration),
                ),
                summary_value(
                    i18n.t("response-received-size"),
                    format_bytes(progress.received_encoded_bytes),
                ),
                summary_value(
                    i18n.t("response-stored-size"),
                    format_bytes(progress.stored_body_bytes),
                ),
            ])
    }
}

fn viewer_mode_items(cx: &App) -> ViewerModeItems {
    ViewerMode::ALL
        .into_iter()
        .map(|value| ViewerModeItem {
            value,
            title: cx.global::<I18n>().t(viewer_mode_key(value)).into(),
        })
        .collect()
}

const fn viewer_mode_key(mode: ViewerMode) -> &'static str {
    match mode {
        ViewerMode::Auto => "response-view-auto",
        ViewerMode::Text => "response-view-text",
        ViewerMode::Json => "response-view-json",
        ViewerMode::Xml => "response-view-xml",
        ViewerMode::Hex => "response-view-hex",
        ViewerMode::Base64 => "response-view-base64",
        ViewerMode::Image => "response-view-image",
    }
}

fn centered_status(message: impl Into<SharedString>) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(0.))
        .flex()
        .items_center()
        .justify_center()
        .child(Label::new(message.into()))
        .into_any_element()
}

fn summary_value(label: impl Into<SharedString>, value: String) -> AnyElement {
    div()
        .flex()
        .gap_1()
        .child(Label::new(label.into()).text_xs())
        .child(Label::new(value).text_xs())
        .into_any_element()
}

fn render_headers(head: &ResponseHead, cx: &App) -> AnyElement {
    if head.headers.is_empty() {
        return centered_status(cx.global::<I18n>().t("response-headers-empty"));
    }
    let i18n = cx.global::<I18n>();
    div()
        .flex_1()
        .min_h(px(0.))
        .overflow_scrollbar()
        .child(
            Table::new()
                .small()
                .child(
                    TableHeader::new().child(
                        TableRow::new()
                            .child(
                                TableHead::new()
                                    .w(px(240.))
                                    .child(Label::new(i18n.t("response-header-name"))),
                            )
                            .child(
                                TableHead::new().child(Label::new(i18n.t("response-header-value"))),
                            ),
                    ),
                )
                .child(
                    TableBody::new().children(head.headers.iter().map(|(name, value)| {
                        TableRow::new()
                            .child(
                                TableCell::new()
                                    .w(px(240.))
                                    .child(Label::new(name.as_str().to_owned())),
                            )
                            .child(TableCell::new().child(Label::new(escape_header_value(value))))
                    })),
                ),
        )
        .into_any_element()
}

fn receiving_message(progress: ResponseProgress, cx: &App) -> String {
    let i18n = cx.global::<I18n>();
    let mut args = FluentArgs::new();
    args.set("received", format_bytes(progress.received_encoded_bytes));
    match progress.declared_encoded_bytes {
        Some(total) => {
            args.set("total", format_bytes(total));
            i18n.t_with_args("response-receiving-known", &args)
        }
        None => i18n.t_with_args("response-receiving-unknown", &args),
    }
}

fn problem_message(kind: RequestProblemKind, cx: &App) -> String {
    let i18n = cx.global::<I18n>();
    match kind {
        RequestProblemKind::Transport => i18n.t("request-problem-transport"),
        RequestProblemKind::Timeout => i18n.t("request-problem-timeout"),
        RequestProblemKind::Redirect(_) => i18n.t("request-problem-redirect"),
        RequestProblemKind::RequestBodyRead => i18n.t("request-problem-request-body"),
        RequestProblemKind::ResponseBodyRead => i18n.t("request-problem-response-read"),
        RequestProblemKind::ResponseBodyDecode => i18n.t("request-problem-response-decode"),
        RequestProblemKind::TemporaryStorage => i18n.t("request-problem-storage"),
        RequestProblemKind::BodyTooLarge {
            dimension,
            limit,
            observed,
        } => {
            let mut args = FluentArgs::new();
            args.set("limit", limit);
            args.set("observed", observed);
            i18n.t_with_args(
                match dimension {
                    BodySizeDimension::Encoded => "request-problem-too-large-encoded",
                    BodySizeDimension::Stored => "request-problem-too-large-stored",
                },
                &args,
            )
        }
        RequestProblemKind::Internal => i18n.t("request-problem-internal"),
    }
}

fn warning_message(warning: ResponseViewWarning, cx: &App) -> String {
    cx.global::<I18n>().t(match warning {
        ResponseViewWarning::Truncated => "response-preview-truncated",
        ResponseViewWarning::UnsupportedDecoding => "response-decoding-unsupported",
        ResponseViewWarning::ModeUnavailable => "response-viewer-mode-unavailable",
        ResponseViewWarning::InvalidJson => "response-viewer-invalid-json",
        ResponseViewWarning::InvalidImage => "response-viewer-invalid-image",
        ResponseViewWarning::ImageTooLarge => "response-image-too-large",
    })
}

fn format_duration(duration: Duration) -> String {
    format!("{} ms", duration.as_millis())
}

fn protocol_label(version: http::Version) -> String {
    match version {
        http::Version::HTTP_09 => "HTTP/0.9".into(),
        http::Version::HTTP_10 => "HTTP/1.0".into(),
        http::Version::HTTP_11 => "HTTP/1.1".into(),
        http::Version::HTTP_2 => "HTTP/2".into(),
        http::Version::HTTP_3 => "HTTP/3".into(),
        _ => format!("{version:?}"),
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024. * 1024.))
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod pane_tests {
    use super::*;

    #[test]
    fn response_tabs_and_viewer_modes_have_total_stable_indices() {
        assert_eq!(ResponseTab::ALL.map(ResponseTab::index), [0, 1]);
        assert_eq!(
            ViewerMode::ALL,
            [
                ViewerMode::Auto,
                ViewerMode::Text,
                ViewerMode::Json,
                ViewerMode::Xml,
                ViewerMode::Hex,
                ViewerMode::Base64,
                ViewerMode::Image,
            ]
        );
    }
}
