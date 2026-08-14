use std::{cell::Cell, path::PathBuf, rc::Rc};

use gpui_form::FormSchema;

use super::method::HttpMethod;

/// The only editable business model for one request editing session.
///
/// Collection identity deliberately belongs to `gpui-form`; draft rows do not carry IDs.
#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct RequestDraft {
    pub(crate) method: HttpMethod,
    #[form(required)]
    pub(crate) url: String,
    #[form(items)]
    pub(crate) headers: Vec<HeaderDraft>,
    #[form(child)]
    pub(crate) body: RequestBodyDraft,
    #[form(child)]
    pub(crate) auth: RequestAuthDraft,
    #[form(child)]
    pub(crate) settings: RequestSettingsDraft,
}

impl Default for RequestDraft {
    fn default() -> Self {
        Self {
            method: HttpMethod::Get,
            url: String::new(),
            headers: Vec::new(),
            body: RequestBodyDraft::None,
            auth: RequestAuthDraft::None,
            settings: RequestSettingsDraft::default(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct HeaderDraft {
    pub(crate) enabled: bool,
    pub(crate) name: String,
    pub(crate) value: String,
}

impl Default for HeaderDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            name: String::new(),
            value: String::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) enum RequestBodyDraft {
    None,
    FormData(FormDataDraft),
    UrlEncoded(UrlEncodedBodyDraft),
    Text(TextBodyDraft),
    Binary(BinaryBodyDraft),
}

impl RequestBodyDraft {
    pub(crate) fn form_data() -> Self {
        Self::FormData(FormDataDraft::default())
    }

    pub(crate) fn url_encoded() -> Self {
        Self::UrlEncoded(UrlEncodedBodyDraft::default())
    }

    pub(crate) fn text() -> Self {
        Self::Text(TextBodyDraft::default())
    }

    pub(crate) fn binary() -> Self {
        Self::Binary(BinaryBodyDraft::default())
    }
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct UrlEncodedBodyDraft {
    #[form(items)]
    pub(crate) fields: Vec<KeyValueDraft>,
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct KeyValueDraft {
    pub(crate) enabled: bool,
    pub(crate) key: String,
    pub(crate) value: String,
}

impl Default for KeyValueDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            key: String::new(),
            value: String::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct TextBodyDraft {
    pub(crate) format: TextBodyFormat,
    pub(crate) content: String,
}

impl Default for TextBodyDraft {
    fn default() -> Self {
        Self {
            format: TextBodyFormat::PlainText,
            content: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TextBodyFormat {
    #[default]
    PlainText,
    Json,
    JavaScript,
    Html,
    Xml,
    Css,
}

impl TextBodyFormat {
    pub(crate) const fn media_type(self) -> &'static str {
        match self {
            Self::PlainText => "text/plain",
            Self::Json => "application/json",
            Self::JavaScript => "application/javascript",
            Self::Html => "text/html",
            Self::Xml => "application/xml",
            Self::Css => "text/css",
        }
    }

    pub(crate) const fn editor_language(self) -> &'static str {
        match self {
            Self::PlainText => "plaintext",
            Self::Json => "json",
            Self::JavaScript => "javascript",
            Self::Html => "html",
            Self::Xml => "xml",
            Self::Css => "css",
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct FormDataDraft {
    #[form(items)]
    pub(crate) parts: Vec<MultipartPartDraft>,
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct MultipartPartDraft {
    pub(crate) enabled: bool,
    pub(crate) name: String,
    #[form(child)]
    pub(crate) value: MultipartPartValueDraft,
}

impl Default for MultipartPartDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            name: String::new(),
            value: MultipartPartValueDraft::Text(MultipartTextDraft::default()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) enum MultipartPartValueDraft {
    Text(MultipartTextDraft),
    File(MultipartFileDraft),
}

impl MultipartPartValueDraft {
    pub(crate) fn text() -> Self {
        Self::Text(MultipartTextDraft::default())
    }

    pub(crate) fn file() -> Self {
        Self::File(MultipartFileDraft::default())
    }
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct MultipartTextDraft {
    pub(crate) value: String,
    pub(crate) content_type: Option<String>,
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct MultipartFileDraft {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct BinaryBodyDraft {
    pub(crate) file: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) enum RequestAuthDraft {
    None,
    Basic(BasicAuthDraft),
    Bearer(BearerAuthDraft),
    ApiKey(ApiKeyAuthDraft),
}

impl RequestAuthDraft {
    pub(crate) fn basic() -> Self {
        Self::Basic(BasicAuthDraft::default())
    }

    pub(crate) fn bearer() -> Self {
        Self::Bearer(BearerAuthDraft::default())
    }

    pub(crate) fn api_key() -> Self {
        Self::ApiKey(ApiKeyAuthDraft::default())
    }
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct BasicAuthDraft {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct BearerAuthDraft {
    pub(crate) token: String,
}

#[derive(Clone, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct ApiKeyAuthDraft {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) location: ApiKeyLocation,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(crate) struct RequestSettingsDraft {
    pub(crate) follow_redirects: bool,
    pub(crate) follow_original_method: bool,
}

impl Default for RequestSettingsDraft {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            follow_original_method: false,
        }
    }
}

/// Page-owned transport settings. This is intentionally not part of the Form or a Store.
#[derive(Clone, Default)]
pub(crate) struct HttpClientTransportSettings {
    timeout_ms: Rc<Cell<u64>>,
}

impl HttpClientTransportSettings {
    pub(crate) fn timeout_ms(&self) -> u64 {
        self.timeout_ms.get()
    }

    pub(crate) fn set_timeout_ms(&self, timeout_ms: u64) {
        self.timeout_ms.set(timeout_ms);
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};
    use gpui_form::Form;

    use super::*;

    #[gpui::test]
    fn request_defaults_expose_the_complete_initial_topology(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = cx.new(|_| Form::new(RequestDraft::default()));

            assert_eq!(RequestDraft::METHOD.get(&form, cx), HttpMethod::Get);
            assert_eq!(RequestDraft::URL.get(&form, cx), "");
            assert!(RequestDraft::HEADERS.items(&form, cx).is_empty());
            assert!(matches!(
                RequestDraft::BODY.get(&form, cx),
                RequestBodyDraft::None
            ));
            assert!(matches!(
                RequestDraft::AUTH.get(&form, cx),
                RequestAuthDraft::None
            ));
            assert!(
                RequestDraft::ROOT
                    .then(RequestDraft::SETTINGS)
                    .then(RequestSettingsDraft::FOLLOW_REDIRECTS)
                    .get(&form, cx)
            );
            assert!(
                !RequestDraft::ROOT
                    .then(RequestDraft::SETTINGS)
                    .then(RequestSettingsDraft::FOLLOW_ORIGINAL_METHOD)
                    .get(&form, cx)
            );
        });
    }

    #[test]
    fn constructors_are_canonical_and_do_not_retain_dormant_payload() {
        assert!(matches!(
            RequestBodyDraft::form_data(),
            RequestBodyDraft::FormData(FormDataDraft { parts }) if parts.is_empty()
        ));
        assert!(matches!(
            RequestBodyDraft::url_encoded(),
            RequestBodyDraft::UrlEncoded(UrlEncodedBodyDraft { fields }) if fields.is_empty()
        ));
        assert!(matches!(
            RequestBodyDraft::text(),
            RequestBodyDraft::Text(TextBodyDraft { format, content })
                if format == TextBodyFormat::PlainText && content.is_empty()
        ));
        assert!(matches!(
            RequestBodyDraft::binary(),
            RequestBodyDraft::Binary(BinaryBodyDraft { file: None })
        ));
        assert!(matches!(
            RequestAuthDraft::api_key(),
            RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                name,
                value,
                location: ApiKeyLocation::Header,
            }) if name.is_empty() && value.is_empty()
        ));
        assert!(matches!(
            RequestAuthDraft::basic(),
            RequestAuthDraft::Basic(BasicAuthDraft { username, password })
                if username.is_empty() && password.is_empty()
        ));
        assert!(matches!(
            RequestAuthDraft::bearer(),
            RequestAuthDraft::Bearer(BearerAuthDraft { token }) if token.is_empty()
        ));
        assert!(matches!(
            MultipartPartValueDraft::text(),
            MultipartPartValueDraft::Text(MultipartTextDraft {
                value,
                content_type: None,
            }) if value.is_empty()
        ));
        assert!(matches!(
            MultipartPartValueDraft::file(),
            MultipartPartValueDraft::File(MultipartFileDraft { path: None })
        ));
    }

    #[test]
    fn all_methods_map_without_string_parsing_or_fallback() {
        let expected = [
            http::Method::GET,
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
            http::Method::PATCH,
            http::Method::HEAD,
            http::Method::OPTIONS,
            http::Method::TRACE,
            http::Method::CONNECT,
        ];

        for (draft, expected) in HttpMethod::ALL.into_iter().zip(expected) {
            assert_eq!(draft.to_http_method(), expected);
        }
    }

    #[test]
    fn text_formats_define_the_only_editable_media_and_editor_languages() {
        let expected = [
            (TextBodyFormat::PlainText, "text/plain", "plaintext"),
            (TextBodyFormat::Json, "application/json", "json"),
            (
                TextBodyFormat::JavaScript,
                "application/javascript",
                "javascript",
            ),
            (TextBodyFormat::Html, "text/html", "html"),
            (TextBodyFormat::Xml, "application/xml", "xml"),
            (TextBodyFormat::Css, "text/css", "css"),
        ];

        for (format, media_type, language) in expected {
            assert_eq!(format.media_type(), media_type);
            assert_eq!(format.editor_language(), language);
        }
    }

    #[test]
    fn transport_settings_clone_is_a_shared_page_local_handle() {
        let settings = HttpClientTransportSettings::default();
        let editor_handle = settings.clone();

        editor_handle.set_timeout_ms(1500);

        assert_eq!(settings.timeout_ms(), 1500);
    }
}
