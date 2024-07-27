#[derive(Debug, Clone)]
pub(crate) struct Title {
    pub(crate) name: String,
    pub(crate) id: i32,
}

#[derive(Debug, Clone)]
pub(crate) enum Author {
    Anonymous(String),
    Known(Title),
}

#[derive(Debug, Clone)]
pub(crate) struct NovelCount {
    pub(crate) word_count: i32,
    pub(crate) read_count: i32,
    pub(crate) reply_count: i32,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) struct UrlWithName {
    pub(crate) name: String,
    pub(crate) href: String,
}
