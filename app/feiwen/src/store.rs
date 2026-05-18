use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

use crate::{
    APP_NAME,
    errors::{FeiwenError, FeiwenResult},
};
use duckdb::DuckdbConnectionManager;
use gpui::App;
use r2d2::Pool;
use tracing::{Level, event};

pub(crate) mod query;
pub(crate) mod service;
pub(crate) mod types;

pub(crate) type DbConn = Pool<DuckdbConnectionManager>;

pub(crate) struct Db(DbConn);

static DATABASE_FILE: &str = "data.duckdb";

impl gpui::Global for Db {}

impl Deref for Db {
    type Target = DbConn;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Db {
    pub(crate) fn pool(&self) -> DbConn {
        self.0.clone()
    }
}

pub(crate) fn init_store(cx: &mut App) {
    event!(Level::INFO, "initializing feiwen store");
    let conn = match establish_connection() {
        Ok(conn) => conn,
        Err(err) => {
            event!(Level::ERROR, error = %err, "failed to initialize feiwen store");
            return;
        }
    };
    cx.set_global(Db(conn));
    event!(Level::INFO, "feiwen store global registered");
}

fn establish_connection() -> FeiwenResult<DbConn> {
    let url_path = get_data_url()?;
    event!(Level::INFO, db_path = %url_path.display(), "opening feiwen database");
    let not_exists = check_data_file(&url_path)?;
    let url = url_path.to_str().ok_or(FeiwenError::DbPath)?;
    let manager = DuckdbConnectionManager::file(url)?;
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    event!(
        Level::INFO,
        db_path = %url_path.display(),
        created = not_exists,
        "database connection pool created"
    );
    let conn = pool.get()?;
    initialize_schema(&conn)?;
    event!(Level::INFO, db_path = %url_path.display(), "database ready");
    Ok(pool)
}

fn get_data_url() -> FeiwenResult<PathBuf> {
    let data_path = dirs_next::config_dir()
        .ok_or(FeiwenError::DbPath)?
        .join(APP_NAME)
        .join(DATABASE_FILE);
    Ok(data_path)
}

fn check_data_file(url: &Path) -> FeiwenResult<bool> {
    if !url.exists() {
        event!(Level::INFO, db_path = %url.display(), "database file does not exist");
        std::fs::create_dir_all(url.parent().ok_or(FeiwenError::DbPath)?)?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn initialize_schema(conn: &duckdb::Connection) -> FeiwenResult<()> {
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
