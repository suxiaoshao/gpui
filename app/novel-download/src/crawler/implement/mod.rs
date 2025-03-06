use scraper::{Html, Selector};

use crate::errors::{NovelError, NovelResult};

mod zgzl;

pub use zgzl::Novel;

async fn get_doc(url: &str, charset: &str) -> NovelResult<Html> {
    let body = reqwest::get(url).await?.text_with_charset(charset).await?;
    let document = Html::parse_document(&body);
    Ok(document)
}

fn parse_text(html: &Html, selector: &Selector) -> NovelResult<String> {
    let element_ref = html.select(selector).next().ok_or(NovelError::ParseError)?;
    let text = element_ref
        .text()
        .map(|x| x.trim())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}
