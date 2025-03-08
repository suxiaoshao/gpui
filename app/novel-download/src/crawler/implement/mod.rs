use std::time::Duration;

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

fn parse_attr(html: &Html, selector: &Selector, attr: &str) -> NovelResult<String> {
    let element_ref = html.select(selector).next().ok_or(NovelError::ParseError)?;
    let url = element_ref
        .value()
        .attr(attr)
        .ok_or(NovelError::ParseError)?
        .to_string();
    Ok(url)
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
