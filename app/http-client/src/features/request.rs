use std::{sync::Arc, time::Instant};

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, Styled, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    label::Label,
    resizable::{resizable_panel, v_resizable},
    select::SelectState,
    v_flex,
};
use gpui_form::Form;
use gpui_operation::Transition as _;

use self::{
    controls::FormScalarSelect,
    draft::{HttpClientTransportSettings, RequestDraft},
    method::{HttpMethod, SelectHttpMethod},
    prepared::{PreparedRequest, RequestPrepareError, compile_request},
    response::{
        ResponsePane, ResponseProjection, ResponseSaveProblem, ResponseViewWarning,
        initial_save_directory, project_response, save_response, suggested_response_name,
    },
    runtime::{HttpRunEffect, HttpRunMessage, RequestProblem, RequestRuntime},
    tab::RequestTabsView,
    transport::{HttpTransport, WorkerEvent},
    url_input::UrlInput,
    validation::RequestValidator,
};
use crate::foundation::{I18n, validation_message};

mod auth;
mod body;
mod controls;
mod draft;
mod headers;
mod method;
mod params;
mod prepared;
mod response;
mod runtime;
mod settings;
mod tab;
mod transport;
mod url_input;
mod validation;

pub(crate) struct RequestView {
    form: Entity<Form<RequestDraft>>,
    transport_settings: HttpClientTransportSettings,
    method: FormScalarSelect<RequestDraft, SelectHttpMethod, HttpMethod>,
    url: UrlInput,
    tabs: Entity<RequestTabsView>,
    transport: HttpTransport,
    runtime: RequestRuntime,
    response_pane: ResponsePane,
    _form_observer: Subscription,
    focus_handle: FocusHandle,
}

impl RequestView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| Form::new(RequestDraft::default()).with_validator(RequestValidator));
        let transport_settings = HttpClientTransportSettings::default();
        let method = FormScalarSelect::new(
            &form,
            RequestDraft::METHOD,
            |window, cx| SelectState::new(SelectHttpMethod, None, window, cx),
            window,
            cx,
        );
        let url = UrlInput::new(&form, window, cx);
        let tabs =
            cx.new(|cx| RequestTabsView::new(form.clone(), transport_settings.clone(), window, cx));
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
        let response_pane = ResponsePane::new(window, cx);
        Self {
            form,
            transport_settings,
            method,
            url,
            tabs,
            transport: HttpTransport::new(),
            runtime: RequestRuntime::new(),
            response_pane,
            _form_observer: form_observer,
            focus_handle: cx.focus_handle(),
        }
    }

    pub(crate) fn prepare_request(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<PreparedRequest, RequestPrepareError> {
        let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
        let (_, draft) = prepared.into_parts();
        compile_request(draft, &self.transport_settings).map_err(Into::into)
    }

    fn start_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.runtime.is_running() {
            return;
        }
        let Ok(prepared) = self.prepare_request(cx) else {
            return;
        };

        let transport = self.transport.clone();
        let owner = cx.entity().downgrade();
        let started_at = Instant::now();
        let task = window.spawn(cx, async move |cx| {
            let (sender, receiver) = HttpTransport::channel();
            let worker = gpui_tokio::Tokio::spawn(cx, transport.run(prepared, sender));
            let mut terminal_seen = false;
            while let Ok(event) = receiver.recv().await {
                terminal_seen = matches!(event, WorkerEvent::Finished { .. });
                if owner
                    .update_in(cx, |this, window, cx| {
                        this.handle_worker_event(event, window, cx)
                    })
                    .is_err()
                {
                    return;
                }
                if terminal_seen {
                    break;
                }
            }
            let worker_result = worker.await;
            if !terminal_seen {
                let _ = owner.update_in(cx, |this, window, cx| {
                    if this.runtime.is_running() {
                        tracing::debug!(
                            operation = "http-request",
                            worker_join_failed = worker_result.is_err(),
                            "request worker ended without a terminal event"
                        );
                        this.handle_run_message(
                            HttpRunMessage::Finished {
                                result: Err(RequestProblem::internal()),
                                finished_after: started_at.elapsed(),
                            },
                            Some(window),
                            cx,
                        );
                    }
                });
            }
        });

        let effect = (&mut self.runtime).transition(HttpRunMessage::Start { task, started_at });
        if effect == HttpRunEffect::Started {
            self.response_pane.reset_for_send(window, cx);
            cx.notify();
        }
    }

    fn cancel_request(&mut self, cx: &mut Context<Self>) {
        self.handle_run_message(HttpRunMessage::Cancel, None, cx);
    }

    fn clear_response(&mut self, cx: &mut Context<Self>) {
        let effect = (&mut self.runtime).transition(HttpRunMessage::Clear);
        if effect != HttpRunEffect::Ignored {
            self.response_pane.clear_projection();
            cx.notify();
        }
    }

    fn handle_worker_event(
        &mut self,
        event: WorkerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = match event {
            WorkerEvent::HeadReceived {
                head,
                head_after,
                progress,
            } => HttpRunMessage::HeadReceived {
                head,
                head_after,
                progress,
            },
            WorkerEvent::BodyProgress(progress) => HttpRunMessage::BodyProgress(progress),
            WorkerEvent::Finished {
                result,
                finished_after,
            } => HttpRunMessage::Finished {
                result,
                finished_after,
            },
        };
        self.handle_run_message(message, Some(window), cx);
    }

    fn handle_run_message(
        &mut self,
        message: HttpRunMessage,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let effect = (&mut self.runtime).transition(message);
        if effect == HttpRunEffect::Ignored {
            return;
        }
        if effect == HttpRunEffect::Ready
            && let Some(window) = window
        {
            self.refresh_response_projection(window, cx);
        }
        cx.notify();
    }

    fn refresh_response_projection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(response) = self.runtime.response().cloned() else {
            self.response_pane.clear_projection();
            return;
        };
        let mode = self.response_pane.mode();
        let source = Arc::clone(&response);
        let projection = gpui_tokio::Tokio::spawn(cx, project_response(response, mode));
        let owner = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let projection = match projection.await {
                Ok(Ok(projection)) => projection,
                Ok(Err(_)) | Err(_) => {
                    ResponseProjection::Unavailable(ResponseViewWarning::ModeUnavailable)
                }
            };
            let _ = owner.update_in(cx, |this, _, cx| {
                let is_current = this
                    .runtime
                    .response()
                    .is_some_and(|current| Arc::ptr_eq(current, &source))
                    && this.response_pane.mode() == mode;
                if is_current {
                    this.response_pane.install_projection(projection);
                    cx.notify();
                }
            });
        });
        self.response_pane.install_preview_task(task);
        cx.notify();
    }

    fn start_response_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.response_pane.save_is_running() {
            return;
        }
        let Some(response) = self.runtime.response().cloned() else {
            return;
        };
        let content_type = response
            .head()
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        let suggested = suggested_response_name(content_type);
        let prompt = cx.prompt_for_new_path(&initial_save_directory(), Some(suggested));
        let owner = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let target = match prompt.await {
                Ok(Ok(Some(target))) => target,
                Ok(Ok(None)) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        this.response_pane.cancel_save_prompt();
                        cx.notify();
                    });
                    return;
                }
                Ok(Err(_)) | Err(_) => {
                    let _ = owner.update_in(cx, |this, _, cx| {
                        this.response_pane
                            .finish_save(Err(ResponseSaveProblem::prompt_failed()));
                        cx.notify();
                    });
                    return;
                }
            };
            let saved = gpui_tokio::Tokio::spawn(cx, save_response(response.read_lease(), target));
            let result = match saved.await {
                Ok(result) => result,
                Err(_) => Err(ResponseSaveProblem::task_failed()),
            };
            let _ = owner.update_in(cx, |this, _, cx| {
                this.response_pane.finish_save(result);
                cx.notify();
            });
        });
        self.response_pane.install_save_task(task);
        cx.notify();
    }
}

impl gpui::Render for RequestView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_label = {
            let i18n = cx.global::<I18n>();
            i18n.t("button-send")
        };
        let url_error = RequestDraft::URL
            .errors(&self.form, cx)
            .first()
            .map(|issue| validation_message(issue.message(), cx));

        let request_line = div()
            .flex()
            .items_start()
            .gap_2()
            .p_2()
            .child(div().w(px(112.)).child(self.method.element()))
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(self.url.element())
                    .when_some(url_error, |this, error| {
                        this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
                    }),
            )
            .child(
                Button::new("request-send")
                    .primary()
                    .label(send_label)
                    .disabled(self.runtime.is_running())
                    .on_click(cx.listener(|this, _, window, cx| this.start_request(window, cx))),
            )
            .when(self.runtime.is_running(), |this| {
                this.child(
                    Button::new("request-cancel")
                        .danger()
                        .label(cx.global::<I18n>().t("button-cancel"))
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_request(cx))),
                )
            });

        let request_editor = div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            .overflow_hidden()
            .child(request_line)
            .child(self.tabs.clone());
        let response = self.response_pane.render(&self.runtime, cx);

        div().track_focus(&self.focus_handle).size_full().child(
            v_resizable("request-response")
                .child(resizable_panel().child(request_editor))
                .child(
                    resizable_panel()
                        .size(px(320.))
                        .size_range(px(160.)..Pixels::MAX)
                        .child(response),
                ),
        )
    }
}

#[cfg(test)]
mod tests;
