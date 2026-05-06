use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum FeiwenError {
    #[error("数据库错误:{}",.0)]
    Sqlite(#[from] diesel::result::Error),
    #[error("数据库连接错误:{}",.0)]
    Connection(#[from] diesel::ConnectionError),
    #[error("数据库连接池错误:{}",.0)]
    Pool(#[from] diesel::r2d2::PoolError),
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
    #[error("文库页面被站点拦截")]
    FetchBlocked,
    #[error("文库页面需要登录")]
    FetchLogin,
    #[error("文库列表解析为空")]
    NovelListParse,
    #[error("word count 解析错误")]
    WordCountParse,
    #[error("count uint 解析错误,{}",.0)]
    CountUintParse(String),
    #[error("log file not found")]
    LogFileNotFound,
}

pub(crate) type FeiwenResult<T> = Result<T, FeiwenError>;
