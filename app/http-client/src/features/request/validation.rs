use gpui_form::{
    ValidationMessage, ValidationRequest, ValidationSink, ValidationTrigger, Validator,
};
use http::HeaderValue;

use super::{
    draft::{
        ApiKeyAuthDraft, ApiKeyLocation, BasicAuthDraft, BearerAuthDraft, BinaryBodyDraft,
        FormDataDraft, HeaderDraft, MultipartFileDraft, MultipartPartDraft,
        MultipartPartValueDraft, MultipartTextDraft, RequestAuthDraft, RequestBodyDraft,
        RequestDraft,
    },
    prepared::{
        RequestFieldError, inspect_request_file, parse_header_name, parse_header_value,
        parse_media_type, parse_request_url, validate_api_key_name, validate_basic_username,
        validate_disposition_text,
    },
};

pub(crate) struct RequestValidator;

impl Validator<RequestDraft> for RequestValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, RequestDraft>,
        out: &mut ValidationSink<'_, RequestDraft>,
    ) {
        if request.trigger() != ValidationTrigger::Submit {
            return;
        }

        validate_url(&request, out);
        validate_headers(&request, out);
        validate_body(&request, out);
        validate_auth(&request, out);
    }
}

fn validate_url(
    request: &ValidationRequest<'_, RequestDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    if !request.includes(&RequestDraft::URL) || request.model().url.trim().is_empty() {
        return;
    }

    if let Err(error) = parse_request_url(&request.model().url) {
        let (code, key) = match error {
            RequestFieldError::UnsupportedUrlScheme => {
                ("request-url-scheme-invalid", "request-url-scheme-invalid")
            }
            _ => ("request-url-invalid", "request-url-invalid"),
        };
        out.at(RequestDraft::URL)
            .error(code, ValidationMessage::key(key));
    }
}

fn validate_headers(
    request: &ValidationRequest<'_, RequestDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let headers = RequestDraft::ROOT.then(RequestDraft::HEADERS);
    for header in request.items(&headers) {
        let enabled = header.clone().then(HeaderDraft::ENABLED);
        if !request.try_get(&enabled).is_ok_and(|enabled| *enabled) {
            continue;
        }

        let name = header.clone().then(HeaderDraft::NAME);
        if request.includes(&name)
            && request
                .try_get(&name)
                .is_ok_and(|value| parse_header_name(value).is_err())
        {
            out.at(name).error(
                "request-header-name-invalid",
                ValidationMessage::key("request-header-name-invalid"),
            );
        }

        let value = header.then(HeaderDraft::VALUE);
        if request.includes(&value)
            && request
                .try_get(&value)
                .is_ok_and(|value| parse_header_value(value).is_err())
        {
            out.at(value).error(
                "request-header-value-invalid",
                ValidationMessage::key("request-header-value-invalid"),
            );
        }
    }
}

fn validate_body(
    request: &ValidationRequest<'_, RequestDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let body = RequestDraft::ROOT.then(RequestDraft::BODY);
    match request.get(&body) {
        RequestBodyDraft::None => {}
        RequestBodyDraft::Text(_) => {}
        RequestBodyDraft::UrlEncoded(_) => {
            // Empty keys and values are valid; disabled rows are omitted by the compiler.
            let _ = request.case(body, RequestBodyDraft::URL_ENCODED);
        }
        RequestBodyDraft::FormData(_) => {
            let Some(form_data) = request.case(body, RequestBodyDraft::FORM_DATA) else {
                return;
            };
            validate_multipart(request, form_data, out);
        }
        RequestBodyDraft::Binary(_) => {
            let Some(binary) = request.case(body, RequestBodyDraft::BINARY) else {
                return;
            };
            let file = binary.then(BinaryBodyDraft::FILE);
            if !request.includes(&file) {
                return;
            }
            match request.try_get(&file) {
                Ok(None) => out.at(file).error(
                    "request-file-required",
                    ValidationMessage::key("request-file-required"),
                ),
                Ok(Some(path)) if inspect_request_file(path).is_err() => out.at(file).error(
                    "request-file-unavailable",
                    ValidationMessage::key("request-file-unavailable"),
                ),
                _ => {}
            }
        }
    }
}

fn validate_multipart<'a>(
    request: &ValidationRequest<'a, RequestDraft>,
    form_data: gpui_form::ValidationDynamicPath<'a, RequestDraft, FormDataDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let parts = form_data.then(FormDataDraft::PARTS);
    let Ok(parts) = request.try_items(&parts) else {
        return;
    };
    for part in parts {
        let enabled = part.clone().then(MultipartPartDraft::ENABLED);
        if !request.try_get(&enabled).is_ok_and(|enabled| *enabled) {
            continue;
        }

        let name = part.clone().then(MultipartPartDraft::NAME);
        if request.includes(&name)
            && let Ok(value) = request.try_get(&name)
        {
            if value.trim().is_empty() {
                out.at(name).error(
                    "request-multipart-name-required",
                    ValidationMessage::key("request-multipart-name-required"),
                );
            } else if validate_disposition_text(value).is_err() {
                out.at(name).error(
                    "request-multipart-name-invalid",
                    ValidationMessage::key("request-multipart-name-invalid"),
                );
            }
        }

        let value = part.then(MultipartPartDraft::VALUE);
        let Ok(active_value) = request.try_get(&value) else {
            continue;
        };
        match active_value {
            MultipartPartValueDraft::Text(_) => {
                let Ok(Some(text)) = value.case(MultipartPartValueDraft::TEXT).resolve(request)
                else {
                    continue;
                };
                validate_optional_media_type(
                    request,
                    text.then(MultipartTextDraft::CONTENT_TYPE),
                    out,
                );
            }
            MultipartPartValueDraft::File(_) => {
                let Ok(Some(file)) = value.case(MultipartPartValueDraft::FILE).resolve(request)
                else {
                    continue;
                };
                validate_multipart_file(request, file, out);
            }
        }
    }
}

fn validate_multipart_file<'a>(
    request: &ValidationRequest<'a, RequestDraft>,
    file: gpui_form::ValidationDynamicPath<'a, RequestDraft, MultipartFileDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let path = file.clone().then(MultipartFileDraft::PATH);
    let selected_path = request.try_get(&path).ok().and_then(Option::as_ref);
    if request.includes(&path) {
        match selected_path.map(|path| inspect_request_file(path)) {
            None => out.at(path).error(
                "request-file-required",
                ValidationMessage::key("request-file-required"),
            ),
            Some(Err(_)) => out.at(path).error(
                "request-file-unavailable",
                ValidationMessage::key("request-file-unavailable"),
            ),
            Some(Ok(file)) if validate_disposition_text(&file.file_name).is_err() => {
                out.at(path).error(
                    "request-file-name-invalid",
                    ValidationMessage::key("request-file-name-invalid"),
                );
            }
            Some(Ok(_)) => {}
        }
    }
}

fn validate_optional_media_type<'a>(
    request: &ValidationRequest<'a, RequestDraft>,
    path: gpui_form::ValidationDynamicPath<'a, RequestDraft, Option<String>>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    if !request.includes(&path) {
        return;
    }
    if request.try_get(&path).is_ok_and(|value| {
        value
            .as_deref()
            .is_some_and(|value| parse_media_type(value).is_err())
    }) {
        out.at(path).error(
            "request-media-type-invalid",
            ValidationMessage::key("request-media-type-invalid"),
        );
    }
}

fn validate_auth(
    request: &ValidationRequest<'_, RequestDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let auth = RequestDraft::ROOT.then(RequestDraft::AUTH);
    match request.get(&auth) {
        RequestAuthDraft::None => {}
        RequestAuthDraft::Basic(_) => {
            let Some(basic) = request.case(auth, RequestAuthDraft::BASIC) else {
                return;
            };
            let username = basic.clone().then(BasicAuthDraft::USERNAME);
            if request.includes(&username)
                && request
                    .try_get(&username)
                    .is_ok_and(|value| validate_basic_username(value).is_err())
            {
                out.at(username).error(
                    "request-basic-username-colon",
                    ValidationMessage::key("request-basic-username-colon"),
                );
            }
        }
        RequestAuthDraft::Bearer(_) => {
            let Some(bearer) = request.case(auth, RequestAuthDraft::BEARER) else {
                return;
            };
            let token = bearer.then(BearerAuthDraft::TOKEN);
            if request.includes(&token)
                && request
                    .try_get(&token)
                    .is_ok_and(|token| HeaderValue::from_str(&format!("Bearer {token}")).is_err())
            {
                out.at(token).error(
                    "request-auth-value-invalid",
                    ValidationMessage::key("request-auth-value-invalid"),
                );
            }
        }
        RequestAuthDraft::ApiKey(_) => {
            let Some(api_key) = request.case(auth, RequestAuthDraft::API_KEY) else {
                return;
            };
            validate_api_key(request, api_key, out);
        }
    }
}

fn validate_api_key<'a>(
    request: &ValidationRequest<'a, RequestDraft>,
    api_key: gpui_form::ValidationDynamicPath<'a, RequestDraft, ApiKeyAuthDraft>,
    out: &mut ValidationSink<'_, RequestDraft>,
) {
    let location = api_key.clone().then(ApiKeyAuthDraft::LOCATION);
    let Ok(location_value) = request.try_get(&location) else {
        return;
    };

    let name = api_key.clone().then(ApiKeyAuthDraft::NAME);
    if request.includes(&name)
        && let Ok(value) = request.try_get(&name)
        && let Err(error) = validate_api_key_name(value, *location_value)
    {
        let (code, key) = match error {
            RequestFieldError::ApiKeyNameRequired => (
                "request-api-key-name-required",
                "request-api-key-name-required",
            ),
            _ => (
                "request-api-key-name-invalid",
                "request-api-key-name-invalid",
            ),
        };
        out.at(name).error(code, ValidationMessage::key(key));
    }

    if *location_value == ApiKeyLocation::Header {
        let value = api_key.then(ApiKeyAuthDraft::VALUE);
        if request.includes(&value)
            && request
                .try_get(&value)
                .is_ok_and(|value| parse_header_value(value).is_err())
        {
            out.at(value).error(
                "request-auth-value-invalid",
                ValidationMessage::key("request-auth-value-invalid"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};
    use gpui_form::{Form, PrepareError, ValidationTrigger};

    use super::*;
    use crate::features::request::draft::{
        ApiKeyAuthDraft, BinaryBodyDraft, FormDataDraft, MultipartFileDraft, MultipartPartDraft,
        MultipartPartValueDraft, RequestSettingsDraft,
    };
    use crate::features::request::prepared::{
        FileCheckError, RequestCompileError, RequestFileField, compile_request,
    };

    fn form(draft: RequestDraft, cx: &mut gpui::App) -> gpui::Entity<Form<RequestDraft>> {
        cx.new(|_| Form::new(draft).with_validator(RequestValidator))
    }

    fn valid_draft() -> RequestDraft {
        RequestDraft {
            method: super::super::method::HttpMethod::Get,
            url: "https://example.test".into(),
            headers: Vec::new(),
            body: RequestBodyDraft::None,
            auth: RequestAuthDraft::None,
            settings: RequestSettingsDraft::default(),
        }
    }

    #[gpui::test]
    fn business_validation_runs_only_on_submit(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(valid_draft(), cx);

            assert!(RequestDraft::URL.errors(&form, cx).is_empty());
            RequestDraft::URL.set(&form, "file:///tmp/request".into(), cx);
            assert!(RequestDraft::URL.errors(&form, cx).is_empty());
            RequestDraft::URL.validate(&form, ValidationTrigger::Blur, cx);
            assert!(RequestDraft::URL.errors(&form, cx).is_empty());
            assert!(matches!(
                form.update(cx, |form, cx| form.prepare(cx)),
                Err(PrepareError::Validation(_))
            ));
            assert_eq!(RequestDraft::URL.errors(&form, cx).len(), 1);
            assert_eq!(
                RequestDraft::URL.errors(&form, cx)[0].code(),
                "request-url-scheme-invalid"
            );
        });
    }

    #[gpui::test]
    fn blank_url_uses_only_the_schema_required_issue(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut draft = valid_draft();
            draft.url = "   ".into();
            let form = form(draft, cx);
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());

            let issues = RequestDraft::URL.errors(&form, cx);
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].code(), "required");
        });
    }

    #[gpui::test]
    fn disabled_headers_are_skipped_and_enabled_errors_are_precise(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut draft = valid_draft();
            draft.headers = vec![
                HeaderDraft {
                    enabled: false,
                    name: "bad header".into(),
                    value: "bad\nvalue".into(),
                },
                HeaderDraft {
                    enabled: true,
                    name: "bad header".into(),
                    value: "ok".into(),
                },
                HeaderDraft {
                    enabled: true,
                    name: "x-valid".into(),
                    value: "bad\nvalue".into(),
                },
            ];
            let form = form(draft, cx);
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());

            let rows = RequestDraft::HEADERS.items(&form, cx);
            assert!(
                rows[0]
                    .clone()
                    .then(HeaderDraft::NAME)
                    .try_errors(&form, cx)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                rows[1]
                    .clone()
                    .then(HeaderDraft::NAME)
                    .try_errors(&form, cx)
                    .unwrap()[0]
                    .code(),
                "request-header-name-invalid"
            );
            assert_eq!(
                rows[2]
                    .clone()
                    .then(HeaderDraft::VALUE)
                    .try_errors(&form, cx)
                    .unwrap()[0]
                    .code(),
                "request-header-value-invalid"
            );
        });
    }

    #[gpui::test]
    fn inactive_cases_do_not_publish_file_or_auth_issues(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let valid_form = form(valid_draft(), cx);
            assert!(valid_form.update(cx, |form, cx| form.prepare(cx)).is_ok());

            let mut draft = valid_draft();
            draft.body = RequestBodyDraft::Binary(BinaryBodyDraft { file: None });
            draft.auth = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                name: String::new(),
                value: String::new(),
                location: ApiKeyLocation::Header,
            });
            let form = form(draft, cx);
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        });
    }

    #[gpui::test]
    fn multipart_issues_are_attached_to_active_dynamic_fields(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let directory = tempfile::tempdir().unwrap();
            #[cfg(unix)]
            let file_path = directory.path().join("unsafe\nname.bin");
            #[cfg(not(unix))]
            let file_path = directory.path().join("safe-name.bin");
            std::fs::write(&file_path, b"payload").unwrap();
            let mut draft = valid_draft();
            draft.body = RequestBodyDraft::FormData(FormDataDraft {
                parts: vec![
                    MultipartPartDraft {
                        enabled: true,
                        name: String::new(),
                        value: MultipartPartValueDraft::File(MultipartFileDraft::default()),
                    },
                    MultipartPartDraft {
                        enabled: true,
                        name: "file".into(),
                        value: MultipartPartValueDraft::File(MultipartFileDraft {
                            path: Some(file_path),
                        }),
                    },
                    MultipartPartDraft {
                        enabled: true,
                        name: "text".into(),
                        value: MultipartPartValueDraft::Text(MultipartTextDraft {
                            value: String::new(),
                            content_type: Some("also invalid".into()),
                        }),
                    },
                ],
            });
            let form = form(draft, cx);
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());

            let body = RequestDraft::BODY.case(RequestBodyDraft::FORM_DATA);
            let form_data = body.resolve(&form, cx).unwrap().unwrap();
            let parts = form_data
                .then(FormDataDraft::PARTS)
                .try_items(&form, cx)
                .unwrap();
            assert_eq!(
                parts[0]
                    .clone()
                    .then(MultipartPartDraft::NAME)
                    .try_errors(&form, cx)
                    .unwrap()[0]
                    .code(),
                "request-multipart-name-required"
            );
            let value = parts[0].clone().then(MultipartPartDraft::VALUE);
            let file = value
                .case(MultipartPartValueDraft::FILE)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                file.then(MultipartFileDraft::PATH)
                    .try_errors(&form, cx)
                    .unwrap()[0]
                    .code(),
                "request-file-required"
            );

            let second_value = parts[1].clone().then(MultipartPartDraft::VALUE);
            let second_file = second_value
                .case(MultipartPartValueDraft::FILE)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            let second_file_errors = second_file
                .then(MultipartFileDraft::PATH)
                .try_errors(&form, cx)
                .unwrap();
            #[cfg(unix)]
            assert_eq!(second_file_errors[0].code(), "request-file-name-invalid");
            #[cfg(not(unix))]
            assert!(second_file_errors.is_empty());

            let text_value = parts[2].clone().then(MultipartPartDraft::VALUE);
            let text = text_value
                .case(MultipartPartValueDraft::TEXT)
                .resolve(&form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                text.then(MultipartTextDraft::CONTENT_TYPE)
                    .try_errors(&form, cx)
                    .unwrap()[0]
                    .code(),
                "request-media-type-invalid"
            );
        });
    }

    #[test]
    fn disposition_text_rejects_line_breaks_on_all_platforms() {
        assert_eq!(
            validate_disposition_text("unsafe\nname.bin"),
            Err(RequestFieldError::UnsafeDispositionText)
        );
        assert!(validate_disposition_text("safe-name.bin").is_ok());
    }

    #[gpui::test]
    fn active_binary_and_auth_issues_are_precise(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut binary_draft = valid_draft();
            binary_draft.body = RequestBodyDraft::Binary(BinaryBodyDraft { file: None });
            let binary_form = form(binary_draft, cx);
            assert!(binary_form.update(cx, |form, cx| form.prepare(cx)).is_err());
            let binary = RequestDraft::BODY
                .case(RequestBodyDraft::BINARY)
                .resolve(&binary_form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                binary
                    .then(BinaryBodyDraft::FILE)
                    .try_errors(&binary_form, cx)
                    .unwrap()[0]
                    .code(),
                "request-file-required"
            );

            let mut basic_draft = valid_draft();
            basic_draft.auth = RequestAuthDraft::Basic(BasicAuthDraft {
                username: "user:name".into(),
                password: String::new(),
            });
            let basic_form = form(basic_draft, cx);
            assert!(basic_form.update(cx, |form, cx| form.prepare(cx)).is_err());
            let basic = RequestDraft::AUTH
                .case(RequestAuthDraft::BASIC)
                .resolve(&basic_form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                basic
                    .then(BasicAuthDraft::USERNAME)
                    .try_errors(&basic_form, cx)
                    .unwrap()[0]
                    .code(),
                "request-basic-username-colon"
            );

            let mut bearer_draft = valid_draft();
            bearer_draft.auth = RequestAuthDraft::Bearer(BearerAuthDraft {
                token: "bad\ntoken".into(),
            });
            let bearer_form = form(bearer_draft, cx);
            assert!(bearer_form.update(cx, |form, cx| form.prepare(cx)).is_err());
            let bearer = RequestDraft::AUTH
                .case(RequestAuthDraft::BEARER)
                .resolve(&bearer_form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                bearer
                    .then(BearerAuthDraft::TOKEN)
                    .try_errors(&bearer_form, cx)
                    .unwrap()[0]
                    .code(),
                "request-auth-value-invalid"
            );

            let mut api_key_draft = valid_draft();
            api_key_draft.auth = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                name: "x-api-key".into(),
                value: "bad\nvalue".into(),
                location: ApiKeyLocation::Header,
            });
            let api_key_form = form(api_key_draft, cx);
            assert!(
                api_key_form
                    .update(cx, |form, cx| form.prepare(cx))
                    .is_err()
            );
            let api_key = RequestDraft::AUTH
                .case(RequestAuthDraft::API_KEY)
                .resolve(&api_key_form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                api_key
                    .then(ApiKeyAuthDraft::VALUE)
                    .try_errors(&api_key_form, cx)
                    .unwrap()[0]
                    .code(),
                "request-auth-value-invalid"
            );

            let mut query_key_draft = valid_draft();
            query_key_draft.auth = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                name: "  ".into(),
                value: String::new(),
                location: ApiKeyLocation::Query,
            });
            let query_key_form = form(query_key_draft, cx);
            assert!(
                query_key_form
                    .update(cx, |form, cx| form.prepare(cx))
                    .is_err()
            );
            let query_key = RequestDraft::AUTH
                .case(RequestAuthDraft::API_KEY)
                .resolve(&query_key_form, cx)
                .unwrap()
                .unwrap();
            assert_eq!(
                query_key
                    .then(ApiKeyAuthDraft::NAME)
                    .try_errors(&query_key_form, cx)
                    .unwrap()[0]
                    .code(),
                "request-api-key-name-required"
            );
        });
    }

    #[gpui::test]
    fn compiler_rechecks_a_file_after_form_validation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("payload.bin");
            std::fs::write(&path, b"payload").unwrap();

            let mut draft = valid_draft();
            draft.body = RequestBodyDraft::Binary(BinaryBodyDraft {
                file: Some(path.clone()),
            });
            let form = form(draft, cx);
            let prepared = form
                .update(cx, |form, cx| form.prepare(cx))
                .expect("the file exists during validation");
            let (_, accepted) = prepared.into_parts();
            let live_before = RequestDraft::ROOT.get(&form, cx);
            let revision_before = form.read(cx).revision();

            std::fs::remove_file(&path).unwrap();
            assert!(matches!(
                compile_request(accepted.clone(), &Default::default()),
                Err(RequestCompileError::FileUnavailable {
                    field: RequestFileField::Binary,
                    reason: FileCheckError::Missing,
                })
            ));
            assert!(RequestDraft::ROOT.get(&form, cx) == live_before);
            assert_eq!(form.read(cx).revision(), revision_before);

            std::fs::create_dir(&path).unwrap();
            let error = compile_request(accepted, &Default::default()).unwrap_err();
            assert_eq!(
                error,
                RequestCompileError::FileUnavailable {
                    field: RequestFileField::Binary,
                    reason: FileCheckError::NotRegular,
                }
            );
            assert!(RequestDraft::ROOT.get(&form, cx) == live_before);
            assert_eq!(form.read(cx).revision(), revision_before);
        });
    }

    #[gpui::test]
    fn auth_compile_does_not_rewrite_form_and_settings_are_frozen(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut draft = valid_draft();
            draft.headers.push(HeaderDraft {
                enabled: true,
                name: "Authorization".into(),
                value: "explicit value".into(),
            });
            draft.auth = RequestAuthDraft::Basic(BasicAuthDraft {
                username: "user".into(),
                password: "secret password".into(),
            });
            draft.settings.follow_original_method = true;
            let form = form(draft, cx);
            let live_before = RequestDraft::ROOT.get(&form, cx);
            let revision_before = form.read(cx).revision();
            let accepted = form
                .update(cx, |form, cx| form.prepare(cx))
                .unwrap()
                .into_parts()
                .1;
            let transport = super::super::draft::HttpClientTransportSettings::default();
            transport.set_timeout_ms(1500);
            let prepared = compile_request(accepted, &transport).unwrap();

            assert!(RequestDraft::ROOT.get(&form, cx) == live_before);
            assert_eq!(form.read(cx).revision(), revision_before);
            assert_eq!(
                RequestDraft::ROOT.get(&form, cx).headers[0].value,
                "explicit value"
            );
            assert_eq!(
                prepared.timeout,
                Some(std::time::Duration::from_millis(1500))
            );
            assert!(prepared.redirect.follow);
            assert!(prepared.redirect.preserve_method);
            assert_eq!(
                prepared.headers.get(http::header::AUTHORIZATION).unwrap(),
                "Basic dXNlcjpzZWNyZXQgcGFzc3dvcmQ="
            );

            transport.set_timeout_ms(9000);
            RequestDraft::ROOT
                .then(RequestDraft::SETTINGS)
                .then(super::super::draft::RequestSettingsDraft::FOLLOW_REDIRECTS)
                .set(&form, false, cx);
            RequestDraft::ROOT
                .then(RequestDraft::SETTINGS)
                .then(super::super::draft::RequestSettingsDraft::FOLLOW_ORIGINAL_METHOD)
                .set(&form, false, cx);

            assert_eq!(
                prepared.timeout,
                Some(std::time::Duration::from_millis(1500))
            );
            assert!(prepared.redirect.follow);
            assert!(prepared.redirect.preserve_method);
        });
    }
}
