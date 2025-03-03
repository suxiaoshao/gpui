use std::sync::LazyLock;

use futures::future::try_join_all;
use scraper::{Html, Selector};

use crate::{
    crawler::{
        ChapterFn, NovelFn,
        implement::{get_doc, parse_text},
    },
    errors::{NovelError, NovelResult},
};
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_until},
    character::complete::alphanumeric1,
    sequence::{delimited, preceded},
};

use super::chapter::Chapter;

static SELECTOR_NOVEL_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("body > div.main > div.catalog > div.catalog1 > h1").unwrap());
static SELECTOR_NOVEL_CHAPTERS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("body > div.main > div.info_chapters > ul:nth-child(5) > li > a").unwrap()
});

#[derive(Debug)]
pub struct Novel {
    novel_id: String,
    name: String,
    chapter_ids: Vec<String>,
}

impl NovelFn for Novel {
    type Chapter = Chapter;

    async fn get_novel_data(novel_id: &str) -> NovelResult<Self> {
        let url = format!("https://m.zgzl.net/info_{novel_id}/#");
        let html = get_doc(&url, "utf-8").await?;
        let name = parse_text(&html, &SELECTOR_NOVEL_NAME)?;
        let chapters: Vec<String> = parse_chapters(&html)?;

        Ok(Novel {
            novel_id: novel_id.to_string(),
            name,
            chapter_ids: chapters,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn chapters(&self) -> NovelResult<Vec<Self::Chapter>> {
        let data = try_join_all(
            self.chapter_ids
                .iter()
                .map(|chapter_id| Chapter::get_chapter_data(chapter_id, &self.novel_id)),
        )
        .await?;
        Ok(data)
    }

    fn get_url_from_id(id: &str) -> String {
        format!("https://m.zgzl.net/info_{id}/#")
    }
}

fn parse_chapters(document: &Html) -> NovelResult<Vec<String>> {
    let selector = &SELECTOR_NOVEL_CHAPTERS;
    fn parse_target(input: &str) -> IResult<&str, &str> {
        delimited(
            (tag("/read_"), take_until("/"), tag("/")),
            alphanumeric1,
            tag(".html"),
        )
        .parse(input)
    }
    let chapters = document
        .select(selector)
        .map(|element| {
            let href = element.value().attr("href").ok_or(NovelError::ParseError)?;
            let (_, id) = parse_target(href)?;
            Ok(id.to_string())
        })
        .collect::<NovelResult<Vec<_>>>()?;
    Ok(chapters)
}

#[cfg(test)]
mod test {
    use crate::crawler::{ChapterFn, NovelFn};

    use super::Novel;

    #[tokio::test]
    async fn test_fetch_novel() -> anyhow::Result<()> {
        let novel = Novel::get_novel_data("otew").await?;
        let chapters = novel.chapters().await?;
        let novel_content =
            chapters
                .iter()
                .map(|c| c.content())
                .fold(String::new(), |mut acc, content| {
                    acc.push_str(content);
                    acc
                });
        std::fs::write(
            format!("/Users/sushao/Downloads/{}.txt", novel.name()),
            novel_content,
        )?;
        Ok(())
    }
}
