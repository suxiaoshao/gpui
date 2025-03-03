use super::chapter::ChapterFn;
use crate::errors::NovelResult;
pub trait NovelFn: Sized + Send + Sync + Sized {
    type Chapter: ChapterFn;
    fn get_novel_data(
        novel_id: &str,
    ) -> impl std::future::Future<Output = NovelResult<Self>> + Send;
    fn name(&self) -> &str;
    fn chapters(&self) -> impl std::future::Future<Output = NovelResult<Vec<Self::Chapter>>>;
    fn get_url_from_id(id: &str) -> String;
}
