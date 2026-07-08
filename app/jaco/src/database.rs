use std::path::Path;

use gpui::{App, Global};
use jaco_db::{FreshRepository, FreshStore};
use tracing::{Level, event};

use crate::{errors::JacoResult, state::config};

#[derive(Clone, Debug)]
pub(crate) struct FreshStoreGlobal {
    store: FreshStore,
}

impl Global for FreshStoreGlobal {}

impl FreshStoreGlobal {
    pub(crate) fn open_in_dir(data_dir: impl AsRef<Path>) -> JacoResult<Self> {
        Ok(Self {
            store: FreshStore::open_in_dir(data_dir)?,
        })
    }

    pub(crate) fn repository(&self) -> FreshRepository {
        self.store.repository()
    }

    pub(crate) fn path(&self) -> &Path {
        self.store.path()
    }
}

pub(crate) fn init_store(cx: &mut App) -> JacoResult<()> {
    let data_dir = config::data_dir(cx)?;
    let store = FreshStoreGlobal::open_in_dir(&data_dir)?;
    event!(
        Level::INFO,
        data_dir = ?data_dir,
        database_path = ?store.path(),
        "opened jaco fresh database"
    );
    cx.set_global(store);
    Ok(())
}

pub(crate) fn repository(cx: &App) -> FreshRepository {
    cx.global::<FreshStoreGlobal>().repository()
}

#[cfg(test)]
mod tests {
    use super::FreshStoreGlobal;
    use jaco_db::DATABASE_FILE;
    use tempfile::tempdir;

    #[test]
    fn fresh_store_global_opens_fresh_database_file_in_selected_dir() {
        let dir = tempdir().unwrap();
        let store = FreshStoreGlobal::open_in_dir(dir.path()).unwrap();

        assert_eq!(store.path(), dir.path().join(DATABASE_FILE));
        assert!(store.repository().metadata().is_ok());
    }
}
