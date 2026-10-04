use std::num::NonZeroU32;

use crate::errors::DownloadInputError;

pub(crate) mod zgzl;

const ZGZL_MOBILE_HOST: &str = "m.zgzl.net";
const ZGZL_WEB_HOST: &str = "www.zgzl.net";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DownloadSource {
    Zgzl(ZgzlRange),
}

impl DownloadSource {
    pub(crate) fn range(&self) -> &ZgzlRange {
        match self {
            Self::Zgzl(range) => range,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ZgzlRange {
    Novel {
        novel_id: String,
    },
    Chapter {
        novel_id: String,
        chapter_id: String,
    },
    Page {
        novel_id: String,
        chapter_id: String,
        page: NonZeroU32,
    },
}

impl ZgzlRange {
    pub(crate) fn novel_id(&self) -> &str {
        match self {
            Self::Novel { novel_id }
            | Self::Chapter { novel_id, .. }
            | Self::Page { novel_id, .. } => novel_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedDownloadRequest {
    submitted_source: String,
    source: DownloadSource,
}

impl PreparedDownloadRequest {
    pub(crate) fn new(submitted_source: String, source: DownloadSource) -> Self {
        Self {
            submitted_source,
            source,
        }
    }

    pub(crate) fn submitted_source(&self) -> &str {
        &self.submitted_source
    }

    pub(crate) fn source(&self) -> &DownloadSource {
        &self.source
    }
}

pub(crate) fn parse_download_source(input: &str) -> Result<DownloadSource, DownloadInputError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(DownloadInputError::Empty);
    }
    if is_identifier(input) {
        return Ok(DownloadSource::Zgzl(ZgzlRange::Novel {
            novel_id: input.to_string(),
        }));
    }

    if !has_supported_origin(input) {
        return Err(DownloadInputError::Unsupported);
    }
    let url = reqwest::Url::parse(input).map_err(|_| DownloadInputError::Unsupported)?;
    let host = url.host_str().ok_or(DownloadInputError::Unsupported)?;
    if url.scheme() != "https"
        || !matches!(host, ZGZL_MOBILE_HOST | ZGZL_WEB_HOST)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some_and(|fragment| !fragment.is_empty())
    {
        return Err(DownloadInputError::Unsupported);
    }

    if let Some(novel_id) = parse_info_path(url.path()) {
        return Ok(DownloadSource::Zgzl(ZgzlRange::Novel {
            novel_id: novel_id.to_string(),
        }));
    }

    if host != ZGZL_MOBILE_HOST || url.fragment().is_some() {
        return Err(DownloadInputError::Unsupported);
    }

    let path = url
        .path()
        .strip_prefix("/read_")
        .ok_or(DownloadInputError::Unsupported)?;
    let (novel_id, leaf) = path
        .split_once('/')
        .ok_or(DownloadInputError::Unsupported)?;
    let leaf = leaf
        .strip_suffix(".html")
        .ok_or(DownloadInputError::Unsupported)?;
    if !is_identifier(novel_id) {
        return Err(DownloadInputError::Unsupported);
    }

    if let Some((chapter_id, page)) = leaf.split_once('_') {
        let page = page
            .parse::<u32>()
            .ok()
            .and_then(NonZeroU32::new)
            .filter(|_| page.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or(DownloadInputError::Unsupported)?;
        if !is_identifier(chapter_id) {
            return Err(DownloadInputError::Unsupported);
        }
        return Ok(DownloadSource::Zgzl(ZgzlRange::Page {
            novel_id: novel_id.to_string(),
            chapter_id: chapter_id.to_string(),
            page,
        }));
    }

    if !is_identifier(leaf) {
        return Err(DownloadInputError::Unsupported);
    }
    Ok(DownloadSource::Zgzl(ZgzlRange::Chapter {
        novel_id: novel_id.to_string(),
        chapter_id: leaf.to_string(),
    }))
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn has_supported_origin(input: &str) -> bool {
    input.starts_with("https://m.zgzl.net/") || input.starts_with("https://www.zgzl.net/")
}

fn parse_info_path(path: &str) -> Option<&str> {
    let novel_id = path.strip_prefix("/info_")?;
    let novel_id = novel_id.strip_suffix('/').unwrap_or(novel_id);
    is_identifier(novel_id).then_some(novel_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_supported_source_forms() {
        let page = NonZeroU32::new(3).unwrap();
        let cases = [
            (
                "otew",
                DownloadSource::Zgzl(ZgzlRange::Novel {
                    novel_id: "otew".into(),
                }),
            ),
            (
                " https://m.zgzl.net/info_otew/# ",
                DownloadSource::Zgzl(ZgzlRange::Novel {
                    novel_id: "otew".into(),
                }),
            ),
            (
                "https://m.zgzl.net/info_otew",
                DownloadSource::Zgzl(ZgzlRange::Novel {
                    novel_id: "otew".into(),
                }),
            ),
            (
                "https://www.zgzl.net/info_qg6k",
                DownloadSource::Zgzl(ZgzlRange::Novel {
                    novel_id: "qg6k".into(),
                }),
            ),
            (
                "https://www.zgzl.net/info_otew/#",
                DownloadSource::Zgzl(ZgzlRange::Novel {
                    novel_id: "otew".into(),
                }),
            ),
            (
                "https://m.zgzl.net/read_otew/68hq7.html",
                DownloadSource::Zgzl(ZgzlRange::Chapter {
                    novel_id: "otew".into(),
                    chapter_id: "68hq7".into(),
                }),
            ),
            (
                "https://m.zgzl.net/read_otew/68hq7_3.html",
                DownloadSource::Zgzl(ZgzlRange::Page {
                    novel_id: "otew".into(),
                    chapter_id: "68hq7".into(),
                    page,
                }),
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(parse_download_source(input), Ok(expected), "{input}");
        }
    }

    #[test]
    fn rejects_partial_or_unsupported_sources() {
        let cases = [
            "",
            "https://m.zgzl.net/",
            "http://m.zgzl.net/info_otew/",
            "https://www.zgzl.net/read_otew/68hq7.html",
            "https://m.zgzl.net:443/info_otew/",
            "https://user@m.zgzl.net/info_otew/",
            "https://m.zgzl.net/info_otew/?page=1",
            "https://m.zgzl.net/info_otew/#section",
            "https://m.zgzl.net/info_otew/trailing",
            "https://www.zgzl.net/info_otew/trailing",
            "https://m.zgzl.net/read_otew/68hq7.html/trailing",
            "https://m.zgzl.net/read_otew/68hq7.html#",
            "https://m.zgzl.net/read_otew/68hq7_2.html#",
            "https://m.zgzl.net/read_otew/68hq7_0.html",
            "https://m.zgzl.net/read_otew/68hq7_1.5.html",
            "https://example.com/read_otew/68hq7.html",
            "info_otew",
            "not-an-id",
        ];

        for input in cases {
            assert!(parse_download_source(input).is_err(), "{input}");
        }
    }
}
