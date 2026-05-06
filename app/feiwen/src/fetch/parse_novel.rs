/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-01-07 19:23:04
 * @FilePath: /tauri/packages/feiwen/src-tauri/src/fetch/parse_novel/mod.rs
 */
use std::sync::LazyLock;

use scraper::{Html, Selector};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::{
        service::Novel,
        types::{Author, Title},
    },
};

mod parse_author;
mod parse_chapter;
pub(crate) mod parse_count;
mod parse_tags;
mod parse_title;
pub(crate) mod parse_url;

static SELECTOR_ARTICLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("article > div").unwrap());
static SELECTOR_RAT: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("div:nth-child(1) > span:nth-child(1) > span.badge.bianyuan-tag.badge-tag")
        .unwrap()
});
static SELECTOR_DESC: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.col-xs-12.h5.brief-0 > span.smaller-5").unwrap());
static SELECTOR_FIREWALL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("body.firewall-page").unwrap());
static SELECTOR_FORM: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("form[action]").unwrap());

pub(crate) fn parse_page(body: String) -> FeiwenResult<Vec<Novel>> {
    let document = Html::parse_document(&body);
    if document.select(&SELECTOR_FIREWALL).next().is_some() {
        return Err(FeiwenError::FetchBlocked);
    }
    if document.select(&SELECTOR_FORM).any(|form| {
        form.value()
            .attr("action")
            .is_some_and(|action| action.ends_with("/login"))
    }) {
        return Err(FeiwenError::FetchLogin);
    }
    let novels = document
        .select(&SELECTOR_ARTICLE)
        .map(|x| Html::parse_document(&x.inner_html()))
        .map(parse_novel)
        .collect::<FeiwenResult<Vec<_>>>()?;
    if novels.is_empty() {
        return Err(FeiwenError::NovelListParse);
    }
    Ok(novels)
}

fn parse_novel(doc: Html) -> FeiwenResult<Novel> {
    let title = parse_title::parse_title(&doc)?;
    let author = parse_author::parse_author(&doc)?;
    let latest_chapter = parse_chapter::parse_chapter(&doc)?;
    let desc = doc
        .select(&SELECTOR_DESC)
        .next()
        .ok_or(FeiwenError::DescParse)?
        .inner_html();
    let count = parse_count::parse_count(&doc)?;
    let tags = parse_tags::parse_tags(&doc)?;
    let is_limit = doc.select(&SELECTOR_RAT).next().is_some();
    Ok(Novel {
        title,
        author,
        latest_chapter,
        desc,
        count,
        tags,
        is_limit,
    })
}

#[cfg(test)]
mod tests {
    use crate::errors::FeiwenError;

    use super::parse_page;

    #[test]
    fn parse_page_rejects_firewall_page() {
        let err = parse_page(r#"<body class="firewall-page"></body>"#.to_owned()).unwrap_err();
        assert!(matches!(err, FeiwenError::FetchBlocked));
    }

    #[test]
    fn parse_page_rejects_login_page() {
        let err = parse_page(
            r#"<form method="POST" action="https://xn--pxtr7m.com/login"></form>"#.to_owned(),
        )
        .unwrap_err();
        assert!(matches!(err, FeiwenError::FetchLogin));
    }

    #[test]
    fn parse_page_rejects_empty_novel_list() {
        let err = parse_page("<html></html>".to_owned()).unwrap_err();
        assert!(matches!(err, FeiwenError::NovelListParse));
    }
}
