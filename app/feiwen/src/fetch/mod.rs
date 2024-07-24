use reqwest::Client;

use crate::{errors::FeiwenResult, store::service::Novel};

use self::parse_novel::parse_page;

mod get_content;
pub mod parse_novel;

pub trait FetchRunner {
    fn get_url(&self) -> &str;
    fn get_cookies(&self) -> &str;
    fn get_start(&self) -> u32;
    fn get_end(&self) -> u32;
    fn resolve_novel(&mut self, novels: Vec<Novel>, page: u32) -> FeiwenResult<()>;
    async fn fetch(&mut self) -> FeiwenResult<()> {
        let url = self.get_url().to_owned();
        let cookies = self.get_cookies().to_owned();
        let end = self.get_end();
        let start = self.get_start();
        let client = Client::new();
        for i in start..=end {
            let data = fetch_one(&url, i, &cookies, &client).await?;
            self.resolve_novel(data, i)?;
        }
        Ok(())
    }
}

/// 获取一个文件
async fn fetch_one(
    url: &str,
    page: u32,
    cookies: &str,
    client: &Client,
) -> FeiwenResult<Vec<Novel>> {
    let body = get_content::get_content(url, page, cookies, client).await?;
    let data = parse_page(body)?;
    Ok(data)
}
