use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _,
    label::Label,
    scroll::ScrollableElement as _,
    select::{SelectItem, SelectState},
    separator::Separator,
    v_flex,
};
use gpui_form::{Form, FormEvent};

use crate::{
    features::request::{
        controls::FormCaseSelect,
        draft::{RequestBodyDraft, RequestDraft},
    },
    foundation::I18n,
};

use self::{
    binary::BinaryBodyView, form_data::FormDataView, http_text::HttpTextView,
    x_form::UrlEncodedView,
};

mod binary;
mod form_data;
mod http_text;
mod x_form;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyKind {
    None,
    FormData,
    UrlEncoded,
    Text,
    Binary,
}

#[derive(Clone)]
struct BodyOption {
    kind: BodyKind,
    title: SharedString,
}

impl SelectItem for BodyOption {
    type Value = BodyKind;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.kind
    }
}

type BodyOptions = Vec<BodyOption>;

enum ActiveBody {
    None,
    FormData(Entity<FormDataView>),
    UrlEncoded(Entity<UrlEncodedView>),
    Text(Entity<HttpTextView>),
    Binary(Entity<BinaryBodyView>),
}

impl ActiveBody {
    fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    fn element(&self) -> AnyElement {
        match self {
            Self::None => div().flex_1().into_any_element(),
            Self::FormData(view) => view.clone().into_any_element(),
            Self::UrlEncoded(view) => view.clone().into_any_element(),
            Self::Text(view) => view.clone().into_any_element(),
            Self::Binary(view) => view.clone().into_any_element(),
        }
    }
}

pub(crate) struct HttpBodyView {
    form: Entity<Form<RequestDraft>>,
    body_kind: BodyKind,
    body_select: FormCaseSelect<RequestDraft, BodyOptions, RequestBodyDraft, BodyKind>,
    active: ActiveBody,
    _subscription: Subscription,
}

impl HttpBodyView {
    pub(crate) fn new(
        form: Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = body_options(cx);
        let body_select = FormCaseSelect::new(
            &form,
            RequestDraft::BODY,
            body_kind,
            body_for_kind,
            move |window, cx| SelectState::new(options, None, window, cx),
            window,
            cx,
        );
        let initial_body_kind = body_kind(&RequestDraft::BODY.get(&form, cx));
        let active = build_active_body(&form, initial_body_kind, window, cx);
        let subscription = cx.subscribe_in(
            &form,
            window,
            |this, _, event: &FormEvent<RequestDraft>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                if !change.impact(&RequestDraft::BODY).structure_changed() {
                    return;
                }
                let kind = body_kind(&RequestDraft::BODY.get(&this.form, cx));
                if kind != this.body_kind {
                    this.body_kind = kind;
                    this.active = build_active_body(&this.form, kind, window, cx);
                    cx.notify();
                }
            },
        );
        Self {
            form,
            body_kind: initial_body_kind,
            body_select,
            active,
            _subscription: subscription,
        }
    }
}

impl Render for HttpBodyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = cx.global::<I18n>().t("tab-body");
        let active = self.active.element();
        let active = if self.active.is_text() {
            v_flex()
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .child(active)
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .child(active)
                .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .p_2()
                    .gap_2()
                    .child(Label::new(label).text_color(cx.theme().muted_foreground))
                    .child(div().w(px(280.)).child(self.body_select.element())),
            )
            .child(Separator::horizontal())
            .child(active)
    }
}

fn body_kind(body: &RequestBodyDraft) -> BodyKind {
    match body {
        RequestBodyDraft::None => BodyKind::None,
        RequestBodyDraft::FormData(_) => BodyKind::FormData,
        RequestBodyDraft::UrlEncoded(_) => BodyKind::UrlEncoded,
        RequestBodyDraft::Text(_) => BodyKind::Text,
        RequestBodyDraft::Binary(_) => BodyKind::Binary,
    }
}

fn body_for_kind(kind: BodyKind) -> RequestBodyDraft {
    match kind {
        BodyKind::None => RequestBodyDraft::None,
        BodyKind::FormData => RequestBodyDraft::form_data(),
        BodyKind::UrlEncoded => RequestBodyDraft::url_encoded(),
        BodyKind::Text => RequestBodyDraft::text(),
        BodyKind::Binary => RequestBodyDraft::binary(),
    }
}

fn body_options(cx: &gpui::App) -> BodyOptions {
    let i18n = cx.global::<I18n>();
    [
        (BodyKind::None, "body-none"),
        (BodyKind::FormData, "body-form-data"),
        (BodyKind::UrlEncoded, "body-urlencoded"),
        (BodyKind::Text, "body-text"),
        (BodyKind::Binary, "body-binary"),
    ]
    .into_iter()
    .map(|(kind, key)| BodyOption {
        kind,
        title: i18n.t(key).into(),
    })
    .collect()
}

fn build_active_body<Owner: 'static>(
    form: &Entity<Form<RequestDraft>>,
    kind: BodyKind,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> ActiveBody {
    let body = RequestDraft::BODY;
    match kind {
        BodyKind::None => ActiveBody::None,
        BodyKind::FormData => body
            .case(RequestBodyDraft::FORM_DATA)
            .resolve(form, cx)
            .ok()
            .flatten()
            .map(|path| cx.new(|cx| FormDataView::new(form.clone(), path, window, cx)))
            .map(ActiveBody::FormData)
            .unwrap_or(ActiveBody::None),
        BodyKind::UrlEncoded => body
            .case(RequestBodyDraft::URL_ENCODED)
            .resolve(form, cx)
            .ok()
            .flatten()
            .map(|path| cx.new(|cx| UrlEncodedView::new(form.clone(), path, window, cx)))
            .map(ActiveBody::UrlEncoded)
            .unwrap_or(ActiveBody::None),
        BodyKind::Text => body
            .case(RequestBodyDraft::TEXT)
            .resolve(form, cx)
            .ok()
            .flatten()
            .map(|path| cx.new(|cx| HttpTextView::new(form.clone(), path, window, cx)))
            .map(ActiveBody::Text)
            .unwrap_or(ActiveBody::None),
        BodyKind::Binary => body
            .case(RequestBodyDraft::BINARY)
            .resolve(form, cx)
            .ok()
            .flatten()
            .map(|path| cx.new(|cx| BinaryBodyView::new(form.clone(), path, window, cx)))
            .map(ActiveBody::Binary)
            .unwrap_or(ActiveBody::None),
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref as _;

    use gpui::{TestAppContext, VisualTestContext};
    use gpui_component::select::SelectEvent;
    use gpui_form::ResolveError;

    use crate::{
        features::request::{
            draft::{
                BinaryBodyDraft, FormDataDraft, KeyValueDraft, MultipartPartDraft,
                MultipartPartValueDraft, MultipartTextDraft, TextBodyDraft, UrlEncodedBodyDraft,
            },
            validation::RequestValidator,
        },
        foundation::i18n::init_i18n,
    };

    use super::*;

    #[gpui::test]
    fn all_body_cases_prepare_and_case_switch_retires_old_dynamic_paths(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let draft = RequestDraft {
                url: "https://example.com".into(),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft).with_validator(RequestValidator));

            for body in [
                RequestBodyDraft::None,
                RequestBodyDraft::Text(TextBodyDraft::default()),
                RequestBodyDraft::UrlEncoded(UrlEncodedBodyDraft {
                    fields: vec![KeyValueDraft::default()],
                }),
                RequestBodyDraft::FormData(FormDataDraft {
                    parts: vec![MultipartPartDraft {
                        enabled: true,
                        name: "field".into(),
                        value: MultipartPartValueDraft::Text(MultipartTextDraft::default()),
                    }],
                }),
            ] {
                RequestDraft::BODY.set(&form, body, cx);
                form.update(cx, |form, cx| form.prepare(cx))
                    .expect("active non-file body case prepares");
            }

            let file = tempfile::NamedTempFile::new().expect("temporary body file");
            RequestDraft::BODY.set(
                &form,
                RequestBodyDraft::Binary(BinaryBodyDraft {
                    file: Some(file.path().to_path_buf()),
                }),
                cx,
            );
            form.update(cx, |form, cx| form.prepare(cx))
                .expect("Binary prepares with a live absolute file");

            RequestDraft::BODY.set(&form, RequestBodyDraft::text(), cx);
            let old_content = RequestDraft::BODY
                .case(RequestBodyDraft::TEXT)
                .resolve(&form, cx)
                .expect("resolve Text")
                .expect("Text active")
                .then(TextBodyDraft::CONTENT);
            RequestDraft::BODY.set(&form, RequestBodyDraft::binary(), cx);
            assert!(matches!(
                old_content.try_get(&form, cx),
                Err(ResolveError::Retired { .. })
            ));
        });
    }

    #[gpui::test]
    fn confirming_a_new_body_kind_twice_before_self_projection_does_not_reset_its_payload(
        cx: &mut TestAppContext,
    ) {
        let (form, window) = cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            let draft = RequestDraft {
                body: RequestBodyDraft::Text(TextBodyDraft::default()),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            let view_form = form.clone();
            let window = cx
                .open_window(Default::default(), move |window, cx| {
                    cx.new(|cx| HttpBodyView::new(view_form, window, cx))
                })
                .expect("open Body test window");
            (form, window)
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Body root");
        let select =
            cx.update(|_, cx| root.read_with(cx, |view, _| view.body_select.deref().clone()));
        cx.update(|_, cx| {
            select.update(cx, |_, cx| {
                cx.emit(SelectEvent::Confirm(Some(BodyKind::Binary)));
                // This second event arrives before the first deferred form write can project back
                // to the select. It must already be recognized as a same-kind confirmation.
                cx.emit(SelectEvent::Confirm(Some(BodyKind::Binary)));
            });
        });
        cx.run_until_parked();
        let selected = std::env::temp_dir().join("http-client-repeat-confirm.bin");
        cx.update(|_, cx| {
            let binary = RequestDraft::BODY
                .case(RequestBodyDraft::BINARY)
                .resolve(&form, cx)
                .expect("resolve Binary")
                .expect("Binary is active");
            binary
                .then(BinaryBodyDraft::FILE)
                .try_set(&form, Some(selected.clone()), cx)
                .expect("set Binary payload");

            // The external projection back to the select is still deferred here. The adapter's
            // local kind must already be Binary, so a second Confirm cannot rebuild the case.
            select.update(cx, |_, cx| {
                cx.emit(SelectEvent::Confirm(Some(BodyKind::Binary)))
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| match RequestDraft::BODY.get(&form, cx) {
            RequestBodyDraft::Binary(binary) => assert_eq!(binary.file, Some(selected)),
            _ => panic!("same-kind confirmation must not change the newly active case"),
        });
    }
}
