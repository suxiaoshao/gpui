use std::fmt;

use reqwest::Client;

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::service::Novel,
};

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
    let body = get_content::get_content(url, page, cookies, client).await?;
    let data = parse_page(body)?;
    Ok(data)
}

#[cfg(test)]
fn run_pages_until_error_sync<F>(
    start: u32,
    end: u32,
    mut fetch_page: F,
) -> Result<(), FetchPageError>
where
    F: FnMut(u32) -> Result<(), FetchPageError>,
{
    for page in start..=end {
        fetch_page(page)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FetchErrorKind, FetchPageError, run_pages_until_error_sync};

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

    #[test]
    fn run_pages_stops_at_first_failure() {
        let mut visited = Vec::new();

        let result = run_pages_until_error_sync(1, 5, |page| {
            visited.push(page);
            if page == 3 {
                Err(FetchPageError {
                    page,
                    kind: FetchErrorKind::Network,
                    message: "network failed".to_string(),
                })
            } else {
                Ok(())
            }
        });

        assert_eq!(visited, vec![1, 2, 3]);
        assert_eq!(result.unwrap_err().page, 3);
    }
}
