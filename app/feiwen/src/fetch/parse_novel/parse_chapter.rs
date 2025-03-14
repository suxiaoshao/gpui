/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-01-07 19:24:59
 * @FilePath: /tauri/packages/feiwen/src-tauri/src/fetch/parse_novel/parse_chapter.rs
 */
use std::sync::LazyLock;

use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_till},
    combinator::complete,
    number::complete::float,
};
use scraper::{Html, Selector};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::types::UrlWithName,
};

use super::{Title, parse_url::parse_url};

static SELECTOR_CHAPTER: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("div.col-xs-12.h5.brief > span.grayout.smaller-20 > a").unwrap()
});

pub(crate) fn parse_chapter(doc: &Html) -> FeiwenResult<Title> {
    let UrlWithName { name, href } = parse_url(doc, &SELECTOR_CHAPTER)?;
    let (_, id) =
        parse_chapter_url(&href).map_err(|err| FeiwenError::ChapterIdParse(err.to_string()))?;
    Ok(Title { name, id })
}

fn parse_chapter_url(name: &str) -> IResult<&str, i32> {
    let (name, (_, _, _, data)) = complete((
        tag("https://"),
        take_till(|c| c == '/'),
        tag("/posts/"),
        float,
    ))
    .parse(name)?;
    Ok((name, data as i32))
}

#[cfg(test)]
mod test {
    use super::parse_chapter_url;

    #[test]
    fn test() -> anyhow::Result<()> {
        let input = "https://xn--pxtr7m.com/posts/8722849";
        parse_chapter_url(input)?;
        Ok(())
    }
}
