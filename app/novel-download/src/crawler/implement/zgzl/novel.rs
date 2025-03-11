use std::{sync::LazyLock, time::Duration};

use async_stream::try_stream;
use scraper::{Html, Selector};

use crate::{
    crawler::{
        ChapterFn, NovelFn,
        chapter::ContentItem,
        implement::{get_doc, parse_attr, retry},
    },
    errors::{NovelError, NovelResult},
};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::alphanumeric1,
    combinator::{map, opt},
    number::complete::float,
};

use super::chapter::{Chapter, fetch_page_content};

static SELECTOR_NOVEL_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:novel:book_name']").unwrap());
static SELECTOR_NOVEL_CHAPTERS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("body > div.main > div.info_chapters > ul:nth-child(5) > li > a").unwrap()
});
static SELECTOR_NOVEL_AUTHOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:novel:author']").unwrap());

#[derive(Debug)]
pub struct Novel {
    novel_id: String,
    name: String,
    chapter_ids: Vec<String>,
    author_name: String,
}

#[derive(PartialEq, Eq, Debug)]
pub enum NovelStart {
    NovelId(String),
    Chapter {
        novel_id: String,
        chapter_id: String,
    },
    Page {
        novel_id: String,
        chapter_id: String,
        page_id: u32,
    },
}

impl NovelStart {
    fn novel_id(&self) -> &str {
        match self {
            NovelStart::NovelId(id) => id,
            NovelStart::Chapter { novel_id, .. } => novel_id,
            NovelStart::Page { novel_id, .. } => novel_id,
        }
    }
}

impl NovelFn for Novel {
    type Start = NovelStart;

    async fn get_novel_data(url: &str) -> NovelResult<(Self, Self::Start)> {
        let start = get_start_from_url(url);
        let novel_id = start.novel_id();
        let url = Self::get_url_from_id(novel_id);

        let html = get_doc(&url, "utf-8").await?;
        let name = parse_attr(&html, &SELECTOR_NOVEL_NAME, "content")?
            .trim()
            .replace("/", "|");
        let chapters: Vec<String> = get_chapters(&html)?;
        let author_name = parse_attr(&html, &SELECTOR_NOVEL_AUTHOR, "content")?;

        Ok((
            Novel {
                novel_id: novel_id.to_string(),
                name,
                chapter_ids: chapters,
                author_name,
            },
            start,
        ))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_url_from_id(id: &str) -> String {
        format!("https://m.zgzl.net/info_{id}/#")
    }

    fn author_name(&self) -> &str {
        &self.author_name
    }

    fn content_stream(
        &self,
        start: &Self::Start,
    ) -> impl futures::Stream<Item = NovelResult<ContentItem>> {
        try_stream! {
            match start {
                NovelStart::NovelId(_) => {
                    for chapter_id in &self.chapter_ids {
                        let chapter = Chapter::get_chapter_data(chapter_id, &self.novel_id).await?;
                        let url = Chapter::get_url_from_id(chapter_id,&self.novel_id);
                        yield ContentItem::new(url, format!("\n{}\n{}", chapter.title(), chapter.content));
                        for i in 2..=chapter.page_count {
                            let page_url = format!("https://m.zgzl.net/read_{}/{}_{}.html",self.novel_id,chapter_id,i);
                            let content = retry(3, Duration::from_secs(1), ||fetch_page_content(&page_url)).await?;
                            yield ContentItem::new(page_url, content);
                        }
                    }
                },
                NovelStart::Chapter { chapter_id,novel_id } => {
                    if let Some(start_index) = self.chapter_ids.iter().position(|s|s==chapter_id) {
                        for chapter_id in &self.chapter_ids[start_index..] {
                            let chapter = Chapter::get_chapter_data(chapter_id, novel_id).await?;
                            let url = Chapter::get_url_from_id(chapter_id,novel_id);
                            yield ContentItem::new(url, format!("\n{}\n{}", chapter.title(), chapter.content));
                            for i in 2..=chapter.page_count {
                                let page_url = format!("https://m.zgzl.net/read_{}/{}_{}.html",novel_id,chapter_id,i);
                                let content = retry(3, Duration::from_secs(1), ||fetch_page_content(&page_url)).await?;
                                yield ContentItem::new(page_url, content);
                            }
                        }
                    }
                },
                NovelStart::Page { chapter_id,  page_id,.. } => {
                    if let Some(start_index) = self.chapter_ids.iter().position(|s|s==chapter_id) {
                        let mut page_id = *page_id;
                        let chapter = Chapter::get_chapter_data(chapter_id, &self.novel_id).await?;
                        if page_id == 1 {
                            let url = Chapter::get_url_from_id(chapter_id,&self.novel_id);
                            page_id += 1;
                            yield ContentItem::new(url, format!("\n{}\n{}", chapter.title(), chapter.content));
                        }
                        for i in page_id..=chapter.page_count {
                            let page_url = format!("https://m.zgzl.net/read_{}/{}_{}.html",self.novel_id,chapter_id,i);
                            let content = retry(3, Duration::from_secs(1), ||fetch_page_content(&page_url)).await?;
                            yield ContentItem::new(page_url, content);
                        }
                        for chapter_id in &self.chapter_ids[start_index+1..] {
                            let chapter = Chapter::get_chapter_data(chapter_id, &self.novel_id).await?;
                            let url = Chapter::get_url_from_id(chapter_id,&self.novel_id);
                            yield ContentItem::new(url, format!("\n{}\n{}", chapter.title(), chapter.content));
                            for i in 2..=chapter.page_count {
                                let page_url = format!("https://m.zgzl.net/read_{}/{}_{}.html",self.novel_id,chapter_id,i);
                                let content = retry(3, Duration::from_secs(1), ||fetch_page_content(&page_url)).await?;
                                yield ContentItem::new(page_url, content);
                            }
                        }
                    }
                },
            }
        }
    }
}

fn get_start_from_url(url: &str) -> NovelStart {
    let (_, novel_start) = match alt((
        map(parse_novel_id, |novel_id| {
            NovelStart::NovelId(novel_id.to_string())
        }),
        map(parse_chapter, |(novel_id, chapter_id)| {
            NovelStart::Chapter {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
            }
        }),
        map(parse_page, |(novel_id, chapter_id, page_id)| {
            NovelStart::Page {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                page_id,
            }
        }),
    ))
    .parse(url)
    {
        Ok(data) => data,
        Err(_) => return NovelStart::NovelId(url.to_string()),
    };
    novel_start
}

fn parse_novel_id(input: &str) -> IResult<&str, &str> {
    let (input, (_, novel_id)) = (tag("https://m.zgzl.net/info_"), alphanumeric1).parse(input)?;
    Ok((input, novel_id))
}

fn parse_chapter(input: &str) -> IResult<&str, (&str, &str)> {
    let (input, (_, _, novel_id, _, chapter_id, _)) = (
        opt(tag("https://m.zgzl.net")),
        tag("/read_"),
        alphanumeric1,
        tag("/"),
        alphanumeric1,
        tag(".html"),
    )
        .parse(input)?;
    Ok((input, (novel_id, chapter_id)))
}

fn parse_page(input: &str) -> IResult<&str, (&str, &str, u32)> {
    let (input, (_, novel_id, _, chapter_id, _, page)) = (
        tag("https://m.zgzl.net/read_"),
        alphanumeric1,
        tag("/"),
        alphanumeric1,
        tag("_"),
        float,
    )
        .parse(input)?;
    Ok((input, (novel_id, chapter_id, page as u32)))
}

fn get_chapters(document: &Html) -> NovelResult<Vec<String>> {
    let selector = &SELECTOR_NOVEL_CHAPTERS;

    let chapters = document
        .select(selector)
        .map(|element| {
            let href = element.value().attr("href").ok_or(NovelError::ParseError)?;
            let (_, (_, id)) = parse_chapter(href)?;
            Ok(id.to_string())
        })
        .collect::<NovelResult<Vec<_>>>()?;
    Ok(chapters)
}

#[cfg(test)]
mod test {
    use crate::crawler::{
        NovelFn,
        implement::zgzl::novel::{NovelStart, get_start_from_url},
    };

    use super::Novel;

    #[tokio::test]
    async fn test_fetch_novel() -> anyhow::Result<()> {
        let (novel, start) =
            Novel::get_novel_data("https://m.zgzl.net/read_otew/68hq7.html").await?;
        println!("{novel:?}");
        assert_eq!(
            start,
            NovelStart::Chapter {
                novel_id: "otew".to_string(),
                chapter_id: "68hq7".to_string(),
            }
        );
        Ok(())
    }
    #[test]
    fn test_get_get_start_from_url() -> anyhow::Result<()> {
        let input = "https://m.zgzl.net/read_otew/68hq7.html";
        let data = get_start_from_url(input);
        assert_eq!(
            data,
            NovelStart::Chapter {
                novel_id: "otew".to_string(),
                chapter_id: "68hq7".to_string(),
            }
        );
        let input = "otew";
        let data = get_start_from_url(input);
        assert_eq!(data, NovelStart::NovelId("otew".to_string()));
        Ok(())
    }
}
