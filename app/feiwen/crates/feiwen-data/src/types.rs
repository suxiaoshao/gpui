#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Title {
    pub name: gpui_kit::SharedString,
    pub id: i32,
}

#[derive(Debug, Clone)]
pub enum Author {
    Anonymous(gpui_kit::SharedString),
    Known(Title),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct NovelCount {
    pub word_count: i32,
    pub read_count: Option<i32>,
    pub reply_count: Option<i32>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub struct UrlWithName {
    pub name: String,
    pub href: String,
}

impl Title {
    pub fn new(name: gpui_kit::SharedString, id: i32) -> Self {
        Self { name, id }
    }
}

impl NovelCount {
    pub fn new(word_count: i32, read_count: Option<i32>, reply_count: Option<i32>) -> Self {
        Self {
            word_count,
            read_count,
            reply_count,
        }
    }
}

impl UrlWithName {
    pub fn new(name: String, href: String) -> Self {
        Self { name, href }
    }
}
