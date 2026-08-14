use std::sync::LazyLock;

use async_stream::try_stream;
use futures::Stream;
use reqwest::Url;
use scraper::{Html, Selector};

use crate::{
    crawler::{ContentItem, NovelMetadata, http::HttpClient},
    errors::{DownloadProblem, ParseProblem, ParseStage, RangeProblem, RangeProblemKind},
};

use super::{
    super::ZgzlRange,
    chapter::{ZgzlChapter, chapter_url, fetch_page_content},
};

static SELECTOR_NOVEL_NAME: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("meta[property='og:novel:book_name']").expect("valid selector")
});
static SELECTOR_NOVEL_CHAPTERS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("body > div.main > div.info_chapters > ul:nth-child(5) > li > a")
        .expect("valid selector")
});
static SELECTOR_NOVEL_AUTHOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:novel:author']").expect("valid selector"));

#[derive(Clone, Debug)]
pub(crate) struct ZgzlNovel {
    metadata: NovelMetadata,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedRange {
    requested: ZgzlRange,
    first_chapter_index: usize,
    first_page: u32,
    first_chapter: Option<ZgzlChapter>,
}

impl ZgzlNovel {
    pub(crate) async fn fetch_metadata(
        range: &ZgzlRange,
        http: &HttpClient,
    ) -> Result<Self, DownloadProblem> {
        let url = metadata_url(range.novel_id());
        let body = http.get_text(&url).await?;
        parse_metadata(&body, url, range.novel_id())
    }

    pub(crate) fn metadata(&self) -> &NovelMetadata {
        &self.metadata
    }

    pub(crate) fn validate_range(&self, range: &ZgzlRange) -> Result<ResolvedRange, RangeProblem> {
        let chapter_id = match range {
            ZgzlRange::Novel { .. } => {
                return Ok(ResolvedRange {
                    requested: range.clone(),
                    first_chapter_index: 0,
                    first_page: 1,
                    first_chapter: None,
                });
            }
            ZgzlRange::Chapter { chapter_id, .. } | ZgzlRange::Page { chapter_id, .. } => {
                chapter_id
            }
        };
        let first_chapter_index = self
            .metadata
            .chapter_ids()
            .iter()
            .position(|candidate| candidate == chapter_id)
            .ok_or_else(|| {
                RangeProblem::new(
                    range.clone(),
                    RangeProblemKind::MissingChapter {
                        chapter_id: chapter_id.clone(),
                    },
                )
            })?;
        let first_page = match range {
            ZgzlRange::Page { page, .. } => page.get(),
            ZgzlRange::Novel { .. } | ZgzlRange::Chapter { .. } => 1,
        };

        Ok(ResolvedRange {
            requested: range.clone(),
            first_chapter_index,
            first_page,
            first_chapter: None,
        })
    }

    pub(crate) async fn resolve_range(
        &self,
        range: &ZgzlRange,
        http: &HttpClient,
    ) -> Result<ResolvedRange, DownloadProblem> {
        let mut resolved = self.validate_range(range)?;
        if let ZgzlRange::Page {
            chapter_id, page, ..
        } = range
        {
            let chapter = ZgzlChapter::fetch(self.metadata.novel_id(), chapter_id, http).await?;
            validate_page_bounds(range, page.get(), chapter.page_count())?;
            resolved.first_chapter = Some(chapter);
        }
        Ok(resolved)
    }

    pub(crate) fn content_stream(
        &self,
        mut range: ResolvedRange,
        http: HttpClient,
    ) -> impl Stream<Item = Result<ContentItem, DownloadProblem>> + Send + 'static {
        let novel_id = self.metadata.novel_id().to_string();
        let chapters = self.metadata.chapter_ids().to_vec();
        try_stream! {
            for (index, chapter_id) in chapters.iter().enumerate().skip(range.first_chapter_index) {
                let chapter = if index == range.first_chapter_index {
                    match range.first_chapter.take() {
                        Some(chapter) => chapter,
                        None => ZgzlChapter::fetch(&novel_id, chapter_id, &http).await?,
                    }
                } else {
                    ZgzlChapter::fetch(&novel_id, chapter_id, &http).await?
                };
                let first_page = if index == range.first_chapter_index {
                    range.first_page
                } else {
                    1
                };
                validate_page_bounds(&range.requested, first_page, chapter.page_count())?;

                if first_page == 1 {
                    yield ContentItem::new(
                        chapter_url(&novel_id, chapter_id).to_string(),
                        format!("\n{}\n{}", chapter.title(), chapter.first_page()),
                    );
                }

                let first_following_page = first_page.max(2);
                for page in first_following_page..=chapter.page_count() {
                    let content = fetch_page_content(&novel_id, chapter_id, page, &http).await?;
                    let url = Url::parse(&format!(
                        "https://m.zgzl.net/read_{novel_id}/{chapter_id}_{page}.html"
                    ))
                    .expect("typed IDs and non-zero pages always make a valid source URL");
                    yield ContentItem::new(url.to_string(), content);
                }
            }
        }
    }
}

fn parse_metadata(body: &str, url: Url, novel_id: &str) -> Result<ZgzlNovel, DownloadProblem> {
    let document = Html::parse_document(body);
    let name = required_meta(&document, &SELECTOR_NOVEL_NAME)
        .ok_or_else(|| ParseProblem::new(url.clone(), ParseStage::NovelMetadata))?;
    let author = required_meta(&document, &SELECTOR_NOVEL_AUTHOR)
        .ok_or_else(|| ParseProblem::new(url.clone(), ParseStage::NovelMetadata))?;
    let chapter_ids = parse_chapter_ids(&document, novel_id)
        .ok_or_else(|| ParseProblem::new(url, ParseStage::ChapterList))?;

    Ok(ZgzlNovel {
        metadata: NovelMetadata::new(name, author, novel_id.to_string(), chapter_ids),
    })
}

fn metadata_url(novel_id: &str) -> Url {
    Url::parse(&format!("https://m.zgzl.net/info_{novel_id}/"))
        .expect("typed IDs always make a valid source URL")
}

fn required_meta(document: &Html, selector: &Selector) -> Option<String> {
    let value = document
        .select(selector)
        .next()?
        .value()
        .attr("content")?
        .trim();
    (!value.is_empty()).then(|| value.replace('/', "|"))
}

fn parse_chapter_ids(document: &Html, expected_novel_id: &str) -> Option<Vec<String>> {
    let chapter_ids = document
        .select(&SELECTOR_NOVEL_CHAPTERS)
        .map(|element| {
            let (novel_id, chapter_id) = parse_chapter_href(element.value().attr("href")?)?;
            (novel_id == expected_novel_id).then_some(chapter_id)
        })
        .collect::<Option<Vec<_>>>()?;
    (!chapter_ids.is_empty()).then_some(chapter_ids)
}

fn parse_chapter_href(href: &str) -> Option<(String, String)> {
    let path = href
        .strip_prefix("https://m.zgzl.net")
        .unwrap_or(href)
        .strip_prefix("/read_")?;
    let (novel_id, leaf) = path.split_once('/')?;
    let chapter_id = leaf.strip_suffix(".html")?;
    (is_identifier(novel_id) && is_identifier(chapter_id))
        .then(|| (novel_id.to_string(), chapter_id.to_string()))
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn validate_page_bounds(
    requested: &ZgzlRange,
    page: u32,
    page_count: u32,
) -> Result<(), RangeProblem> {
    if page <= page_count {
        return Ok(());
    }

    Err(RangeProblem::new(
        requested.clone(),
        RangeProblemKind::PageOutOfRange { page, page_count },
    ))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use crate::{
        crawler::{NovelMetadata, source::ZgzlRange},
        errors::RangeProblemKind,
    };

    use super::{ZgzlNovel, parse_chapter_href, parse_metadata, validate_page_bounds};

    fn fixture() -> ZgzlNovel {
        ZgzlNovel {
            metadata: NovelMetadata::new(
                "Novel".to_string(),
                "Author".to_string(),
                "novel".to_string(),
                vec!["chapter-a".to_string(), "chapter-b".to_string()],
            ),
        }
    }

    #[test]
    fn validates_missing_chapters_before_creating_a_stream() {
        let range = ZgzlRange::Chapter {
            novel_id: "novel".to_string(),
            chapter_id: "missing".to_string(),
        };

        let error = fixture().validate_range(&range).unwrap_err();
        assert_eq!(
            error.kind(),
            &RangeProblemKind::MissingChapter {
                chapter_id: "missing".to_string(),
            }
        );
    }

    #[test]
    fn accepts_page_ranges_for_an_existing_chapter() {
        let range = ZgzlRange::Page {
            novel_id: "novel".to_string(),
            chapter_id: "chapter-b".to_string(),
            page: NonZeroU32::new(2).unwrap(),
        };

        assert!(fixture().validate_range(&range).is_ok());
    }

    #[test]
    fn rejects_a_page_outside_the_resolved_first_chapter() {
        let range = ZgzlRange::Page {
            novel_id: "novel".to_string(),
            chapter_id: "chapter-b".to_string(),
            page: NonZeroU32::new(4).unwrap(),
        };

        let error = validate_page_bounds(&range, 4, 3).unwrap_err();
        assert_eq!(
            error.kind(),
            &RangeProblemKind::PageOutOfRange {
                page: 4,
                page_count: 3,
            }
        );
    }

    #[test]
    fn parses_only_complete_relative_chapter_links() {
        assert_eq!(
            parse_chapter_href("/read_novel/chapter.html"),
            Some(("novel".to_string(), "chapter".to_string()))
        );
        assert_eq!(
            parse_chapter_href("https://m.zgzl.net/read_novel/chapter.html"),
            Some(("novel".to_string(), "chapter".to_string()))
        );
        for href in [
            "/read_novel/chapter.html?query=1",
            "/read_novel/chapter.html/rest",
            "https://other.example/read_novel/chapter.html",
            "https://m.zgzl.net:443/read_novel/chapter.html",
            "chapter.html",
        ] {
            assert_eq!(parse_chapter_href(href), None, "{href}");
        }
    }

    #[test]
    fn metadata_fixture_requires_chapters_from_the_requested_novel() {
        let html = r#"
            <html>
              <head>
                <meta property="og:novel:book_name" content="Novel" />
                <meta property="og:novel:author" content="Author" />
              </head>
              <body><div class="main"><div class="info_chapters">
                <div></div><div></div><div></div><div></div>
                <ul><li><a href="/read_novel/chapter1.html">Chapter</a></li></ul>
              </div></div></body>
            </html>
        "#;
        let url = reqwest::Url::parse("https://m.zgzl.net/info_novel/").unwrap();
        let novel = parse_metadata(html, url.clone(), "novel").unwrap();
        assert_eq!(novel.metadata().chapter_ids(), &["chapter1"]);

        let mismatched = html.replace("/read_novel/", "/read_other/");
        assert!(parse_metadata(&mismatched, url, "novel").is_err());
    }
}
