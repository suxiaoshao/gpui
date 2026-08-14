use std::{
    fmt,
    fs::File,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{HeaderMap, HeaderName, HeaderValue, header};
use mime::Mime;
use thiserror::Error;
use url::Url;

use super::draft::{
    ApiKeyLocation, HttpClientTransportSettings, MultipartPartValueDraft, RequestAuthDraft,
    RequestBodyDraft, RequestDraft,
};

pub(crate) struct PreparedRequest {
    pub(crate) method: http::Method,
    pub(crate) url: Url,
    pub(crate) headers: HeaderMap,
    pub(crate) body: PreparedBody,
    pub(crate) body_content_type: BodyContentType,
    pub(crate) redirect: PreparedRedirect,
    pub(crate) timeout: Option<Duration>,
}

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedRequest")
            .field("method", &self.method)
            .field("url", &Redacted)
            .field("header_count", &self.headers.len())
            .field("body", &self.body)
            .field("body_content_type", &self.body_content_type)
            .field("redirect", &self.redirect)
            .field("timeout", &self.timeout)
            .finish()
    }
}

pub(crate) enum PreparedBody {
    None,
    Text(Vec<u8>),
    UrlEncoded(Vec<u8>),
    Multipart(Vec<PreparedMultipartPart>),
    Binary(PathBuf),
}

impl fmt::Debug for PreparedBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Text(_) => formatter.write_str("Text(<redacted>)"),
            Self::UrlEncoded(_) => formatter.write_str("UrlEncoded(<redacted>)"),
            Self::Multipart(parts) => formatter
                .debug_tuple("Multipart")
                .field(&RedactedCount(parts.len()))
                .finish(),
            Self::Binary(_) => formatter.write_str("Binary(<redacted>)"),
        }
    }
}

pub(crate) enum PreparedMultipartPart {
    Text {
        name: String,
        value: String,
        content_type: Option<Mime>,
    },
    File {
        name: String,
        path: PathBuf,
        file_name: String,
        content_type: Mime,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum BodyContentType {
    None,
    Fixed(HeaderValue),
    MultipartBoundary,
}

impl fmt::Debug for BodyContentType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Fixed(_) => formatter.write_str("Fixed(<media-type>)"),
            Self::MultipartBoundary => formatter.write_str("MultipartBoundary"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedRedirect {
    pub(crate) follow: bool,
    pub(crate) max_hops: u8,
    pub(crate) preserve_method: bool,
    pub(crate) forward_authorization_cross_host: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestFieldError {
    Required,
    InvalidUrl,
    UnsupportedUrlScheme,
    MissingUrlHost,
    InvalidHeaderName,
    InvalidHeaderValue,
    InvalidMediaType,
    UnsafeDispositionText,
    BasicUsernameContainsColon,
    ApiKeyNameRequired,
    File(FileCheckError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileCheckError {
    NotAbsolute,
    Missing,
    NotRegular,
    Unreadable,
    MissingFileName,
}

pub(crate) struct RequestFile {
    pub(crate) absolute_path: PathBuf,
    pub(crate) file_name: String,
    pub(crate) content_type: Mime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestFileField {
    Binary,
    MultipartPart { draft_index: usize },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum RequestCompileError {
    #[error("request URL is invalid")]
    InvalidUrl,
    #[error("request header is invalid")]
    InvalidHeader,
    #[error("request media type is invalid")]
    InvalidMediaType,
    #[error("request authentication is invalid")]
    InvalidAuth,
    #[error("request file is unavailable ({field:?}: {reason:?})")]
    FileUnavailable {
        field: RequestFileField,
        reason: FileCheckError,
    },
    #[error("request draft violates an internal preparation invariant")]
    UnsupportedInvariant,
}

#[derive(Debug, Error)]
pub(crate) enum RequestPrepareError {
    #[error("request form is invalid")]
    Invalid(#[from] gpui_form::PrepareError),
    #[error("request could not be prepared: {0}")]
    Compile(#[from] RequestCompileError),
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

struct RedactedCount(usize);

impl fmt::Debug for RedactedCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("parts")
            .field("count", &self.0)
            .finish()
    }
}

pub(crate) fn parse_request_url(raw: &str) -> Result<Url, RequestFieldError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(RequestFieldError::Required);
    }

    let url = Url::parse(raw).map_err(|error| match error {
        url::ParseError::EmptyHost => RequestFieldError::MissingUrlHost,
        _ => RequestFieldError::InvalidUrl,
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(RequestFieldError::UnsupportedUrlScheme);
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(RequestFieldError::MissingUrlHost);
    }
    Ok(url)
}

pub(crate) fn parse_header_name(raw: &str) -> Result<HeaderName, RequestFieldError> {
    HeaderName::from_bytes(raw.as_bytes()).map_err(|_| RequestFieldError::InvalidHeaderName)
}

pub(crate) fn parse_header_value(raw: &str) -> Result<HeaderValue, RequestFieldError> {
    HeaderValue::from_str(raw).map_err(|_| RequestFieldError::InvalidHeaderValue)
}

pub(crate) fn parse_media_type(raw: &str) -> Result<Mime, RequestFieldError> {
    Mime::from_str(raw).map_err(|_| RequestFieldError::InvalidMediaType)
}

pub(crate) fn inspect_request_file(path: &Path) -> Result<RequestFile, RequestFieldError> {
    if !path.is_absolute() {
        return Err(RequestFieldError::File(FileCheckError::NotAbsolute));
    }

    let metadata = std::fs::metadata(path).map_err(|error| {
        RequestFieldError::File(match error.kind() {
            std::io::ErrorKind::NotFound => FileCheckError::Missing,
            _ => FileCheckError::Unreadable,
        })
    })?;
    if !metadata.is_file() {
        return Err(RequestFieldError::File(FileCheckError::NotRegular));
    }
    File::open(path)
        .map(drop)
        .map_err(|_| RequestFieldError::File(FileCheckError::Unreadable))?;

    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .ok_or(RequestFieldError::File(FileCheckError::MissingFileName))?;

    Ok(RequestFile {
        absolute_path: path.to_path_buf(),
        file_name,
        content_type: mime_guess::from_path(path).first_or_octet_stream(),
    })
}

pub(crate) fn validate_disposition_text(raw: &str) -> Result<(), RequestFieldError> {
    if raw.contains(['\r', '\n', '\0']) {
        Err(RequestFieldError::UnsafeDispositionText)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_basic_username(raw: &str) -> Result<(), RequestFieldError> {
    if raw.contains(':') {
        Err(RequestFieldError::BasicUsernameContainsColon)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_api_key_name(
    raw: &str,
    location: ApiKeyLocation,
) -> Result<(), RequestFieldError> {
    if raw.trim().is_empty() {
        return Err(RequestFieldError::ApiKeyNameRequired);
    }
    if location == ApiKeyLocation::Header {
        parse_header_name(raw)?;
    }
    Ok(())
}

pub(crate) fn compile_request(
    draft: RequestDraft,
    settings: &HttpClientTransportSettings,
) -> Result<PreparedRequest, RequestCompileError> {
    let RequestDraft {
        method,
        url,
        headers: draft_headers,
        body: draft_body,
        auth,
        settings: request_settings,
    } = draft;

    let mut url = parse_request_url(&url).map_err(|_| RequestCompileError::InvalidUrl)?;
    url.set_fragment(None);

    let mut headers = HeaderMap::new();
    for header in draft_headers.into_iter().filter(|header| header.enabled) {
        let name =
            parse_header_name(&header.name).map_err(|_| RequestCompileError::InvalidHeader)?;
        let value =
            parse_header_value(&header.value).map_err(|_| RequestCompileError::InvalidHeader)?;
        headers.append(name, value);
    }

    let (body, body_content_type) = compile_body(draft_body)?;
    apply_auth(auth, &mut url, &mut headers)?;

    Ok(PreparedRequest {
        method: method.to_http_method(),
        url,
        headers,
        body,
        body_content_type,
        redirect: PreparedRedirect {
            follow: request_settings.follow_redirects,
            max_hops: 10,
            preserve_method: request_settings.follow_original_method,
            forward_authorization_cross_host: false,
        },
        timeout: (settings.timeout_ms() != 0).then(|| Duration::from_millis(settings.timeout_ms())),
    })
}

fn compile_body(
    body: RequestBodyDraft,
) -> Result<(PreparedBody, BodyContentType), RequestCompileError> {
    match body {
        RequestBodyDraft::None => Ok((PreparedBody::None, BodyContentType::None)),
        RequestBodyDraft::Text(text) => Ok((
            PreparedBody::Text(text.content.into_bytes()),
            BodyContentType::Fixed(HeaderValue::from_static(text.format.media_type())),
        )),
        RequestBodyDraft::UrlEncoded(url_encoded) => {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for field in url_encoded.fields.into_iter().filter(|field| field.enabled) {
                serializer.append_pair(&field.key, &field.value);
            }
            Ok((
                PreparedBody::UrlEncoded(serializer.finish().into_bytes()),
                BodyContentType::Fixed(HeaderValue::from_static(
                    "application/x-www-form-urlencoded",
                )),
            ))
        }
        RequestBodyDraft::FormData(form_data) => {
            let mut prepared_parts = Vec::new();
            for (draft_index, part) in form_data.parts.into_iter().enumerate() {
                if !part.enabled {
                    continue;
                }
                if part.name.trim().is_empty() || validate_disposition_text(&part.name).is_err() {
                    return Err(RequestCompileError::UnsupportedInvariant);
                }
                match part.value {
                    MultipartPartValueDraft::Text(text) => {
                        let content_type = text
                            .content_type
                            .as_deref()
                            .map(parse_media_type)
                            .transpose()
                            .map_err(|_| RequestCompileError::InvalidMediaType)?;
                        prepared_parts.push(PreparedMultipartPart::Text {
                            name: part.name,
                            value: text.value,
                            content_type,
                        });
                    }
                    MultipartPartValueDraft::File(file) => {
                        let path = file.path.ok_or(RequestCompileError::FileUnavailable {
                            field: RequestFileField::MultipartPart { draft_index },
                            reason: FileCheckError::Missing,
                        })?;
                        let inspected = inspect_request_file(&path).map_err(|error| {
                            file_compile_error(
                                RequestFileField::MultipartPart { draft_index },
                                error,
                            )
                        })?;
                        if validate_disposition_text(&inspected.file_name).is_err() {
                            return Err(RequestCompileError::FileUnavailable {
                                field: RequestFileField::MultipartPart { draft_index },
                                reason: FileCheckError::MissingFileName,
                            });
                        }
                        prepared_parts.push(PreparedMultipartPart::File {
                            name: part.name,
                            path: inspected.absolute_path,
                            file_name: inspected.file_name,
                            content_type: inspected.content_type,
                        });
                    }
                }
            }
            Ok((
                PreparedBody::Multipart(prepared_parts),
                BodyContentType::MultipartBoundary,
            ))
        }
        RequestBodyDraft::Binary(binary) => {
            let path = binary.file.ok_or(RequestCompileError::FileUnavailable {
                field: RequestFileField::Binary,
                reason: FileCheckError::Missing,
            })?;
            let inspected = inspect_request_file(&path)
                .map_err(|error| file_compile_error(RequestFileField::Binary, error))?;
            Ok((
                PreparedBody::Binary(inspected.absolute_path),
                BodyContentType::None,
            ))
        }
    }
}

fn apply_auth(
    auth: RequestAuthDraft,
    url: &mut Url,
    headers: &mut HeaderMap,
) -> Result<(), RequestCompileError> {
    match auth {
        RequestAuthDraft::None => Ok(()),
        RequestAuthDraft::Basic(basic) => {
            validate_basic_username(&basic.username)
                .map_err(|_| RequestCompileError::InvalidAuth)?;
            let encoded = STANDARD.encode(format!("{}:{}", basic.username, basic.password));
            let value = HeaderValue::from_str(&format!("Basic {encoded}"))
                .map_err(|_| RequestCompileError::InvalidAuth)?;
            headers.remove(header::AUTHORIZATION);
            headers.append(header::AUTHORIZATION, value);
            Ok(())
        }
        RequestAuthDraft::Bearer(bearer) => {
            let value = HeaderValue::from_str(&format!("Bearer {}", bearer.token))
                .map_err(|_| RequestCompileError::InvalidAuth)?;
            headers.remove(header::AUTHORIZATION);
            headers.append(header::AUTHORIZATION, value);
            Ok(())
        }
        RequestAuthDraft::ApiKey(api_key) => match api_key.location {
            ApiKeyLocation::Header => {
                validate_api_key_name(&api_key.name, api_key.location)
                    .map_err(|_| RequestCompileError::InvalidAuth)?;
                let name = parse_header_name(&api_key.name)
                    .map_err(|_| RequestCompileError::InvalidAuth)?;
                let value = parse_header_value(&api_key.value)
                    .map_err(|_| RequestCompileError::InvalidAuth)?;
                headers.remove(&name);
                headers.append(name, value);
                Ok(())
            }
            ApiKeyLocation::Query => {
                validate_api_key_name(&api_key.name, api_key.location)
                    .map_err(|_| RequestCompileError::InvalidAuth)?;
                let retained = url
                    .query_pairs()
                    .filter(|(name, _)| name.as_ref() != api_key.name)
                    .map(|(name, value)| (name.into_owned(), value.into_owned()))
                    .collect::<Vec<_>>();
                url.query_pairs_mut()
                    .clear()
                    .extend_pairs(retained)
                    .append_pair(&api_key.name, &api_key.value);
                Ok(())
            }
        },
    }
}

fn file_compile_error(field: RequestFileField, error: RequestFieldError) -> RequestCompileError {
    match error {
        RequestFieldError::File(reason) => RequestCompileError::FileUnavailable { field, reason },
        _ => RequestCompileError::UnsupportedInvariant,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;
    use crate::features::request::draft::{
        ApiKeyAuthDraft, BasicAuthDraft, BearerAuthDraft, BinaryBodyDraft, FormDataDraft,
        HeaderDraft, KeyValueDraft, MultipartFileDraft, MultipartPartDraft, MultipartTextDraft,
        RequestSettingsDraft, TextBodyDraft, TextBodyFormat, UrlEncodedBodyDraft,
    };
    use crate::features::request::method::HttpMethod;

    fn draft() -> RequestDraft {
        RequestDraft {
            method: HttpMethod::Post,
            url: " https://example.test/path?a=1#fragment ".into(),
            headers: Vec::new(),
            body: RequestBodyDraft::None,
            auth: RequestAuthDraft::None,
            settings: RequestSettingsDraft::default(),
        }
    }

    #[test]
    fn url_parser_requires_absolute_http_or_https_with_a_host() {
        assert_eq!(parse_request_url("  "), Err(RequestFieldError::Required));
        assert_eq!(
            parse_request_url("relative/path"),
            Err(RequestFieldError::InvalidUrl)
        );
        assert_eq!(
            parse_request_url("file:///tmp/request"),
            Err(RequestFieldError::UnsupportedUrlScheme)
        );
        assert_eq!(
            parse_request_url("http://"),
            Err(RequestFieldError::MissingUrlHost)
        );
        assert!(parse_request_url("http://example.test/path").is_ok());
        assert!(parse_request_url("https://example.test/path").is_ok());
    }

    #[test]
    fn compiler_trims_url_removes_fragment_and_freezes_settings() {
        let mut request = draft();
        request.settings.follow_original_method = true;
        let settings = HttpClientTransportSettings::default();
        settings.set_timeout_ms(2500);
        let prepared = compile_request(request, &settings).unwrap();

        assert_eq!(prepared.method, http::Method::POST);
        assert_eq!(prepared.url.as_str(), "https://example.test/path?a=1");
        assert!(matches!(prepared.body, PreparedBody::None));
        assert_eq!(prepared.body_content_type, BodyContentType::None);
        assert_eq!(prepared.timeout, Some(Duration::from_millis(2500)));
        assert_eq!(
            prepared.redirect,
            PreparedRedirect {
                follow: true,
                max_hops: 10,
                preserve_method: true,
                forward_authorization_cross_host: false,
            }
        );

        let unlimited = compile_request(draft(), &HttpClientTransportSettings::default()).unwrap();
        assert_eq!(unlimited.timeout, None);

        settings.set_timeout_ms(5000);
        assert_eq!(prepared.timeout, Some(Duration::from_millis(2500)));
        assert!(prepared.redirect.follow);
        assert!(prepared.redirect.preserve_method);
    }

    #[test]
    fn headers_keep_duplicate_values_and_auth_replaces_conflicts() {
        let mut request = draft();
        request.headers = vec![
            HeaderDraft {
                enabled: true,
                name: "x-test".into(),
                value: "one".into(),
            },
            HeaderDraft {
                enabled: true,
                name: "X-Test".into(),
                value: "two".into(),
            },
            HeaderDraft {
                enabled: true,
                name: "Authorization".into(),
                value: "explicit".into(),
            },
            HeaderDraft {
                enabled: true,
                name: "x-empty".into(),
                value: String::new(),
            },
        ];
        request.auth = RequestAuthDraft::Basic(BasicAuthDraft {
            username: "user".into(),
            password: "password".into(),
        });

        let prepared = compile_request(request, &Default::default()).unwrap();
        assert_eq!(
            prepared
                .headers
                .get_all("x-test")
                .iter()
                .map(|value| value.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(
            prepared.headers.get(header::AUTHORIZATION).unwrap(),
            "Basic dXNlcjpwYXNzd29yZA=="
        );
        assert_eq!(prepared.headers.get("x-empty").unwrap(), "");
    }

    #[test]
    fn bearer_and_header_api_key_replace_all_explicit_conflicts() {
        let mut bearer = draft();
        bearer.headers = vec![
            HeaderDraft {
                enabled: true,
                name: "authorization".into(),
                value: "first".into(),
            },
            HeaderDraft {
                enabled: true,
                name: "Authorization".into(),
                value: "second".into(),
            },
        ];
        bearer.auth = RequestAuthDraft::Bearer(BearerAuthDraft {
            token: "token".into(),
        });
        let prepared = compile_request(bearer, &Default::default()).unwrap();
        assert_eq!(
            prepared
                .headers
                .get_all(header::AUTHORIZATION)
                .iter()
                .collect::<Vec<_>>(),
            [&HeaderValue::from_static("Bearer token")]
        );

        let mut api_key = draft();
        api_key.headers = vec![
            HeaderDraft {
                enabled: true,
                name: "x-api-key".into(),
                value: "first".into(),
            },
            HeaderDraft {
                enabled: true,
                name: "X-API-KEY".into(),
                value: "second".into(),
            },
        ];
        api_key.auth = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
            name: "X-Api-Key".into(),
            value: "generated".into(),
            location: ApiKeyLocation::Header,
        });
        let prepared = compile_request(api_key, &Default::default()).unwrap();
        assert_eq!(
            prepared
                .headers
                .get_all("x-api-key")
                .iter()
                .collect::<Vec<_>>(),
            [&HeaderValue::from_static("generated")]
        );
    }

    #[test]
    fn body_compiler_preserves_enabled_order_and_content_type_policy() {
        let mut request = draft();
        request.body = RequestBodyDraft::UrlEncoded(UrlEncodedBodyDraft {
            fields: vec![
                KeyValueDraft {
                    enabled: true,
                    key: "same".into(),
                    value: "one".into(),
                },
                KeyValueDraft {
                    enabled: false,
                    key: "skip".into(),
                    value: "ignored".into(),
                },
                KeyValueDraft {
                    enabled: true,
                    key: "same".into(),
                    value: String::new(),
                },
                KeyValueDraft {
                    enabled: true,
                    key: String::new(),
                    value: "empty-key".into(),
                },
            ],
        });
        let prepared = compile_request(request, &Default::default()).unwrap();
        assert!(matches!(
            prepared.body,
            PreparedBody::UrlEncoded(ref bytes) if bytes == b"same=one&same=&=empty-key"
        ));
        assert_eq!(
            prepared.body_content_type,
            BodyContentType::Fixed(HeaderValue::from_static(
                "application/x-www-form-urlencoded"
            ))
        );

        let mut text = draft();
        text.body = RequestBodyDraft::Text(TextBodyDraft {
            format: TextBodyFormat::Json,
            content: "secret body 世界".into(),
        });
        let prepared = compile_request(text, &Default::default()).unwrap();
        assert!(matches!(
            prepared.body,
            PreparedBody::Text(ref bytes) if bytes == "secret body 世界".as_bytes()
        ));

        let mut explicit = draft();
        explicit.headers.push(HeaderDraft {
            enabled: true,
            name: "Content-Type".into(),
            value: "application/custom".into(),
        });
        explicit.body = RequestBodyDraft::Text(TextBodyDraft {
            format: TextBodyFormat::PlainText,
            content: String::new(),
        });
        let prepared = compile_request(explicit, &Default::default()).unwrap();
        assert_eq!(
            prepared.headers.get(header::CONTENT_TYPE).unwrap(),
            "application/custom"
        );
        assert_eq!(
            prepared.body_content_type,
            BodyContentType::Fixed(HeaderValue::from_static("text/plain"))
        );
    }

    #[test]
    fn multipart_and_binary_freeze_paths_without_reading_file_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let file_path = directory.path().join("payload.json");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"payload bytes").unwrap();
        drop(file);

        let mut multipart = draft();
        multipart.body = RequestBodyDraft::FormData(FormDataDraft {
            parts: vec![
                MultipartPartDraft {
                    enabled: true,
                    name: "text".into(),
                    value: MultipartPartValueDraft::Text(MultipartTextDraft {
                        value: "value".into(),
                        content_type: None,
                    }),
                },
                MultipartPartDraft {
                    enabled: true,
                    name: "file".into(),
                    value: MultipartPartValueDraft::File(MultipartFileDraft {
                        path: Some(file_path.clone()),
                    }),
                },
            ],
        });
        let prepared = compile_request(multipart, &Default::default()).unwrap();
        let PreparedBody::Multipart(parts) = prepared.body else {
            panic!("expected multipart body");
        };
        assert_eq!(parts.len(), 2);
        match &parts[0] {
            PreparedMultipartPart::Text {
                name,
                value,
                content_type,
            } => {
                assert_eq!(name, "text");
                assert_eq!(value, "value");
                assert_eq!(content_type, &None);
            }
            PreparedMultipartPart::File { .. } => panic!("expected text part"),
        }
        match &parts[1] {
            PreparedMultipartPart::File {
                path,
                file_name,
                content_type,
                ..
            } => {
                assert_eq!(path, &file_path);
                assert_eq!(file_name, "payload.json");
                assert_eq!(content_type, &mime::APPLICATION_JSON);
            }
            PreparedMultipartPart::Text { .. } => panic!("expected file part"),
        }

        let mut binary = draft();
        binary.body = RequestBodyDraft::Binary(BinaryBodyDraft {
            file: Some(file_path.clone()),
        });
        let prepared = compile_request(binary, &Default::default()).unwrap();
        assert!(matches!(prepared.body, PreparedBody::Binary(path) if path == file_path));
        assert_eq!(prepared.body_content_type, BodyContentType::None);
    }

    #[test]
    fn multipart_file_with_unknown_extension_uses_octet_stream() {
        let directory = tempfile::tempdir().unwrap();
        let file_path = directory.path().join("payload.unknown-http-client-type");
        std::fs::write(&file_path, b"payload").unwrap();
        let mut request = draft();
        request.body = RequestBodyDraft::FormData(FormDataDraft {
            parts: vec![MultipartPartDraft {
                enabled: true,
                name: "file".into(),
                value: MultipartPartValueDraft::File(MultipartFileDraft {
                    path: Some(file_path),
                }),
            }],
        });

        let prepared = compile_request(request, &Default::default()).unwrap();
        let PreparedBody::Multipart(parts) = prepared.body else {
            panic!("expected multipart body");
        };
        let PreparedMultipartPart::File { content_type, .. } = &parts[0] else {
            panic!("expected file part");
        };
        assert_eq!(content_type, &mime::APPLICATION_OCTET_STREAM);
    }

    #[test]
    fn api_key_query_replaces_matching_decoded_pairs_at_the_end() {
        let mut request = draft();
        request.url = "https://example.test/?keep=1&token=old&keep=2&token=older".into();
        request.auth = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
            name: "token".into(),
            value: "new value".into(),
            location: ApiKeyLocation::Query,
        });
        let prepared = compile_request(request, &Default::default()).unwrap();
        assert_eq!(
            prepared.url.query_pairs().collect::<Vec<_>>(),
            [
                ("keep".into(), "1".into()),
                ("keep".into(), "2".into()),
                ("token".into(), "new value".into())
            ]
        );
    }

    #[test]
    fn compile_rechecks_file_state_and_returns_only_redacted_categories() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("secret-name.bin");
        let mut request = draft();
        request.body = RequestBodyDraft::Binary(BinaryBodyDraft {
            file: Some(missing.clone()),
        });

        let error = compile_request(request, &Default::default()).unwrap_err();
        assert_eq!(
            error,
            RequestCompileError::FileUnavailable {
                field: RequestFileField::Binary,
                reason: FileCheckError::Missing,
            }
        );
        let diagnostic = format!("{error:?} {error}");
        assert!(!diagnostic.contains("secret-name.bin"));
        assert!(!diagnostic.contains(directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn prepared_debug_redacts_url_headers_body_and_paths() {
        let mut request = draft();
        request.url = "https://user:password@example.test/?token=secret".into();
        request.headers.push(HeaderDraft {
            enabled: true,
            name: "x-secret".into(),
            value: "header-secret".into(),
        });
        request.body = RequestBodyDraft::Text(TextBodyDraft {
            format: TextBodyFormat::PlainText,
            content: "body-secret".into(),
        });
        request.auth = RequestAuthDraft::Bearer(BearerAuthDraft {
            token: "bearer-secret".into(),
        });
        let diagnostic = format!(
            "{:?}",
            compile_request(request, &Default::default()).unwrap()
        );

        for secret in [
            "user",
            "password",
            "token=secret",
            "header-secret",
            "body-secret",
            "bearer-secret",
        ] {
            assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
        }
    }

    #[test]
    fn compile_error_diagnostics_do_not_echo_invalid_sensitive_input() {
        let mut invalid_header = draft();
        invalid_header.headers.push(HeaderDraft {
            enabled: true,
            name: "x-secret".into(),
            value: "header-secret\ninvalid".into(),
        });
        let error = compile_request(invalid_header, &Default::default()).unwrap_err();
        let diagnostic = format!("{error:?} {error}");
        assert!(!diagnostic.contains("header-secret"));

        let mut invalid_auth = draft();
        invalid_auth.auth = RequestAuthDraft::Bearer(BearerAuthDraft {
            token: "token-secret\ninvalid".into(),
        });
        let error = compile_request(invalid_auth, &Default::default()).unwrap_err();
        let diagnostic = format!("{error:?} {error}");
        assert!(!diagnostic.contains("token-secret"));
    }
}
