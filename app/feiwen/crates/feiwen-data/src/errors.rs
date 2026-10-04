use thiserror::Error;

#[derive(Error, Debug)]
pub enum FeiwenError {
    #[error("数据库错误:{}",.0)]
    DuckDb(#[from] duckdb::Error),
    #[error("数据库连接池错误:{}",.0)]
    Pool(#[from] r2d2::Error),
    #[error("文件系统错误:{}",.0)]
    Fs(#[from] std::io::Error),
    #[error("获取不了历史记录数据库路径")]
    DbPath,
}

pub type FeiwenResult<T> = Result<T, FeiwenError>;
