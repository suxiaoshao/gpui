use super::chapter::{ChapterFn, ContentItem};
use crate::errors::NovelResult;
pub trait NovelFn: Sized + Send + Sync + Sized {
    type Chapter: ChapterFn;
    async fn get_novel_data(novel_id: &str) -> NovelResult<Self>;
    fn name(&self) -> &str;
    fn author_name(&self) -> &str;
    fn get_url_from_id(id: &str) -> String;
    fn content_stream(&self) -> impl futures::Stream<Item = NovelResult<ContentItem>>;
}
