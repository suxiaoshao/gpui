use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, Styled, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    label::Label,
    select::SelectState,
    v_flex,
};
use gpui_form::Form;

use self::{
    controls::FormScalarSelect,
    draft::{HttpClientTransportSettings, RequestDraft},
    method::{HttpMethod, SelectHttpMethod},
    prepared::{PreparedRequest, RequestPrepareError, compile_request},
    tab::RequestTabsView,
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
#[allow(dead_code)]
mod prepared;
mod settings;
mod tab;
mod url_input;
mod validation;

pub(crate) struct RequestView {
    form: Entity<Form<RequestDraft>>,
    transport_settings: HttpClientTransportSettings,
    method: FormScalarSelect<RequestDraft, SelectHttpMethod, HttpMethod>,
    url: UrlInput,
    tabs: Entity<RequestTabsView>,
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
        Self {
            form,
            transport_settings,
            method,
            url,
            tabs,
            _form_observer: form_observer,
            focus_handle: cx.focus_handle(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn prepare_request(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<PreparedRequest, RequestPrepareError> {
        let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
        let (_, draft) = prepared.into_parts();
        compile_request(draft, &self.transport_settings).map_err(Into::into)
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
                    .disabled(true),
            );

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .child(request_line)
            .child(self.tabs.clone())
    }
}

#[cfg(test)]
mod tests;
