/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-01-07 19:24:15
 * @FilePath: /tauri/packages/feiwen/src-tauri/src/fetch/parse_novel/parse_author.rs
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

use super::{Author, Title, parse_url::parse_url};
static SELECTOR_AUTHOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div:nth-child(1) > span.pull-right.smaller-5 > a").unwrap());
static SELECTOR_AUTHOR_NNONYMOUS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("div:nth-child(1) > span.pull-right.smaller-5 > span").unwrap()
});

pub(crate) fn parse_author(doc: &Html) -> FeiwenResult<Author> {
    let value = match parse_url(doc, &SELECTOR_AUTHOR) {
        Ok(UrlWithName { name, href }) => {
            let (_, id) = parse_author_url(&href)
                .map_err(|err| FeiwenError::AuthorIdParse(err.to_string()))?;
            Author::Known(Title { name, id })
        }
        Err(_) => {
            let author = match doc
                .select(&SELECTOR_AUTHOR_NNONYMOUS)
                .next()
                .ok_or(FeiwenError::AuthorNameParse)
            {
                Ok(element) => element.inner_html(),
                Err(_) => "".to_owned(),
            };
            Author::Anonymous(author)
        }
    };
    Ok(value)
}

fn parse_author_url(name: &str) -> IResult<&str, i32> {
    let (name, (_, _, _, data)) = complete((
        tag("https://"),
        take_till(|c| c == '/'),
        tag("/users/"),
        float,
    ))
    .parse(name)?;
    Ok((name, data as i32))
}

#[cfg(test)]
mod test {
    use super::parse_author_url;

    #[test]
    fn test() -> anyhow::Result<()> {
        let input = "https://xn--pxtr7m.com/users/538220";
        parse_author_url(input)?;
        Ok(())
    }
}
