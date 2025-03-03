use crate::{crawler::AuthorFn, errors::NovelResult};

use super::novel::Novel;

pub struct Author {}

impl AuthorFn for Author {
    type Novel = Novel;

    async fn get_author_data(author_id: &str) -> NovelResult<Self> {
        todo!()
    }

    fn url(&self) -> String {
        todo!()
    }

    fn name(&self) -> &str {
        todo!()
    }

    async fn novels(&self) -> NovelResult<Vec<Self::Novel>> {
        todo!()
    }

    fn get_url_from_id(id: &str) -> String {
        todo!()
    }

    fn id(&self) -> &str {
        todo!()
    }
}
