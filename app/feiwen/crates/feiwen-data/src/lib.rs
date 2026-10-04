use std::path::{Path, PathBuf};

use duckdb::DuckdbConnectionManager;
use r2d2::Pool;
use tracing::{Level, event};

pub mod catalog;
pub mod database;
pub mod query;
pub mod service;
pub mod types;

pub type DbConn = Pool<DuckdbConnectionManager>;

mod errors;
pub use errors::{FeiwenError, FeiwenResult};
pub mod catalog_options;

#[derive(Clone)]
struct DatabasePath(Option<PathBuf>);
impl gpui_kit::Global for DatabasePath {}

pub fn init_store(path: Option<PathBuf>, cx: &mut gpui_kit::App) {
    cx.set_global(DatabasePath(path));
    event!(Level::INFO, "initializing feiwen store");
    catalog::init(cx);
    database::init(cx);
    event!(Level::INFO, "feiwen stores registered");
}

pub fn establish_connection_at(url_path: &Path) -> FeiwenResult<DbConn> {
    event!(Level::INFO, db_path = %url_path.display(), "opening feiwen database");
    let created = check_data_file(url_path)?;
    let pool = open_connection_at(url_path)?;
    let conn = pool.get()?;
    initialize_schema(&conn)?;
    event!(Level::INFO, db_path = %url_path.display(), created, "database ready");
    Ok(pool)
}

pub fn open_connection_at(url_path: &Path) -> FeiwenResult<DbConn> {
    let url = url_path.to_str().ok_or(FeiwenError::DbPath)?;
    let manager = DuckdbConnectionManager::file(url)?;
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    event!(Level::INFO, db_path = %url_path.display(), "database connection pool created");
    Ok(pool)
}

pub fn validate_schema(pool: &DbConn) -> FeiwenResult<()> {
    let conn = pool.get()?;
    let table_count = conn.query_row(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema = 'main' AND table_name IN ('novel', 'tag', 'novel_tag')",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let index_count = conn.query_row(
        "SELECT count(*) FROM duckdb_indexes() \
         WHERE schema_name = 'main' AND index_name IN (\
             'idx_novel_is_limit', \
             'idx_novel_reply_count', \
             'idx_novel_tag_tag_id_novel_id'\
         )",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if table_count != 3 || index_count != 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "database schema is incomplete: expected 3 tables and 3 indexes, found {table_count} tables and {index_count} indexes"
            ),
        )
        .into());
    }
    Ok(())
}

pub fn get_data_url(cx: &gpui_kit::App) -> FeiwenResult<PathBuf> {
    cx.global::<DatabasePath>()
        .0
        .clone()
        .ok_or(FeiwenError::DbPath)
}

fn check_data_file(url: &Path) -> FeiwenResult<bool> {
    if !url.exists() {
        event!(Level::INFO, db_path = %url.display(), "database file does not exist");
        std::fs::create_dir_all(url.parent().ok_or(FeiwenError::DbPath)?)?;
        return Ok(true);
    }
    Ok(false)
}

pub fn initialize_schema(conn: &duckdb::Connection) -> FeiwenResult<()> {
    event!(Level::INFO, "ensuring feiwen duckdb schema");
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS novel (
            id INTEGER PRIMARY KEY,
            name VARCHAR NOT NULL,
            "desc" VARCHAR NOT NULL,
            is_limit BOOLEAN NOT NULL,
            latest_chapter_name VARCHAR NOT NULL,
            latest_chapter_id INTEGER NOT NULL,
            word_count INTEGER NOT NULL,
            read_count INTEGER,
            reply_count INTEGER,
            author_id INTEGER,
            author_name VARCHAR NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tag (
            id INTEGER,
            name VARCHAR PRIMARY KEY
        );

        CREATE TABLE IF NOT EXISTS novel_tag (
            novel_id INTEGER NOT NULL,
            tag_id VARCHAR NOT NULL,
            PRIMARY KEY (novel_id, tag_id)
        );

        CREATE INDEX IF NOT EXISTS idx_novel_is_limit ON novel(is_limit);
        CREATE INDEX IF NOT EXISTS idx_novel_reply_count ON novel(reply_count);
        CREATE INDEX IF NOT EXISTS idx_novel_tag_tag_id_novel_id ON novel_tag(tag_id, novel_id);
        "#,
    )?;
    event!(Level::INFO, "feiwen duckdb schema ready");
    Ok(())
}
