/*
 * @Author: suxiaoshao 48886207+suxiaoshao@users.noreply.github.com
 * @Date: 2024-05-28 14:55:02
 * @LastEditors: suxiaoshao 48886207+suxiaoshao@users.noreply.github.com
 * @LastEditTime: 2025-03-03 17:08:16
 * @FilePath: /gpui-app/app/novel-download/src/crawler/implement/zgzl/chapter.rs
 * @Description: 这是默认设置,请设置`customMade`, 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 */
use std::sync::LazyLock;

use futures::future::try_join_all;
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_until},
    number::complete::float,
};
use scraper::Selector;

use crate::{
    crawler::{
        ChapterFn,
        implement::{get_doc, parse_text},
    },
    errors::NovelResult,
};

use super::{author::Author, novel::Novel};

static SELECTOR_CHAPTER_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelbody > div.nr_function > h1").unwrap());
static SELECTOR_CHAPTER_CONTENT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelcontent").unwrap());

#[derive(Debug)]
pub struct Chapter {
    novel_id: String,
    chapter_id: String,
    title: String,
    content: String,
}

impl ChapterFn for Chapter {
    type Author = Author;

    type Novel = Novel;

    fn url(&self) -> String {
        Self::get_url_from_id(&self.chapter_id, &self.novel_id)
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn chapter_id(&self) -> &str {
        &self.chapter_id
    }

    fn novel_id(&self) -> &str {
        todo!()
    }

    fn get_url_from_id(chapter_id: &str, novel_id: &str) -> String {
        format!("https://m.zgzl.net/read_{}/{}.html", novel_id, chapter_id)
    }

    fn content(&self) -> &str {
        &self.content
    }

    async fn get_chapter_data(chapter_id: &str, novel_id: &str) -> NovelResult<Self> {
        let url = Self::get_url_from_id(chapter_id, novel_id);
        let html = get_doc(&url, "utf-8").await?;
        let title = parse_text(&html, &SELECTOR_CHAPTER_NAME)?;
        let (_, (title, count)) = parse_title(&title)?;
        let mut content = parse_text(&html, &SELECTOR_CHAPTER_CONTENT)?;
        let contents = try_join_all(
            (2..=count).map(|page_id| fetch_page_content(chapter_id, novel_id, page_id)),
        )
        .await?;
        for c in contents {
            content.push_str(&c);
        }
        Ok(Self {
            title,
            chapter_id: chapter_id.to_string(),
            novel_id: novel_id.to_string(),
            content,
        })
    }
}

fn parse_title(html: &str) -> IResult<&str, (String, u32)> {
    let (input, (title, _, chapter_id, _)) =
        (take_until("("), tag("(1 / "), float, tag(")")).parse(html)?;
    Ok((input, (title.to_string(), chapter_id as u32)))
}

async fn fetch_page_content(chapter_id: &str, novel_id: &str, page_id: u32) -> NovelResult<String> {
    let page_url = format!("https://m.zgzl.net/read_{novel_id}/{chapter_id}_{page_id}.html");
    let html = get_doc(&page_url, "utf-8").await?;
    let content = parse_text(&html, &SELECTOR_CHAPTER_CONTENT)?;
    Ok(content)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_fetch_page_content() -> anyhow::Result<()> {
        let chapter_id = "68hqo";
        let novel_id = "otew";
        let content = Chapter::get_chapter_data(chapter_id, novel_id).await?;
        println!("{:?}", content);
        Ok(())
    }
}
