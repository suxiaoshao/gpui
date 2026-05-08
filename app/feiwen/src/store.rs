use std::{ops::Deref, path::PathBuf};

use crate::{
    APP_NAME,
    errors::{FeiwenError, FeiwenResult},
};
use diesel::{
    RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, Pool},
};
use gpui::App;
use tracing::{Level, event};

pub(crate) mod model;
pub(crate) mod query;
pub(crate) mod schema;
pub(crate) mod service;
pub(crate) mod types;

pub(crate) type DbConn = Pool<ConnectionManager<SqliteConnection>>;

pub(crate) struct Db(DbConn);

static DATABASE_FILE: &str = "data.sqlite";

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
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    let pool = Pool::builder().test_on_check_out(true).build(manager)?;
    event!(
        Level::INFO,
        db_path = %url_path.display(),
        created = not_exists,
        "database connection pool created"
    );
    if not_exists {
        create_tables(&pool)?;
    }
    migrate_tables(&pool)?;
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

fn check_data_file(url: &PathBuf) -> FeiwenResult<bool> {
    use std::fs::File;
    if !url.exists() {
        event!(Level::INFO, db_path = %url.display(), "creating database file");
        std::fs::create_dir_all(url.parent().ok_or(FeiwenError::DbPath)?)?;
        File::create(url)?;
        return Ok(true);
    }
    Ok(false)
}
fn create_tables(conn: &DbConn) -> FeiwenResult<()> {
    event!(Level::INFO, "creating initial database tables");
    let conn = &mut conn.get()?;
    conn.batch_execute(include_str!("../migrations/2022-05-15-162950_novel/up.sql"))?;
    conn.batch_execute(include_str!("../migrations/2022-05-15-163112_tag/up.sql"))?;
    conn.batch_execute(include_str!(
        "../migrations/2022-05-16-064913_novel_tag/up.sql"
    ))?;
    event!(Level::INFO, "initial database tables created");
    Ok(())
}

fn migrate_tables(conn: &DbConn) -> FeiwenResult<()> {
    let conn = &mut conn.get()?;
    let needs_nullable_counts_migration = novel_counts_are_not_null(conn)?;
    event!(
        Level::INFO,
        needs_nullable_counts_migration,
        "checked database migrations"
    );
    if needs_nullable_counts_migration {
        event!(Level::INFO, "running nullable novel counts migration");
        conn.batch_execute(include_str!(
            "../migrations/2026-05-06-000001_nullable_novel_counts/up.sql"
        ))?;
        event!(Level::INFO, "nullable novel counts migration completed");
    }
    Ok(())
}

fn novel_counts_are_not_null(conn: &mut SqliteConnection) -> FeiwenResult<bool> {
    #[derive(diesel::QueryableByName)]
    struct TableColumn {
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        notnull: i32,
    }

    let columns = diesel::sql_query("PRAGMA table_info(novel)").load::<TableColumn>(conn)?;
    Ok(columns.iter().any(|column| {
        matches!(column.name.as_str(), "read_count" | "reply_count") && column.notnull != 0
    }))
}
