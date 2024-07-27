use std::{ops::Deref, path::PathBuf};

use crate::errors::{FeiwenError, FeiwenResult};
use diesel::{
    connection::SimpleConnection,
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};
use gpui::AppContext;

pub(crate) mod model;
pub(crate) mod schema;
pub(crate) mod service;
pub(crate) mod types;
pub(crate) type DbConn = Pool<ConnectionManager<SqliteConnection>>;

pub(crate) struct Db(DbConn);

impl gpui::Global for Db {}

impl Deref for Db {
    type Target = DbConn;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn init_store(cx: &mut AppContext) {
    let conn = match establish_connection() {
        Ok(conn) => conn,
        Err(_) => {
            // todo log
            return;
        }
    };
    cx.set_global(Db(conn));
}

fn establish_connection() -> FeiwenResult<DbConn> {
    let url_path = get_data_url()?;
    let not_exists = check_data_file(&url_path)?;
    let url = url_path.to_str().ok_or(FeiwenError::DbPath)?;
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    if not_exists {
        create_tables(&pool)?;
    }
    Ok(pool)
}

fn get_data_url() -> FeiwenResult<PathBuf> {
    let data_path = dirs_next::config_dir()
        .ok_or(FeiwenError::DbPath)?
        .join("top.sushao.feiwen")
        .join("data.sqlite");
    Ok(data_path)
}

fn check_data_file(url: &PathBuf) -> FeiwenResult<bool> {
    use std::fs::File;
    if !url.exists() {
        std::fs::create_dir_all(url.parent().ok_or(FeiwenError::DbPath)?)?;
        File::create(url)?;
        return Ok(true);
    }
    Ok(false)
}
fn create_tables(conn: &DbConn) -> FeiwenResult<()> {
    let conn = &mut conn.get()?;
    conn.batch_execute(include_str!(
        "../../migrations/2022-05-15-162950_novel/up.sql"
    ))?;
    conn.batch_execute(include_str!(
        "../../migrations/2022-05-15-163112_tag/up.sql"
    ))?;
    conn.batch_execute(include_str!(
        "../../migrations/2022-05-16-064913_novel_tag/up.sql"
    ))?;
    Ok(())
}
