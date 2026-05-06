use reqwest::{
    Client,
    header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, COOKIE, PRAGMA, REFERER, USER_AGENT},
};

use crate::errors::FeiwenResult;

pub(crate) async fn get_content(
    url: &str,
    page: u32,
    cookies: &str,
    client: &Client,
) -> FeiwenResult<String> {
    let body = client
        .get(url)
        .header(COOKIE, cookies)
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
        )
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9")
        .header(CACHE_CONTROL, "no-cache")
        .header(PRAGMA, "no-cache")
        .header(REFERER, url)
        .header("sec-ch-ua", "\"Chromium\";v=\"147\", \"Not.A/Brand\";v=\"8\"")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"macOS\"")
        .header("sec-fetch-dest", "document")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-user", "?1")
        .header("upgrade-insecure-requests", "1")
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
        )
        .query(&[("page", page)])
        .send()
        .await?
        .text()
        .await?;
    Ok(body)
}
