use std::sync::LazyLock;

use reqwest::Url;
use scraper::{Html, Selector};

use crate::{
    crawler::http::HttpClient,
    errors::{DownloadProblem, ParseProblem, ParseStage},
};

static SELECTOR_CHAPTER_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelbody > div.nr_function > h1").expect("valid selector"));
static SELECTOR_CHAPTER_CONTENT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelcontent").expect("valid selector"));

#[derive(Clone, Debug)]
pub(super) struct ZgzlChapter {
    title: String,
    page_count: u32,
    first_page: String,
}

impl ZgzlChapter {
    pub(super) async fn fetch(
        novel_id: &str,
        chapter_id: &str,
        http: &HttpClient,
    ) -> Result<Self, DownloadProblem> {
        let url = chapter_url(novel_id, chapter_id);
        let body = http.get_text(&url).await?;
        Self::parse(&body, url)
    }

    pub(super) fn title(&self) -> &str {
        &self.title
    }

    pub(super) fn page_count(&self) -> u32 {
        self.page_count
    }

    pub(super) fn first_page(&self) -> &str {
        &self.first_page
    }

    fn parse(body: &str, url: Url) -> Result<Self, DownloadProblem> {
        let document = Html::parse_document(body);
        let raw_title = select_text(&document, &SELECTOR_CHAPTER_NAME)
            .ok_or_else(|| ParseProblem::new(url.clone(), ParseStage::ChapterMetadata))?;
        let (title, page_count) = parse_title(&raw_title)
            .ok_or_else(|| ParseProblem::new(url.clone(), ParseStage::ChapterMetadata))?;
        let first_page = select_text(&document, &SELECTOR_CHAPTER_CONTENT)
            .ok_or_else(|| ParseProblem::new(url, ParseStage::ChapterContent))?;

        Ok(Self {
            title,
            page_count,
            first_page,
        })
    }
}

pub(super) async fn fetch_page_content(
    novel_id: &str,
    chapter_id: &str,
    page: u32,
    http: &HttpClient,
) -> Result<String, DownloadProblem> {
    let url = page_url(novel_id, chapter_id, page);
    let body = http.get_text(&url).await?;
    let document = Html::parse_document(&body);
    select_text(&document, &SELECTOR_CHAPTER_CONTENT)
        .ok_or_else(|| ParseProblem::new(url, ParseStage::PageContent).into())
}

pub(super) fn chapter_url(novel_id: &str, chapter_id: &str) -> Url {
    Url::parse(&format!(
        "https://m.zgzl.net/read_{novel_id}/{chapter_id}.html"
    ))
    .expect("typed IDs always make a valid source URL")
}

fn page_url(novel_id: &str, chapter_id: &str, page: u32) -> Url {
    Url::parse(&format!(
        "https://m.zgzl.net/read_{novel_id}/{chapter_id}_{page}.html"
    ))
    .expect("typed IDs and non-zero pages always make a valid source URL")
}

fn select_text(document: &Html, selector: &Selector) -> Option<String> {
    let text = document
        .select(selector)
        .next()?
        .text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn parse_title(value: &str) -> Option<(String, u32)> {
    let (title, pages) = value.rsplit_once("(1 / ")?;
    let pages = pages.strip_suffix(')')?.trim().parse::<u32>().ok()?;
    let title = title.trim().replace('/', "|");
    (!title.is_empty() && pages > 0).then_some((title, pages))
}

#[cfg(test)]
mod tests {
    use super::parse_title;

    #[test]
    fn parses_a_non_zero_page_count_from_the_first_page_title() {
        assert_eq!(
            parse_title("第一章 / 测试 (1 / 12)"),
            Some(("第一章 | 测试".to_string(), 12))
        );
    }

    #[test]
    fn rejects_malformed_or_zero_page_counts() {
        for title in ["第一章", "第一章 (1 / 0)", "第一章 (2 / 3)"] {
            assert_eq!(parse_title(title), None, "{title}");
        }
    }
}
