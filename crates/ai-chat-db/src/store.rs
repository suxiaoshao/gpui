use crate::{FreshRepository, Result, migrations};
use diesel::{
    SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, CustomizeConnection, Pool},
};
use std::path::{Path, PathBuf};

pub const DATABASE_FILE: &str = "ai_chat_fresh.sqlite3";
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone, Debug)]
pub struct FreshStore {
    path: PathBuf,
    pool: DbPool,
}

impl FreshStore {
    pub fn open_in_dir(data_dir: impl AsRef<Path>) -> Result<Self> {
        Self::open(data_dir.as_ref().join(DATABASE_FILE))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let pool = create_pool(&path)?;
        {
            let mut conn = pool.get()?;
            migrations::bootstrap(&mut conn)?;
        }
        Ok(Self { path, pool })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub fn repository(&self) -> FreshRepository {
        FreshRepository::new(self.pool.clone())
    }

    #[cfg(test)]
    pub(crate) fn open_with_migrations(
        path: impl AsRef<Path>,
        migrations: &[migrations::Migration],
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let pool = create_pool(&path)?;
        {
            let mut conn = pool.get()?;
            migrations::bootstrap_with_migrations(&mut conn, migrations)?;
        }
        Ok(Self { path, pool })
    }
}

fn create_pool(path: &Path) -> Result<DbPool> {
    let url = path
        .to_str()
        .ok_or(crate::error::DbError::InvalidDatabasePath)?;
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    Ok(Pool::builder()
        .test_on_check_out(true)
        .connection_customizer(Box::new(SqlitePragmaCustomizer))
        .build(manager)?)
}

#[derive(Debug)]
struct SqlitePragmaCustomizer;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmaCustomizer {
    fn on_acquire(
        &self,
        conn: &mut SqliteConnection,
    ) -> std::result::Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA foreign_keys = ON;")
            .map_err(diesel::r2d2::Error::QueryError)
    }
}
