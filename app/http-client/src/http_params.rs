use gpui::*;
use prelude::FluentBuilder;
use url::Url;

use crate::{errors::HttpClientResult, http_form::HttpForm};

#[derive(IntoElement)]
pub struct HttpParams {
    pub http_form: Model<HttpForm>,
}

impl HttpParams {
    fn get_url(&self, cx: &mut WindowContext) -> HttpClientResult<Url> {
        let form = self.http_form.read(cx);
        let url = form.url.as_str();
        let url = url.parse::<url::Url>()?;
        Ok(url)
    }
    pub fn new(http_form: Model<HttpForm>) -> Self {
        Self { http_form }
    }
}

impl RenderOnce for HttpParams {
    fn render(self, cx: &mut WindowContext) -> impl IntoElement {
        let url = self.get_url(cx);
        div().when_some(url.ok(), |this, url| {
            let query = url.query_pairs();
            this.children(query.map(|(key, value)| {
                div()
                    .flex()
                    .flex_row()
                    .child(key.to_string())
                    .child(value.to_string())
            }))
        })
    }
}
