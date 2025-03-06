/*
 * @Author: suxiaoshao 48886207+suxiaoshao@users.noreply.github.com
 * @Date: 2024-05-28 14:55:02
 * @LastEditors: suxiaoshao 48886207+suxiaoshao@users.noreply.github.com
 * @LastEditTime: 2025-03-03 17:08:16
 * @FilePath: /gpui-app/app/novel-download/src/crawler/implement/zgzl/chapter.rs
 * @Description: 这是默认设置,请设置`customMade`, 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 */
use std::{sync::LazyLock, time::Duration};

use async_stream::try_stream;
use futures::Stream;
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

use super::novel::Novel;

static SELECTOR_CHAPTER_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelbody > div.nr_function > h1").unwrap());
static SELECTOR_CHAPTER_CONTENT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#novelcontent").unwrap());

#[derive(Debug)]
pub struct Chapter {
    novel_id: String,
    chapter_id: String,
    title: String,
    page_count: u32,
}

impl Chapter {
    fn stream(&self) -> impl Stream<Item = NovelResult<String>> {
        try_stream! {
            for i in 1..=self.page_count {
                smol::Timer::after(Duration::from_secs(1)).await;
                let content = fetch_page_content(&self.chapter_id, &self.novel_id, i).await?;
                yield content;
            }
        }
    }
}

impl ChapterFn for Chapter {
    type Novel = Novel;

    async fn get_chapter_data(chapter_id: &str, novel_id: &str) -> NovelResult<Self> {
        let url = Self::get_url_from_id(chapter_id, novel_id);
        let html = get_doc(&url, "utf-8").await?;
        let title = parse_text(&html, &SELECTOR_CHAPTER_NAME)?;
        let (_, (title, count)) = parse_title(&title)?;
        Ok(Self {
            title,
            chapter_id: chapter_id.to_string(),
            novel_id: novel_id.to_string(),
            page_count: count,
        })
    }

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

    fn content_stream(&self) -> impl futures::Stream<Item = NovelResult<String>> {
        self.stream()
    }
}

fn parse_title(html: &str) -> IResult<&str, (String, u32)> {
    let (input, (title, _, chapter_id, _)) =
        (take_until("("), tag("(1 / "), float, tag(")")).parse(html)?;
    Ok((input, (title.trim().replace("/", "|"), chapter_id as u32)))
}

async fn fetch_page_content(chapter_id: &str, novel_id: &str, page_id: u32) -> NovelResult<String> {
    let page_url = format!("https://m.zgzl.net/read_{novel_id}/{chapter_id}_{page_id}.html");
    let html = retry(3, Duration::from_secs(1), || get_doc(&page_url, "utf-8")).await?;
    let content = parse_text(&html, &SELECTOR_CHAPTER_CONTENT)?;
    Ok(content)
}

async fn retry<T, E, Fut, F>(retries: usize, duration: Duration, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut count = 0;
    loop {
        let result = f().await;
        if result.is_ok() {
            return result;
        } else {
            smol::Timer::after(duration).await;
            count += 1;
            if count >= retries {
                return result;
            }
        }
    }
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
