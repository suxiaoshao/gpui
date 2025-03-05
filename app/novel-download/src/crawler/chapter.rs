use crate::errors::NovelResult;

pub trait ChapterFn: Sized + Send {
    type Novel: super::NovelFn;
    async fn get_chapter_data(chapter_id: &str, novel_id: &str) -> NovelResult<Self>;
    fn url(&self) -> String;
    fn title(&self) -> &str;
    fn chapter_id(&self) -> &str;
    fn novel_id(&self) -> &str;
    fn get_url_from_id(chapter_id: &str, novel_id: &str) -> String;
    fn content(&self) -> &str;
    fn content_stream(&self) -> impl futures::Stream<Item = NovelResult<String>>;
}
