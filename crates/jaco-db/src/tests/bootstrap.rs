use super::*;

#[test]
fn creates_fresh_database_and_reads_internal_version() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    assert_eq!(store.path(), &dir.path().join(crate::DATABASE_FILE));

    let metadata = store.repository().metadata().unwrap();
    assert_eq!(metadata.schema_version, crate::repository::schema_version());
    assert_eq!(metadata.payload.store_kind, "fresh");
    assert_eq!(metadata.payload.legacy_policy, LegacyStorePolicy::Ignore);
}

#[test]
fn bootstrap_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(crate::DATABASE_FILE);
    let first = FreshStore::open_or_create_initial(&path).unwrap();
    let first_updated_at = first.repository().metadata().unwrap().updated_at;

    let second = FreshStore::open_or_create_initial(&path).unwrap();
    let metadata = second.repository().metadata().unwrap();
    assert_eq!(metadata.schema_version, crate::repository::schema_version());
    assert!(metadata.updated_at >= first_updated_at);

    let mut conn = second.pool().get().unwrap();
    assert_eq!(count(&mut conn, "schema_migrations"), 1);
}

#[test]
fn pooled_connections_configure_sqlite_busy_timeout() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let mut conn = store.pool().get().unwrap();

    assert_eq!(
        busy_timeout(&mut conn),
        crate::store::SQLITE_BUSY_TIMEOUT_MS
    );
}

#[test]
fn failed_migration_rolls_back_partial_schema() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.sqlite3");
    let migration = crate::migrations::broken_migration_for_test();
    let err = FreshStore::open_with_migrations(&path, &[migration]).unwrap_err();
    assert!(err.to_string().contains("database query failed"));

    let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    assert_eq!(count(&mut conn, "broken_rollback_probe"), 0);
    assert_eq!(count(&mut conn, "schema_migrations"), 0);
}

#[test]
fn empty_first_run_has_no_user_data_or_source_tables() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let mut conn = store.pool().get().unwrap();

    assert_eq!(count(&mut conn, "projects"), 0);
    assert_eq!(count(&mut conn, "conversations"), 0);

    let tables: HashSet<_> = store
        .repository()
        .table_names()
        .unwrap()
        .into_iter()
        .collect();
    for disallowed in [
        "skills",
        "skill_roots",
        "mcp_servers",
        "mcp_tools",
        "app_settings",
        "conversation_entry_fts",
    ] {
        assert!(!tables.contains(disallowed));
    }
}
