use super::chapter::ContentItem;
use crate::errors::NovelResult;
pub trait NovelFn: Sized + Send + Sync + Sized {
    type Start;
    async fn get_novel_data(url: &str) -> NovelResult<(Self, Self::Start)>;
    fn name(&self) -> &str;
    fn author_name(&self) -> &str;
    fn get_url_from_id(id: &str) -> String;
    fn content_stream(
        &self,
        start: &Self::Start,
    ) -> impl futures::Stream<Item = NovelResult<ContentItem>>;
}
