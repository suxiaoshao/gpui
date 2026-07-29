use super::*;

#[test]
fn legacy_store_files_coexist_with_fresh_database() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("history.sqlite3"), "legacy-v1").unwrap();
    fs::write(dir.path().join("history_v6.sqlite3"), "legacy-v6").unwrap();

    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();

    assert!(store.path().exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("history.sqlite3")).unwrap(),
        "legacy-v1"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("history_v6.sqlite3")).unwrap(),
        "legacy-v6"
    );
}
