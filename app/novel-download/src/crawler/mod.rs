mod chapter;
mod implement;
mod novel;

use futures::{StreamExt, pin_mut};
use implement::Novel;

use crate::errors::NovelResult;

pub use self::{chapter::ChapterFn, novel::NovelFn};

struct NovelBaseData<'a> {
    name: &'a str,
    author_name: &'a str,
}

pub trait Fetch {
    fn on_fetch_base(&self, base_data: &NovelBaseData) -> NovelResult<()>;
    fn on_add_content(&self, content: &str) -> NovelResult<()>;
    async fn fetch(&self, novel_id: &str) -> NovelResult<()> {
        let data = Novel::get_novel_data(novel_id).await?;
        let base_data = NovelBaseData {
            name: data.name(),
            author_name: data.author_name(),
        };
        self.on_fetch_base(&base_data)?;
        let stream = data.content_stream();
        pin_mut!(stream);
        while let Some(content) = stream.next().await {
            self.on_add_content(&content?)?;
        }
        Ok(())
    }
}
