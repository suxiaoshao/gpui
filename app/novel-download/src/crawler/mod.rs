mod chapter;
mod implement;
mod novel;

use futures::{StreamExt, pin_mut};
use implement::Novel;

use crate::errors::{NovelError, NovelResult};

pub use self::{
    chapter::{ChapterFn, ContentItem},
    novel::NovelFn,
};

#[derive(Default, Clone)]
pub struct NovelBaseData<'a> {
    pub name: &'a str,
    pub author_name: &'a str,
}

pub trait Fetch {
    type BaseData;
    fn on_success(&mut self, base_data: &mut Self::BaseData) -> NovelResult<()>;
    fn get_novel_id(&self) -> &str;
    fn on_start(&mut self) -> NovelResult<()>;
    async fn on_fetch_base(&mut self, base_data: NovelBaseData) -> NovelResult<Self::BaseData>;
    async fn on_add_content(
        &mut self,
        content: &ContentItem,
        base_data: &mut Self::BaseData,
    ) -> NovelResult<()>;
    fn on_error(&mut self, error: &NovelError);
    async fn __inner_fetch(&mut self) -> NovelResult<()> {
        self.on_start()?;
        let data = Novel::get_novel_data(self.get_novel_id()).await?;
        let base_data = NovelBaseData {
            name: data.name(),
            author_name: data.author_name(),
        };
        let mut base_data = self.on_fetch_base(base_data).await?;
        let stream = data.content_stream();
        pin_mut!(stream);
        while let Some(content) = stream.next().await {
            self.on_add_content(&content?, &mut base_data).await?;
        }
        self.on_success(&mut base_data)?;
        Ok(())
    }
    async fn fetch(&mut self) {
        if let Err(err) = self.__inner_fetch().await {
            self.on_error(&err);
        }
    }
}
