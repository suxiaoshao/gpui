use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled,
    Subscription, Window, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, IndexPath,
    form::{field, v_form},
    h_flex,
    input::{Input, InputContentType, InputState},
    label::Label,
    select::{SelectItem, SelectState},
    v_flex,
};
use gpui_form::{DynamicPath, Form, FormEvent};
use gpui_form_gpui_component::FormInput;

use super::{
    controls::{FormCaseSelect, FormScalarSelect},
    draft::{
        ApiKeyAuthDraft, ApiKeyLocation, BasicAuthDraft, BearerAuthDraft, RequestAuthDraft,
        RequestDraft,
    },
};
use crate::foundation::{I18n, validation_message};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthKind {
    None,
    Basic,
    Bearer,
    ApiKey,
}

#[derive(Clone)]
struct AuthOption {
    kind: AuthKind,
    title: SharedString,
}

impl SelectItem for AuthOption {
    type Value = AuthKind;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.kind
    }
}

fn auth_kind(value: &RequestAuthDraft) -> AuthKind {
    match value {
        RequestAuthDraft::None => AuthKind::None,
        RequestAuthDraft::Basic(_) => AuthKind::Basic,
        RequestAuthDraft::Bearer(_) => AuthKind::Bearer,
        RequestAuthDraft::ApiKey(_) => AuthKind::ApiKey,
    }
}

fn build_auth(kind: AuthKind) -> RequestAuthDraft {
    match kind {
        AuthKind::None => RequestAuthDraft::None,
        AuthKind::Basic => RequestAuthDraft::basic(),
        AuthKind::Bearer => RequestAuthDraft::bearer(),
        AuthKind::ApiKey => RequestAuthDraft::api_key(),
    }
}

#[derive(Clone)]
struct ApiKeyLocationOption {
    location: ApiKeyLocation,
    title: SharedString,
}

impl SelectItem for ApiKeyLocationOption {
    type Value = ApiKeyLocation;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.location
    }
}

enum AuthFields {
    None,
    Basic {
        username_path: DynamicPath<RequestDraft, String>,
        username: FormInput,
        password_path: DynamicPath<RequestDraft, String>,
        password: FormInput,
    },
    Bearer {
        token_path: DynamicPath<RequestDraft, String>,
        token: FormInput,
    },
    ApiKey {
        name_path: DynamicPath<RequestDraft, String>,
        name: FormInput,
        value_path: DynamicPath<RequestDraft, String>,
        value: FormInput,
        location_path: DynamicPath<RequestDraft, ApiKeyLocation>,
        location: FormScalarSelect<RequestDraft, Vec<ApiKeyLocationOption>, ApiKeyLocation>,
    },
}

pub(super) struct AuthView {
    form: Entity<Form<RequestDraft>>,
    kind: FormCaseSelect<RequestDraft, Vec<AuthOption>, RequestAuthDraft, AuthKind>,
    fields: AuthFields,
    _subscription: Subscription,
}

impl AuthView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = auth_options(cx);
        let kind = FormCaseSelect::new(
            &form,
            RequestDraft::AUTH,
            auth_kind,
            build_auth,
            move |window, cx| SelectState::new(options, None, window, cx),
            window,
            cx,
        );
        let fields = Self::build_fields(&form, window, cx);
        let subscription = cx.subscribe_in(
            &form,
            window,
            |this, _, event: &FormEvent<RequestDraft>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    cx.notify();
                    return;
                };
                let impact = change.impact(&RequestDraft::AUTH);
                if impact.structure_changed() || impact.retired() {
                    this.fields = Self::build_fields(&this.form, window, cx);
                }
                cx.notify();
            },
        );
        Self {
            form,
            kind,
            fields,
            _subscription: subscription,
        }
    }

    fn build_fields(
        form: &Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AuthFields {
        match RequestDraft::AUTH.get(form, cx) {
            RequestAuthDraft::None => AuthFields::None,
            RequestAuthDraft::Basic(_) => {
                let basic = RequestDraft::AUTH
                    .case(RequestAuthDraft::BASIC)
                    .resolve(form, cx)
                    .expect("the auth path must resolve")
                    .expect("Basic payload must be active");
                let username_path = basic.clone().then(BasicAuthDraft::USERNAME);
                let password_path = basic.then(BasicAuthDraft::PASSWORD);
                let username =
                    FormInput::try_new(form, username_path.clone(), InputState::new, window, cx)
                        .expect("active Basic username must bind");
                let password = FormInput::try_new(
                    form,
                    password_path.clone(),
                    |window, cx| InputState::new(window, cx).masked(true),
                    window,
                    cx,
                )
                .expect("active Basic password must bind");
                AuthFields::Basic {
                    username_path,
                    username,
                    password_path,
                    password,
                }
            }
            RequestAuthDraft::Bearer(_) => {
                let bearer = RequestDraft::AUTH
                    .case(RequestAuthDraft::BEARER)
                    .resolve(form, cx)
                    .expect("the auth path must resolve")
                    .expect("Bearer payload must be active");
                let token_path = bearer.then(BearerAuthDraft::TOKEN);
                let token = FormInput::try_new(
                    form,
                    token_path.clone(),
                    |window, cx| InputState::new(window, cx).masked(true),
                    window,
                    cx,
                )
                .expect("active Bearer token must bind");
                AuthFields::Bearer { token_path, token }
            }
            RequestAuthDraft::ApiKey(_) => {
                let api_key = RequestDraft::AUTH
                    .case(RequestAuthDraft::API_KEY)
                    .resolve(form, cx)
                    .expect("the auth path must resolve")
                    .expect("API Key payload must be active");
                let name_path = api_key.clone().then(ApiKeyAuthDraft::NAME);
                let value_path = api_key.clone().then(ApiKeyAuthDraft::VALUE);
                let location_path = api_key.then(ApiKeyAuthDraft::LOCATION);
                let name = FormInput::try_new(form, name_path.clone(), InputState::new, window, cx)
                    .expect("active API Key name must bind");
                let value = FormInput::try_new(
                    form,
                    value_path.clone(),
                    |window, cx| InputState::new(window, cx).masked(true),
                    window,
                    cx,
                )
                .expect("active API Key value must bind");
                let location_options = api_key_location_options(cx);
                let location = FormScalarSelect::try_new(
                    form,
                    location_path.clone(),
                    |window, cx| {
                        SelectState::new(location_options, Some(IndexPath::default()), window, cx)
                    },
                    window,
                    cx,
                )
                .expect("active API Key location must bind");
                AuthFields::ApiKey {
                    name_path,
                    name,
                    value_path,
                    value,
                    location_path,
                    location,
                }
            }
        }
    }

    fn error<T: Clone + PartialEq + 'static>(
        &self,
        path: &DynamicPath<RequestDraft, T>,
        cx: &Context<Self>,
    ) -> Option<SharedString> {
        path.try_errors(&self.form, cx)
            .ok()?
            .first()
            .map(|issue| validation_message(issue.message(), cx))
    }

    fn error_matching<T: Clone + PartialEq + 'static>(
        &self,
        path: &DynamicPath<RequestDraft, T>,
        include: impl Fn(&gpui_form::ValidationIssue) -> bool,
        cx: &Context<Self>,
    ) -> Option<SharedString> {
        path.try_errors(&self.form, cx)
            .ok()?
            .iter()
            .find(|issue| include(issue))
            .map(|issue| validation_message(issue.message(), cx))
    }

    fn control_with_error(
        &self,
        control: impl IntoElement,
        error: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .gap_1()
            .child(control)
            .when_some(error, |this, error| {
                this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
            })
            .into_any_element()
    }
}

fn auth_options(cx: &gpui::App) -> Vec<AuthOption> {
    let i18n = cx.global::<I18n>();
    [
        (AuthKind::None, "auth-none"),
        (AuthKind::Basic, "auth-basic"),
        (AuthKind::Bearer, "auth-bearer"),
        (AuthKind::ApiKey, "auth-api-key"),
    ]
    .into_iter()
    .map(|(kind, key)| AuthOption {
        kind,
        title: i18n.t(key).into(),
    })
    .collect()
}

fn api_key_location_options(cx: &gpui::App) -> Vec<ApiKeyLocationOption> {
    let i18n = cx.global::<I18n>();
    [
        (ApiKeyLocation::Header, "auth-location-header"),
        (ApiKeyLocation::Query, "auth-location-query"),
    ]
    .into_iter()
    .map(|(location, key)| ApiKeyLocationOption {
        location,
        title: i18n.t(key).into(),
    })
    .collect()
}

impl gpui::Render for AuthView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, value, username, password, token, location_label, override_hint) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-name"),
                i18n.t("field-value"),
                i18n.t("field-username"),
                i18n.t("field-password"),
                i18n.t("field-token"),
                i18n.t("field-location"),
                i18n.t("auth-generated-override"),
            )
        };

        let fields = match &self.fields {
            AuthFields::None => v_form().into_any_element(),
            AuthFields::Basic {
                username_path,
                username: username_input,
                password_path,
                password: password_input,
            } => v_form()
                .child(field().label(username).child(self.control_with_error(
                    Input::new(username_input),
                    self.error(username_path, cx),
                    cx,
                )))
                .child(
                    field().label(password).child(
                        self.control_with_error(
                            Input::new(password_input)
                                .content_type(InputContentType::Password)
                                .mask_toggle(),
                            self.error(password_path, cx),
                            cx,
                        ),
                    ),
                )
                .into_any_element(),
            AuthFields::Bearer {
                token_path,
                token: token_input,
            } => v_form()
                .child(
                    field().label(token).child(
                        self.control_with_error(
                            Input::new(token_input)
                                .content_type(InputContentType::Password)
                                .mask_toggle(),
                            self.error(token_path, cx),
                            cx,
                        ),
                    ),
                )
                .into_any_element(),
            AuthFields::ApiKey {
                name_path,
                name: name_input,
                value_path,
                value: value_input,
                location_path,
                location,
            } => {
                let location_value = location_path
                    .try_get(&self.form, cx)
                    .unwrap_or(ApiKeyLocation::Header);
                let name_error = self.error_matching(
                    name_path,
                    |issue| api_key_name_issue_visible(location_value, issue.code()),
                    cx,
                );
                let value_error = (location_value == ApiKeyLocation::Header)
                    .then(|| self.error(value_path, cx))
                    .flatten();
                v_form()
                    .child(field().label(name).child(self.control_with_error(
                        Input::new(name_input),
                        name_error,
                        cx,
                    )))
                    .child(
                        field().label(value).child(
                            self.control_with_error(
                                Input::new(value_input)
                                    .content_type(InputContentType::Password)
                                    .mask_toggle(),
                                value_error,
                                cx,
                            ),
                        ),
                    )
                    .child(field().label(location_label).child(location.element()))
                    .into_any_element()
            }
        };

        v_flex()
            .p_3()
            .gap_3()
            .child(h_flex().w(px(260.)).child(self.kind.element()))
            .child(fields)
            .when(self.kind.current_kind() != AuthKind::None, |this| {
                this.child(
                    Label::new(override_hint)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
            })
    }
}

fn api_key_name_issue_visible(location: ApiKeyLocation, code: &str) -> bool {
    location == ApiKeyLocation::Header || code == "request-api-key-name-required"
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};

    use super::*;
    use crate::foundation::i18n::init_i18n;

    #[test]
    fn api_key_location_hides_header_only_submit_issues() {
        assert!(api_key_name_issue_visible(
            ApiKeyLocation::Header,
            "request-api-key-name-invalid"
        ));
        assert!(!api_key_name_issue_visible(
            ApiKeyLocation::Query,
            "request-api-key-name-invalid"
        ));
        assert!(api_key_name_issue_visible(
            ApiKeyLocation::Query,
            "request-api-key-name-required"
        ));
    }

    #[gpui::test]
    fn auth_leaf_changes_keep_native_controls_and_case_changes_rebuild_them(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            let draft = RequestDraft {
                auth: RequestAuthDraft::basic(),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            AuthView::new(form, window, cx)
        });

        let (form, username_path, username_control) = cx.update(|_, cx| {
            let view = view.read(cx);
            let AuthFields::Basic {
                username_path,
                username,
                ..
            } = &view.fields
            else {
                panic!("Basic controls must be active");
            };
            (
                view.form.clone(),
                username_path.clone(),
                std::ops::Deref::deref(username).clone(),
            )
        });

        cx.update(|_, cx| {
            username_path
                .try_set(&form, "alice".to_owned(), cx)
                .expect("active Basic username must remain writable");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let view = view.read(cx);
            let AuthFields::Basic { username, .. } = &view.fields else {
                panic!("leaf changes must not change the active auth case");
            };
            assert_eq!(
                std::ops::Deref::deref(username).entity_id(),
                username_control.entity_id(),
                "a leaf projection must not rebuild the native editor",
            );
            assert_eq!(username_control.read(cx).value(), "alice");
        });

        cx.update(|_, cx| {
            RequestDraft::AUTH.set(&form, RequestAuthDraft::bearer(), cx);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(matches!(view.read(cx).fields, AuthFields::Bearer { .. }));
        });
    }
}
