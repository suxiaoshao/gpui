use http::{HeaderMap, HeaderValue};
use mime::Mime;
use std::{fmt, path::PathBuf, time::Duration};
use url::Url;
#[non_exhaustive]
pub struct PreparedRequest {
    pub method: http::Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: PreparedBody,
    pub body_content_type: BodyContentType,
    pub redirect: PreparedRedirect,
    pub timeout: Option<Duration>,
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

pub enum PreparedBody {
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

pub enum PreparedMultipartPart {
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
pub enum BodyContentType {
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
#[non_exhaustive]
pub struct PreparedRedirect {
    pub follow: bool,
    pub max_hops: u8,
    pub preserve_method: bool,
    pub forward_authorization_cross_host: bool,
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

impl PreparedRequest {
    pub fn new(
        method: http::Method,
        url: Url,
        headers: HeaderMap,
        body: PreparedBody,
        body_content_type: BodyContentType,
        redirect: PreparedRedirect,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            method,
            url,
            headers,
            body,
            body_content_type,
            redirect,
            timeout,
        }
    }
}

impl PreparedRedirect {
    pub fn new(
        follow: bool,
        max_hops: u8,
        preserve_method: bool,
        forward_authorization_cross_host: bool,
    ) -> Self {
        Self {
            follow,
            max_hops,
            preserve_method,
            forward_authorization_cross_host,
        }
    }
}
