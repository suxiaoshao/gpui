use scraper::{Html, Selector};

use crate::{
    errors::{FetchError, FetchResult},
    store::types::UrlWithName,
};

pub(in crate::fetch::parse_novel) fn parse_url(
    doc: &Html,
    selector: &Selector,
) -> FetchResult<UrlWithName> {
    let title = doc.select(selector).next().ok_or(FetchError::HrefParse)?;
    let name = title.inner_html();
    let href = title
        .value()
        .attr("href")
        .ok_or(FetchError::HrefParse)?
        .to_string();
    Ok(UrlWithName::new(name, href))
}
