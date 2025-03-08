use crate::errors::NovelResult;

pub struct ContentItem {
    pub url: String,
    pub content: String,
}

impl ContentItem {
    pub fn new(url: String, content: String) -> Self {
        Self { url, content }
    }
}

pub trait ChapterFn: Sized + Send {
    type Novel: super::NovelFn;
    async fn get_chapter_data(chapter_id: &str, novel_id: &str) -> NovelResult<Self>;
    fn get_url_from_id(chapter_id: &str, novel_id: &str) -> String;
    fn title(&self) -> &str;
}
