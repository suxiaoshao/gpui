use crate::errors::NovelResult;

use super::novel::NovelFn;

pub trait AuthorFn: Sized + Send + Sync + Sized {
    type Novel: NovelFn;
    fn get_author_data(
        author_id: &str,
    ) -> impl std::future::Future<Output = NovelResult<Self>> + Send;
    fn url(&self) -> String;
    fn name(&self) -> &str;
    fn novels(&self) -> impl std::future::Future<Output = NovelResult<Vec<Self::Novel>>> + Send;
    fn get_url_from_id(id: &str) -> String;
    fn id(&self) -> &str;
}
