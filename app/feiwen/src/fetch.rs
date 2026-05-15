use std::fmt;

use reqwest::Client;

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::service::Novel,
};
use tracing::{Level, event};

use self::parse_novel::parse_page;

mod get_content;
pub(crate) mod parse_novel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FetchErrorKind {
    Database,
    Network,
    Parse,
    Other,
}

impl FetchErrorKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            FetchErrorKind::Database => "database",
            FetchErrorKind::Network => "network",
            FetchErrorKind::Parse => "parse",
            FetchErrorKind::Other => "other",
        }
    }
}

impl fmt::Display for FetchErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FetchPageError {
    pub(crate) page: u32,
    pub(crate) kind: FetchErrorKind,
    pub(crate) message: String,
}

impl FetchPageError {
    pub(crate) fn new(page: u32, error: FeiwenError) -> Self {
        Self {
            page,
            kind: classify_error(&error),
            message: error.to_string(),
        }
    }
}

pub(crate) fn classify_error(error: &FeiwenError) -> FetchErrorKind {
    match error {
        FeiwenError::Sqlite(_)
        | FeiwenError::Connection(_)
        | FeiwenError::Pool(_)
        | FeiwenError::GetConnection(_) => FetchErrorKind::Database,
        FeiwenError::HeaderParse(_) | FeiwenError::Request(_) => FetchErrorKind::Network,
        FeiwenError::DescParse
        | FeiwenError::FetchBlocked
        | FeiwenError::FetchLogin
        | FeiwenError::HrefParse
        | FeiwenError::NovelListParse
        | FeiwenError::CountParse
        | FeiwenError::WordCountParse
        | FeiwenError::AuthorNameParse
        | FeiwenError::NovelIdParse(_)
        | FeiwenError::AuthorIdParse(_)
        | FeiwenError::ChapterIdParse(_)
        | FeiwenError::CountUintParse(_) => FetchErrorKind::Parse,
        _ => FetchErrorKind::Other,
    }
}

pub(crate) async fn fetch_page(
    url: &str,
    page: u32,
    cookies: &str,
    client: &Client,
) -> FeiwenResult<Vec<Novel>> {
    event!(
        Level::INFO,
        page,
        has_cookie = !cookies.is_empty(),
        "fetching feiwen page"
    );
    let body = get_content::get_content(url, page, cookies, client).await?;
    event!(
        Level::INFO,
        page,
        body_len = body.len(),
        "feiwen page response received"
    );
    let data = parse_page(body)?;
    event!(Level::INFO, page, count = data.len(), "feiwen page parsed");
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::{FetchErrorKind, FetchPageError};

    #[test]
    fn fetch_page_error_keeps_page_context() {
        let err = FetchPageError {
            page: 42,
            kind: FetchErrorKind::Parse,
            message: "parse failed".to_string(),
        };

        assert_eq!(err.page, 42);
        assert_eq!(err.kind, FetchErrorKind::Parse);
        assert_eq!(err.message, "parse failed");
    }
}
