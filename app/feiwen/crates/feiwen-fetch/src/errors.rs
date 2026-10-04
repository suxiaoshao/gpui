use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum FetchError {
    #[error(transparent)]
    Data(#[from] feiwen_data::FeiwenError),
    #[error("请求头构造错误:{}",.0)]
    HeaderParse(#[from] reqwest::header::InvalidHeaderValue),
    #[error("请求错误:{}",.0)]
    Request(#[from] reqwest::Error),
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
}

pub(crate) type FetchResult<T> = Result<T, FetchError>;
