mod collector;
mod data;
mod decoding;
mod media;
mod pdf;
mod save;
mod viewer;

use std::{sync::Arc, time::Duration};

use fluent_bundle::FluentArgs;
use futures_util::{FutureExt as _, future::Either, future::select};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ObjectFit, ParentElement as _, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Task, Window, div, img,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _,
    alert::Alert,
    button::Button,
    input::{Editor, EditorState},
    label::Label,
    progress::Progress,
    scroll::ScrollableElement as _,
    select::{Select, SelectEvent, SelectItem, SelectState},
    slider::{Slider, SliderEvent, SliderState, SliderValue},
    tab::{Tab, TabBar},
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_operation::Transition as _;

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
    declared_encoded_bytes, decode_text, escape_header_value,
};
use media::audio::AudioDriver;
use media::{
    MediaDriverEvent, MediaDriverEvents, MediaMessage, MediaPhase, MediaProblem, MediaProblemKind,
    MediaRuntime, PreviewToken, ResponseAssetProblem, ResponseAssetProblemKind,
};
use pdf::{PdfPreview, PdfProblemKind, PdfViewport, PdfWorkerHandle};
pub(crate) use save::{
    ResponseSaveProblem, ResponseSaveProblemKind, initial_save_directory, save_response,
    suggested_response_name,
};
pub(crate) use viewer::{
    ResponseProjection, ResponseViewWarning, ViewerMode, project_response, resolved_viewer_mode,
};

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
    disabled: bool,
}

impl SelectItem for ViewerModeItem {
    type Value = ViewerMode;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.disabled
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

struct PendingMediaPreparation {
    task: Task<()>,
    gate: async_channel::Sender<()>,
}

pub(super) struct ResponsePane {
    tab: ResponseTab,
    mode: ViewerMode,
    pub(super) mode_state: Entity<SelectState<ViewerModeItems>>,
    projection: Option<ResponseProjection>,
    text_editor: Option<Entity<EditorState>>,
    preview_task: Option<Task<()>>,
    preview_generation: u64,
    preview_token: Option<PreviewToken>,
    media: MediaRuntime,
    pdf: PdfPreview,
    pdf_viewport_handle: ScrollHandle,
    pdf_resize_pending: Option<PdfViewport>,
    seek_state: Entity<SliderState>,
    volume_state: Entity<SliderState>,
    seek_dragging: bool,
    save_status: ResponseSaveStatus,
    save_task: Option<Task<()>>,
    _mode_subscription: Subscription,
    _seek_subscription: Subscription,
    _volume_subscription: Subscription,
}

impl ResponsePane {
    pub(super) fn new(window: &mut Window, cx: &mut Context<RequestView>) -> Self {
        let items = viewer_mode_items(None, cx);
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
        let seek_state = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.001)
                .default_value(0.)
        });
        let seek_subscription = cx.subscribe_in(
            &seek_state,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => this.response_pane.seek_dragging = true,
                SliderEvent::Release(SliderValue::Single(value)) => {
                    this.response_pane.seek_dragging = false;
                    let duration = this
                        .response_pane
                        .media
                        .active()
                        .and_then(|active| active.metadata().duration());
                    if let Some(duration) = duration {
                        (&mut this.response_pane.media)
                            .transition(MediaMessage::Seek(duration.mul_f32(value.clamp(0., 1.))));
                        this.response_pane.sync_media_controls(window, cx);
                        cx.notify();
                    }
                }
                SliderEvent::Release(SliderValue::Range(_, _)) => {
                    this.response_pane.seek_dragging = false;
                }
            },
        );
        let volume_state = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.01)
                .default_value(1.)
        });
        let volume_subscription = cx.subscribe_in(
            &volume_state,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                let SliderEvent::Change(SliderValue::Single(value)) = event else {
                    return;
                };
                (&mut this.response_pane.media)
                    .transition(MediaMessage::SetVolume(value.clamp(0., 1.)));
                this.response_pane.sync_media_controls(window, cx);
                cx.notify();
            },
        );
        Self {
            tab: ResponseTab::Body,
            mode: ViewerMode::Auto,
            mode_state,
            projection: None,
            text_editor: None,
            preview_task: None,
            preview_generation: 0,
            preview_token: None,
            media: MediaRuntime::default(),
            pdf: PdfPreview::new(),
            pdf_viewport_handle: ScrollHandle::new(),
            pdf_resize_pending: None,
            seek_state,
            volume_state,
            seek_dragging: false,
            save_status: ResponseSaveStatus::Idle,
            save_task: None,
            _mode_subscription: mode_subscription,
            _seek_subscription: seek_subscription,
            _volume_subscription: volume_subscription,
        }
    }

    pub(super) fn reset_for_send(&mut self, window: &mut Window, cx: &mut Context<RequestView>) {
        self.tab = ResponseTab::Body;
        self.mode = ViewerMode::Auto;
        self.teardown_preview();
        self.mode_state.update(cx, |state, cx| {
            state.set_items(viewer_mode_items(None, cx), window, cx);
            state.set_selected_value(&ViewerMode::Auto, window, cx);
        });
        if self.save_task.is_none() {
            self.save_status = ResponseSaveStatus::Idle;
        }
    }

    pub(super) fn clear_projection(&mut self) {
        self.teardown_preview();
    }

    pub(super) fn begin_preview(
        &mut self,
        response: Arc<ResponseData>,
        effective_mode: ViewerMode,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) -> PreviewToken {
        self.teardown_preview();
        self.mode_state.update(cx, |state, cx| {
            state.set_items(viewer_mode_items(Some(&response), cx), window, cx);
        });
        let token = PreviewToken::new(response, self.mode, effective_mode, self.preview_generation);
        self.preview_token = Some(token.clone());
        token
    }

    pub(super) fn is_current_preview(&self, token: &PreviewToken) -> bool {
        self.preview_token
            .as_ref()
            .is_some_and(|current| current.matches(token))
    }

    fn teardown_preview(&mut self) {
        self.preview_generation = self
            .preview_generation
            .checked_add(1)
            .expect("response preview generation exhausted");
        self.preview_token = None;
        self.projection = None;
        self.text_editor = None;
        self.preview_task.take();
        (&mut self.media).transition(MediaMessage::Stop);
        self.pdf.stop();
        self.pdf_resize_pending = None;
    }

    pub(super) fn install_preview_task(&mut self, task: Task<()>) {
        self.preview_task = Some(task);
    }

    pub(super) fn install_projection(
        &mut self,
        projection: ResponseProjection,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) {
        self.text_editor = match &projection {
            ResponseProjection::Text {
                source, language, ..
            } => {
                let source = source.clone();
                let language = language.editor_language();
                Some(cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language(language)
                        .line_number(true)
                        .searchable(true)
                        .replaceable(false)
                        .soft_wrap(false)
                        .scroll_beyond_last_line(Some(0))
                        .default_value(source)
                }))
            }
            ResponseProjection::Empty
            | ResponseProjection::Image { .. }
            | ResponseProjection::Unavailable(_) => None,
        };
        self.projection = Some(projection);
        self.preview_task.take();
    }

    pub(super) fn start_pdf_preview(
        &mut self,
        token: PreviewToken,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) {
        self.pdf.begin_read(token.clone());
        let lease = token.response().read_lease();
        let read = gpui_tokio::Tokio::spawn(cx, async move {
            lease.read_all_bounded(CAPTURE_LIMIT_BYTES).await
        });
        let owner = cx.entity().downgrade();
        let task_token = token.clone();
        let viewport = pdf_viewport(&self.pdf_viewport_handle, window);
        let task = window.spawn(cx, async move |cx| {
            let bytes = match read.await {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(ResponseReadProblem::LimitExceeded)) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        if this.response_pane.is_current_preview(&task_token) {
                            this.response_pane
                                .pdf
                                .fail_before_load(task_token.clone(), pdf::PdfProblem::budget());
                            cx.notify();
                        }
                    });
                    return;
                }
                Ok(Err(_)) | Err(_) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        if this.response_pane.is_current_preview(&task_token) {
                            this.response_pane
                                .pdf
                                .fail_before_load(task_token.clone(), pdf::PdfProblem::internal());
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let mut worker = match PdfWorkerHandle::new(bytes) {
                Ok(worker) => worker,
                Err(problem) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        if this.response_pane.is_current_preview(&task_token) {
                            this.response_pane
                                .pdf
                                .fail_before_load(task_token.clone(), problem);
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let receiver = match worker.take_event_receiver() {
                Ok(receiver) => receiver,
                Err(problem) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        if this.response_pane.is_current_preview(&task_token) {
                            this.response_pane
                                .pdf
                                .fail_before_load(task_token.clone(), problem);
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let bridge_owner = owner.clone();
            let bridge_token = task_token.clone();
            let _ = owner.update_in(cx, |this, window, cx| {
                if !this.response_pane.is_current_preview(&task_token) {
                    return;
                }
                let event_task = window.spawn(cx, async move |cx| {
                    while let Ok(event) = receiver.recv().await {
                        let event_token = event.token().clone();
                        if bridge_owner
                            .update_in(cx, |this, _, cx| {
                                if this.response_pane.is_current_preview(&bridge_token)
                                    && event_token.matches(&bridge_token)
                                {
                                    this.response_pane.pdf.handle_event(event);
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    let _ = bridge_owner.update_in(cx, |this, _, cx| {
                        if this.response_pane.is_current_preview(&bridge_token)
                            && this.response_pane.pdf.is_loading()
                        {
                            this.response_pane
                                .pdf
                                .fail_if_active(bridge_token.clone(), pdf::PdfProblem::internal());
                            cx.notify();
                        }
                    });
                });
                this.response_pane
                    .pdf
                    .load(task_token, worker, event_task, viewport);
                cx.notify();
            });
        });
        self.install_preview_task(task);
    }

    pub(super) fn start_audio_preview(
        &mut self,
        token: PreviewToken,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) {
        let pending = Self::build_audio_prepare_task(token.clone(), window, cx);
        (&mut self.media).transition(MediaMessage::Start {
            token,
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: pending.task,
        });
        let _ = pending.gate.try_send(());
    }

    fn build_audio_prepare_task(
        token: PreviewToken,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) -> PendingMediaPreparation {
        let lease = token.response().read_lease();
        let owner = cx.entity().downgrade();
        let (gate, start) = async_channel::bounded(1);
        let task = window.spawn(cx, async move |cx| {
            if start.recv().await.is_err() {
                return;
            }
            let worker = gpui_tokio::Tokio::spawn(cx, async move {
                let asset = lease
                    .materialize_media_asset()
                    .await
                    .map_err(media_asset_problem)?;
                AudioDriver::prepare(asset)
            });
            let result = match worker.await {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(problem)) => Err(problem),
                Err(_) => Err(MediaProblem::new(MediaProblemKind::Internal)),
            };
            let _ = owner.update_in(cx, |this, window, cx| {
                if !this.response_pane.is_current_preview(&token) {
                    return;
                }
                match result {
                    Ok(prepared) => {
                        let (driver, events, metadata) = prepared.into_parts();
                        let event_task =
                            Self::build_media_event_task(events, token.clone(), window, cx);
                        (&mut this.response_pane.media).transition(MediaMessage::Prepared {
                            token: token.clone(),
                            driver: Box::new(driver),
                            metadata,
                            task: event_task,
                        });
                    }
                    Err(problem) => {
                        (&mut this.response_pane.media).transition(MediaMessage::PrepareFailed {
                            token: token.clone(),
                            problem,
                        });
                    }
                }
                this.response_pane.sync_media_controls(window, cx);
                cx.notify();
            });
        });
        PendingMediaPreparation { task, gate }
    }

    fn build_media_event_task(
        events: MediaDriverEvents,
        token: PreviewToken,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) -> Task<()> {
        let owner = cx.entity().downgrade();
        window.spawn(cx, async move |cx| {
            loop {
                let event = events.recv().fuse();
                let tick = cx
                    .background_executor()
                    .timer(Duration::from_millis(250))
                    .fuse();
                futures_util::pin_mut!(event, tick);
                let event = match select(event, tick).await {
                    Either::Left((Ok(event), _)) => Some(event),
                    Either::Left((Err(_), _)) => {
                        let _ = owner.update_in(cx, |this, window, cx| {
                            if this.response_pane.is_current_preview(&token)
                                && this.response_pane.media.phase() != MediaPhase::Ended
                            {
                                this.response_pane.handle_media_driver_event(
                                    MediaDriverEvent::PlaybackFailed(MediaProblem::new(
                                        MediaProblemKind::Internal,
                                    )),
                                    token.clone(),
                                );
                                this.response_pane.sync_media_controls(window, cx);
                                cx.notify();
                            }
                        });
                        return;
                    }
                    Either::Right(((), _)) => None,
                };
                if owner
                    .update_in(cx, |this, window, cx| {
                        if !this.response_pane.is_current_preview(&token) {
                            return;
                        }
                        if let Some(event) = event {
                            this.response_pane
                                .handle_media_driver_event(event, token.clone());
                        } else {
                            (&mut this.response_pane.media).transition(MediaMessage::PollPosition);
                        }
                        this.response_pane.sync_media_controls(window, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
    }

    fn handle_media_driver_event(&mut self, event: MediaDriverEvent, token: PreviewToken) {
        let message = match event {
            MediaDriverEvent::Metadata(metadata) => MediaMessage::Metadata { token, metadata },
            MediaDriverEvent::Position(position) => MediaMessage::Position { token, position },
            MediaDriverEvent::Ended => MediaMessage::Ended { token },
            MediaDriverEvent::PlaybackFailed(problem) => {
                MediaMessage::PlaybackFailed { token, problem }
            }
        };
        (&mut self.media).transition(message);
    }

    fn sync_media_controls(&mut self, window: &mut Window, cx: &mut Context<RequestView>) {
        let Some(active) = self.media.active() else {
            return;
        };
        let position = active.position();
        let ratio = position.duration().and_then(|duration| {
            (!duration.is_zero()).then(|| {
                (position.position().as_secs_f64() / duration.as_secs_f64()).clamp(0., 1.) as f32
            })
        });
        let volume = active.volume();
        if !self.seek_dragging
            && let Some(ratio) = ratio
        {
            self.seek_state
                .update(cx, |state, cx| state.set_value(ratio, window, cx));
        }
        self.volume_state
            .update(cx, |state, cx| state.set_value(volume, window, cx));
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
        &mut self,
        runtime: &RequestRuntime,
        window: &mut Window,
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
            RequestRuntime::Ready { response } => self.render_ready(response, window, cx),
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
        &mut self,
        response: &Arc<ResponseData>,
        window: &mut Window,
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
                ResponseTab::Body => self.render_body_projection(window, cx),
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

    fn render_body_projection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RequestView>,
    ) -> AnyElement {
        if let Some(mode) = self
            .preview_token
            .as_ref()
            .map(PreviewToken::effective_mode)
        {
            match mode {
                ViewerMode::Audio => return self.render_media(cx),
                ViewerMode::Pdf => return self.render_pdf(window, cx),
                ViewerMode::Auto
                | ViewerMode::Text
                | ViewerMode::Json
                | ViewerMode::Xml
                | ViewerMode::Hex
                | ViewerMode::Base64
                | ViewerMode::Image => {}
            }
        }
        let warning = self
            .projection
            .as_ref()
            .and_then(ResponseProjection::warning)
            .map(|warning| warning_message(warning, cx));
        let projection = match &self.projection {
            None => centered_status(cx.global::<I18n>().t("response-sending")),
            Some(ResponseProjection::Empty) => centered_status(SharedString::default()),
            Some(ResponseProjection::Text { .. }) => self
                .text_editor
                .as_ref()
                .map(|editor| {
                    div()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .bg(cx.theme().input_background())
                        .child(
                            Editor::new(editor)
                                .readonly(true)
                                .appearance(false)
                                .size_full()
                                .font_family(cx.theme().mono_font_family.clone()),
                        )
                        .into_any_element()
                })
                .unwrap_or_else(|| {
                    centered_status(cx.global::<I18n>().t("response-viewer-mode-unavailable"))
                }),
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

    fn render_media(&self, cx: &mut Context<RequestView>) -> AnyElement {
        let phase = self.media.phase();
        let problem = self
            .media
            .problem()
            .map(|problem| media_problem_message(problem, cx));
        let active = self.media.active();
        let position_label = active.map(|active| {
            let position = active.position();
            let mut args = FluentArgs::new();
            args.set("current", format_media_duration(position.position()));
            args.set(
                "total",
                position
                    .duration()
                    .map(format_media_duration)
                    .unwrap_or_else(|| "—".to_owned()),
            );
            cx.global::<I18n>()
                .t_with_args("response-media-position", &args)
        });

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
            .when_some(problem, |this, problem| {
                this.child(Alert::error("response-media-problem", problem))
            })
            .when(phase == MediaPhase::Preparing, |this| {
                this.child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(Label::new(cx.global::<I18n>().t("response-media-loading")))
                        .child(Progress::new("response-media-loading-progress").loading(true)),
                )
            })
            .when(active.is_some(), |this| {
                this.child(div().flex_1().min_h(px(0.)))
            })
            .when_some(position_label, |this, position_label| {
                let active = self
                    .media
                    .active()
                    .expect("position label is only built for active media");
                let playing = phase == MediaPhase::Playing;
                let muted = active.muted();
                let seek_disabled = active.metadata().duration().is_none();
                this.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("response-media-play-pause")
                                .label(cx.global::<I18n>().t(if playing {
                                    "response-media-pause"
                                } else {
                                    "response-media-play"
                                }))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    (&mut this.response_pane.media).transition(if playing {
                                        MediaMessage::Pause
                                    } else {
                                        MediaMessage::Play
                                    });
                                    this.response_pane.sync_media_controls(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(Label::new(position_label))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(80.))
                                .child(Slider::new(&self.seek_state).disabled(seek_disabled)),
                        )
                        .child(div().w(px(100.)).child(Slider::new(&self.volume_state)))
                        .child(
                            Button::new("response-media-mute")
                                .label(cx.global::<I18n>().t(if muted {
                                    "response-media-unmute"
                                } else {
                                    "response-media-mute"
                                }))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    (&mut this.response_pane.media)
                                        .transition(MediaMessage::SetMuted(!muted));
                                    this.response_pane.sync_media_controls(window, cx);
                                    cx.notify();
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_pdf(&mut self, window: &mut Window, cx: &mut Context<RequestView>) -> AnyElement {
        let viewport = pdf_viewport(&self.pdf_viewport_handle, window);
        if self.pdf.current_page().is_some()
            && !self.pdf.is_loading()
            && self.pdf.viewport() != Some(viewport)
            && self.pdf_resize_pending != Some(viewport)
            && let Some(token) = self.preview_token.clone()
        {
            self.pdf_resize_pending = Some(viewport);
            cx.defer_in(window, move |this, _, cx| {
                if this.response_pane.pdf_resize_pending == Some(viewport) {
                    this.response_pane.pdf_resize_pending = None;
                    if this.response_pane.is_current_preview(&token)
                        && this.response_pane.pdf.viewport() != Some(viewport)
                    {
                        this.response_pane.pdf.rerender(viewport);
                        cx.notify();
                    }
                }
            });
        }
        let problem = self.pdf.problem().map(|problem| {
            let key = match problem.kind() {
                PdfProblemKind::Parse => "response-pdf-invalid",
                PdfProblemKind::Encrypted => "response-pdf-encrypted",
                PdfProblemKind::Budget => "response-pdf-too-large",
                PdfProblemKind::Render | PdfProblemKind::Internal => "response-pdf-render-failed",
            };
            cx.global::<I18n>().t(key)
        });
        let image = self.pdf.image().cloned();
        let page = self.pdf.current_page();
        let page_count = self.pdf.page_count();
        let page_label = match (page, page_count) {
            (Some(page), Some(page_count)) => {
                let mut args = FluentArgs::new();
                args.set("current", page.saturating_add(1));
                args.set("total", page_count);
                Some(cx.global::<I18n>().t_with_args("response-pdf-page", &args))
            }
            _ => None,
        };

        let preview = div()
            .id("response-pdf-viewport")
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .track_scroll(&self.pdf_viewport_handle)
            .when(self.pdf.is_loading() && image.is_none(), |this| {
                this.child(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(Label::new(cx.global::<I18n>().t("response-pdf-loading")))
                        .child(Progress::new("response-pdf-loading-progress").loading(true)),
                )
            })
            .when_some(image, |this, image| {
                this.child(img(image).object_fit(ObjectFit::Contain).size_full())
            });

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
            .when_some(problem, |this, problem| {
                this.child(Alert::error("response-pdf-problem", problem))
            })
            .child(preview)
            .when_some(page_label, |this, page_label| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            Button::new("response-pdf-previous")
                                .label(cx.global::<I18n>().t("response-pdf-previous"))
                                .disabled(!self.pdf.can_previous() || self.pdf.is_loading())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let viewport = pdf_viewport(
                                        &this.response_pane.pdf_viewport_handle,
                                        window,
                                    );
                                    this.response_pane.pdf.previous(viewport);
                                    cx.notify();
                                })),
                        )
                        .child(Label::new(page_label))
                        .child(
                            Button::new("response-pdf-next")
                                .label(cx.global::<I18n>().t("response-pdf-next"))
                                .disabled(!self.pdf.can_next() || self.pdf.is_loading())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let viewport = pdf_viewport(
                                        &this.response_pane.pdf_viewport_handle,
                                        window,
                                    );
                                    this.response_pane.pdf.next(viewport);
                                    cx.notify();
                                })),
                        ),
                )
            })
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

const fn media_asset_problem(problem: ResponseAssetProblem) -> MediaProblem {
    let kind = match problem.kind() {
        ResponseAssetProblemKind::ReadResponse => MediaProblemKind::AssetRead,
        ResponseAssetProblemKind::CreateTemporaryAsset
        | ResponseAssetProblemKind::WriteTemporaryAsset => MediaProblemKind::TemporaryAsset,
    };
    MediaProblem::new(kind)
}

fn media_problem_message(problem: MediaProblem, cx: &App) -> SharedString {
    let i18n = cx.global::<I18n>();
    match problem.kind() {
        MediaProblemKind::RuntimeUnavailable => i18n.t("response-media-runtime-unavailable").into(),
        MediaProblemKind::Decode => i18n.t("response-media-decode-failed").into(),
        MediaProblemKind::Control => i18n.t("response-media-control-failed").into(),
        MediaProblemKind::AssetRead
        | MediaProblemKind::TemporaryAsset
        | MediaProblemKind::Internal => i18n.t("response-viewer-mode-unavailable").into(),
    }
}

fn format_media_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours == 0 {
        format!("{minutes:02}:{seconds:02}")
    } else {
        format!("{hours}:{minutes:02}:{seconds:02}")
    }
}

fn viewer_mode_items(response: Option<&ResponseData>, cx: &App) -> ViewerModeItems {
    ViewerMode::ALL
        .into_iter()
        .map(|value| ViewerModeItem {
            value,
            title: cx.global::<I18n>().t(viewer_mode_key(value)).into(),
            disabled: response
                .is_some_and(|response| resolved_viewer_mode(response, value).is_none()),
        })
        .collect()
}

fn pdf_viewport(handle: &ScrollHandle, window: &Window) -> PdfViewport {
    let measured = handle.bounds().size;
    let viewport = if measured.width > px(0.) && measured.height > px(0.) {
        measured
    } else {
        window.viewport_size()
    };
    let scale = window.scale_factor();
    let width: f32 = viewport.width.into();
    let height: f32 = viewport.height.into();
    PdfViewport::new(
        (width * scale).ceil().max(1.0) as u32,
        (height * scale).ceil().max(1.0) as u32,
    )
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
        ViewerMode::Audio => "response-view-audio",
        ViewerMode::Pdf => "response-view-pdf",
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
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use bytes::Bytes;
    use gpui::TestAppContext;
    use http::{HeaderMap, StatusCode, Version};
    use url::Url;

    use super::*;
    use crate::foundation::i18n::init_i18n;

    struct FakeMediaDriver;

    impl media::MediaDriver for FakeMediaDriver {
        fn command(&mut self, _: media::MediaCommand) -> Result<(), MediaProblem> {
            Ok(())
        }
    }

    struct TaskDrop(Arc<AtomicBool>);

    impl Drop for TaskDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    fn pending_task(cx: &mut Context<RequestView>, dropped: Arc<AtomicBool>) -> Task<()> {
        cx.spawn(async move |_, _| {
            let _drop = TaskDrop(dropped);
            std::future::pending::<()>().await;
        })
    }

    fn initialize(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            gpui_tokio::init(cx);
        });
    }

    fn completed_response() -> Arc<ResponseData> {
        Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/media").unwrap(),
                HeaderMap::new(),
            ),
            ResponseTiming {
                head_after: Duration::from_millis(1),
                completed_after: Duration::from_millis(2),
            },
            CompletedBody {
                body: StoredBody::Memory(Bytes::from_static(b"media")),
                body_decoding: BodyDecoding::Identity,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(5),
                    received_encoded_bytes: 5,
                    stored_body_bytes: 5,
                },
            },
        ))
    }

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
                ViewerMode::Audio,
                ViewerMode::Pdf,
            ]
        );
    }

    #[gpui::test]
    fn text_projection_uses_a_selectable_read_only_editor_and_teardown_releases_it(
        cx: &mut TestAppContext,
    ) {
        initialize(cx);
        let (view, cx) = cx.add_window_view(RequestView::new);
        let response = completed_response();
        let source = "{\n  \"answer\": 42\n}";
        let editor = cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.runtime = RequestRuntime::Ready {
                    response: Arc::clone(&response),
                };
                view.response_pane
                    .begin_preview(response, ViewerMode::Json, window, cx);
                view.response_pane.install_projection(
                    ResponseProjection::Text {
                        source: source.into(),
                        language: SourceLanguage::Json,
                        warning: None,
                    },
                    window,
                    cx,
                );
                cx.notify();
                view.response_pane
                    .text_editor
                    .clone()
                    .expect("text projection must install an editor")
            })
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            let presentation = editor.read(cx).presentation();
            assert!(presentation.is_readonly());
            assert!(!presentation.is_disabled());
        });

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.set_selected_range(0..source.len(), cx);
                gpui::EntityInputHandler::replace_text_in_range(
                    editor, None, "changed", window, cx,
                );
            });
        });
        assert_eq!(cx.update(|_, cx| editor.read(cx).value()), source);
        assert_eq!(cx.update(|_, cx| editor.read(cx).selected_value()), source);

        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.response_pane.reset_for_send(window, cx);
                assert!(view.response_pane.text_editor.is_none());
            })
        });
    }

    #[gpui::test]
    fn unexpected_media_event_route_close_becomes_a_terminal_internal_problem(
        cx: &mut TestAppContext,
    ) {
        initialize(cx);
        let (view, cx) = cx.add_window_view(RequestView::new);
        let token = cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                let token = view.response_pane.begin_preview(
                    completed_response(),
                    ViewerMode::Audio,
                    window,
                    cx,
                );
                let preparation = cx.spawn(async |_, _| std::future::pending::<()>().await);
                (&mut view.response_pane.media).transition(MediaMessage::Start {
                    token: token.clone(),
                    resume_position: Duration::ZERO,
                    resume_playing: false,
                    task: preparation,
                });

                let (critical_sender, critical) = async_channel::bounded(1);
                let (telemetry_sender, telemetry) = async_channel::bounded(1);
                drop(critical_sender);
                drop(telemetry_sender);
                let event_task = ResponsePane::build_media_event_task(
                    MediaDriverEvents::from_lanes(critical, telemetry),
                    token.clone(),
                    window,
                    cx,
                );
                (&mut view.response_pane.media).transition(MediaMessage::Prepared {
                    token: token.clone(),
                    driver: Box::new(FakeMediaDriver),
                    metadata: media::MediaMetadata::new(None),
                    task: event_task,
                });
                token
            })
        });

        cx.run_until_parked();
        cx.update(|_, cx| {
            let view = view.read(cx);
            assert!(view.response_pane.is_current_preview(&token));
            assert_eq!(view.response_pane.media.phase(), MediaPhase::Failed);
            assert_eq!(
                view.response_pane.media.problem().map(MediaProblem::kind),
                Some(MediaProblemKind::Internal)
            );
        });
    }

    #[gpui::test]
    fn media_and_pdf_preview_teardown_reject_stale_work_without_autoplay(cx: &mut TestAppContext) {
        initialize(cx);
        let (view, cx) = cx.add_window_view(RequestView::new);
        let response = completed_response();
        let audio_task_dropped = Arc::new(AtomicBool::new(false));

        let audio_token = cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                let token = view.response_pane.begin_preview(
                    Arc::clone(&response),
                    ViewerMode::Audio,
                    window,
                    cx,
                );
                (&mut view.response_pane.media).transition(MediaMessage::Start {
                    token: token.clone(),
                    resume_position: Duration::ZERO,
                    resume_playing: false,
                    task: pending_task(cx, Arc::clone(&audio_task_dropped)),
                });
                assert_eq!(view.response_pane.mode(), ViewerMode::Auto);
                assert_eq!(view.response_pane.media.phase(), MediaPhase::Preparing);
                token
            })
        });
        cx.run_until_parked();

        let pdf_token = cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                // A fresh Auto PDF preview uses the same mode-switch teardown
                // entry point as the audio player.
                view.response_pane.mode = ViewerMode::Auto;
                let token = view.response_pane.begin_preview(
                    Arc::clone(&response),
                    ViewerMode::Pdf,
                    window,
                    cx,
                );
                assert!(!token.matches(&audio_token));
                assert_eq!(view.response_pane.media.phase(), MediaPhase::Idle);

                // A completion from the cancelled Auto audio preparation
                // cannot install a driver after switching to PDF.
                (&mut view.response_pane.media).transition(MediaMessage::Prepared {
                    token: audio_token.clone(),
                    driver: Box::new(FakeMediaDriver),
                    metadata: media::MediaMetadata::new(None),
                    task: pending_task(cx, Arc::new(AtomicBool::new(false))),
                });
                assert_eq!(view.response_pane.media.phase(), MediaPhase::Idle);

                // PDF installs its worker-owned Loading state before the
                // parser can poll; PDFs have no playback transition.
                view.response_pane.pdf.begin_read(token.clone());
                let worker = PdfWorkerHandle::new(Bytes::from_static(b"not a PDF")).unwrap();
                view.response_pane.pdf.load(
                    token.clone(),
                    worker,
                    pending_task(cx, Arc::new(AtomicBool::new(false))),
                    PdfViewport::new(1, 1),
                );
                assert!(view.response_pane.pdf.is_loading());
                token
            })
        });
        cx.run_until_parked();
        assert!(audio_task_dropped.load(Ordering::Acquire));

        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                // `reset_for_send` is the new-Send teardown path. Stale
                // audio completions cannot revive a stopped preview.
                view.response_pane.reset_for_send(window, cx);
                assert_eq!(view.response_pane.mode(), ViewerMode::Auto);
                assert!(!view.response_pane.is_current_preview(&audio_token));
                assert_eq!(view.response_pane.media.phase(), MediaPhase::Idle);
                (&mut view.response_pane.media).transition(MediaMessage::Prepared {
                    token: audio_token.clone(),
                    driver: Box::new(FakeMediaDriver),
                    metadata: media::MediaMetadata::new(None),
                    task: pending_task(cx, Arc::new(AtomicBool::new(false))),
                });
                assert_eq!(view.response_pane.media.phase(), MediaPhase::Idle);

                // Clear uses the same final teardown and invalidates the PDF
                // identity even while it is still in the pre-worker read.
                view.response_pane.clear_projection();
                assert!(!view.response_pane.is_current_preview(&pdf_token));
                assert!(!view.response_pane.pdf.is_loading());
            })
        });
    }
}
