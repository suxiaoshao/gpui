use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum FeiwenError {
    #[error("数据库错误:{}",.0)]
    Sqlite(#[source] diesel::result::Error),
    #[error("数据库连接错误:{}",.0)]
    Connection(#[source] diesel::ConnectionError),
    #[error("数据库连接池错误:{}",.0)]
    Pool(#[source] diesel::r2d2::PoolError),
    #[error("数据库连接池获取链接错误:{}",.0)]
    GetConnection(#[from] diesel::r2d2::Error),
    #[error("文件系统错误:{}",.0)]
    Fs(#[from] std::io::Error),
    #[error("请求头构造错误:{}",.0)]
    HeaderParse(#[from] reqwest::header::InvalidHeaderValue),
    #[error("请求错误:{}",.0)]
    Request(#[from] reqwest::Error),
    #[error("获取不了历史记录数据库路径")]
    DbPath,
    #[error("desc 解析错误")]
    DescParse,
    #[error("href 解析错误")]
    HrefParse,
    #[error("novel id 解析错误:{}",.0)]
    NovelIdParse(String),
    #[error("author id 解析错误:{}",.0)]
    AuthorIdParse(String),
    #[error("author name 解析错误")]
    AuthorNameParse,
    #[error("chapter id 解析错误:{}",.0)]
    ChapterIdParse(String),
    #[error("count 解析错误")]
    CountParse,
    #[error("word count 解析错误")]
    WordCountParse,
    #[error("read count 解析错误")]
    ReadCountParse,
    #[error("reply count 解析错误")]
    ReplyCountParse,
    #[error("count uint 解析错误,{}",.0)]
    CountUintParse(String),
}

impl From<diesel::result::Error> for FeiwenError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Sqlite(error)
    }
}
impl From<diesel::ConnectionError> for FeiwenError {
    fn from(error: diesel::ConnectionError) -> Self {
        Self::Connection(error)
    }
}
impl From<diesel::r2d2::PoolError> for FeiwenError {
    fn from(error: diesel::r2d2::PoolError) -> Self {
        Self::Pool(error)
    }
}

pub(crate) type FeiwenResult<T> = Result<T, FeiwenError>;
