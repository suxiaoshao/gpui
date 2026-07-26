use crate::{DatabaseValidationError, FreshRepository, Result, migrations, validation};
use diesel::{
    Connection, SqliteConnection,
    connection::SimpleConnection,
    r2d2::{ConnectionManager, CustomizeConnection, Pool},
};
use std::path::{Path, PathBuf};

pub const DATABASE_FILE: &str = "jaco.sqlite3";
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub(crate) const SQLITE_BUSY_TIMEOUT_MS: i32 = 5_000;

#[derive(Clone, Debug)]
pub struct FreshStore {
    path: PathBuf,
    pool: DbPool,
}

impl FreshStore {
    pub fn open_or_create_initial(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let should_bootstrap = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.len() == 0,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(error.into()),
        };
        if !should_bootstrap {
            validate_read_only_path(&path)?;
        }
        let pool = create_pool(&path)?;
        if should_bootstrap {
            let mut conn = pool.get()?;
            migrations::bootstrap(&mut conn)?;
        }
        let store = Self { path, pool };
        store.validate()?;
        Ok(store)
    }

    pub fn reopen_validated_existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let metadata = std::fs::metadata(&path)?;
        if metadata.len() == 0 {
            return Err(
                DatabaseValidationError::Schema("database file is empty".to_string()).into(),
            );
        }
        validate_read_only_path(&path)?;
        let store = Self {
            pool: create_pool(&path)?,
            path,
        };
        store.validate()?;
        Ok(store)
    }

    pub fn create_fresh_staging(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("staging database already exists: {}", path.display()),
            )
            .into());
        }
        Self::open_or_create_initial(path)
    }

    pub fn validate(&self) -> std::result::Result<(), DatabaseValidationError> {
        validate_read_only_path(&self.path)
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

fn validate_read_only_path(path: &Path) -> std::result::Result<(), DatabaseValidationError> {
    let mut url = url::Url::from_file_path(path).map_err(|_| {
        DatabaseValidationError::Query(format!(
            "database path cannot be represented as a file URL: {}",
            path.display()
        ))
    })?;
    url.query_pairs_mut().append_pair("mode", "ro");
    let mut conn = SqliteConnection::establish(url.as_str())
        .map_err(|error| DatabaseValidationError::Query(error.to_string()))?;
    validation::validate_connection(&mut conn)
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
        conn.batch_execute(&format!(
            "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS};"
        ))
        .map_err(diesel::r2d2::Error::QueryError)
    }
}
