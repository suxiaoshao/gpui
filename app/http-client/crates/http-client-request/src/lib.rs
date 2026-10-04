#[cfg(test)]
use http_client_core as response;
use http_client_core::ResponseState;
use http_client_core::transport::{HttpTransport, WorkerEvent};
mod localization;
use std::time::Instant;

use gpui_form::Form;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    label::Label,
    select::SelectState,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, Styled, Subscription, Window, div, prelude::FluentBuilder as _,
};
use gpui_operation::Transition as _;

use self::{
    controls::FormScalarSelect,
    draft::{HttpClientTransportSettings, RequestDraft},
    method::{HttpMethod, SelectHttpMethod},
    prepared::{PreparedRequest, RequestPrepareError, compile_request},
    runtime::{HttpRunEffect, HttpRunMessage, RequestProblem, RequestRuntime},
    tab::RequestTabsView,
    url_input::UrlInput,
    validation::RequestValidator,
};
use crate::localization::{I18n, validation_message};

mod auth;
mod body;
mod controls;
mod draft;
mod headers;
mod method;
mod params;
mod prepared;
mod runtime;
mod settings;
mod tab;
mod url_input;
mod validation;

pub struct RequestView {
    form: Entity<Form<RequestDraft>>,
    transport_settings: HttpClientTransportSettings,
    method: FormScalarSelect<RequestDraft, SelectHttpMethod, HttpMethod>,
    url: UrlInput,
    tabs: Entity<RequestTabsView>,
    transport: HttpTransport,
    runtime: RequestRuntime,
    save_is_running: bool,
    _form_observer: Subscription,
    focus_handle: FocusHandle,
}

pub enum RequestEvent {
    ResponseChanged(ResponseState),
}
impl gpui_kit::EventEmitter<RequestEvent> for RequestView {}
impl RequestView {
    pub fn set_save_activity(&mut self, running: bool, cx: &mut Context<Self>) {
        if self.save_is_running != running {
            self.save_is_running = running;
            cx.notify();
        }
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        Self {
            form,
            transport_settings,
            method,
            url,
            tabs,
            transport: HttpTransport::new(),
            runtime: RequestRuntime::new(),
            save_is_running: false,
            _form_observer: form_observer,
            focus_handle: cx.focus_handle(),
        }
    }

    fn prepare_request(
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
                let _ = owner.update_in(cx, |this, _window, cx| {
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
                            cx,
                        );
                    }
                });
            }
        });

        let effect = (&mut self.runtime).transition(HttpRunMessage::Start { task, started_at });
        if effect == HttpRunEffect::Started {
            cx.emit(RequestEvent::ResponseChanged(ResponseState::Sending));
            cx.notify();
        }
    }

    fn cancel_request(&mut self, cx: &mut Context<Self>) {
        self.handle_run_message(HttpRunMessage::Cancel, cx);
    }

    pub fn clear_response(&mut self, cx: &mut Context<Self>) {
        let effect = (&mut self.runtime).transition(HttpRunMessage::Clear);
        if effect != HttpRunEffect::Ignored {
            cx.emit(RequestEvent::ResponseChanged(ResponseState::Idle));
            cx.notify();
        }
    }

    fn handle_worker_event(
        &mut self,
        event: WorkerEvent,
        _window: &mut Window,
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
        self.handle_run_message(message, cx);
    }

    fn handle_run_message(&mut self, message: HttpRunMessage, cx: &mut Context<Self>) {
        let effect = (&mut self.runtime).transition(message);
        if effect == HttpRunEffect::Ignored {
            return;
        }
        cx.emit(RequestEvent::ResponseChanged(self.runtime.presentation()));
        cx.notify();
    }

    fn send_is_disabled(&self) -> bool {
        self.runtime.is_running() || self.save_is_running
    }
}

impl gpui_kit::Render for RequestView {
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
            .child(div().w(gpui_kit::rems(7.0)).child(self.method.element()))
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
                    .disabled(self.send_is_disabled())
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
            .min_h(gpui_kit::rems(0.0))
            .overflow_hidden()
            .child(request_line)
            .child(self.tabs.clone());
        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .child(request_editor)
    }
}

#[cfg(test)]
mod tests;
