use super::*;

#[test]
fn projects_can_be_listed_in_display_order() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    repo.insert_project(NewProject {
        path: "/tmp/zeta".to_string(),
        display_name: "Zeta".to_string(),
        kind: ProjectKind::Normal,
        pinned: false,
        removed: false,
        metadata: project_metadata(),
    })
    .unwrap();
    repo.insert_project(NewProject {
        path: "/tmp/alpha-b".to_string(),
        display_name: "Alpha".to_string(),
        kind: ProjectKind::Normal,
        pinned: false,
        removed: false,
        metadata: project_metadata(),
    })
    .unwrap();
    repo.insert_project(NewProject {
        path: "/tmp/alpha-a".to_string(),
        display_name: "Alpha".to_string(),
        kind: ProjectKind::Scratch,
        pinned: false,
        removed: false,
        metadata: ProjectMetadata {
            scratch_reason: Some("temporary".to_string()),
            git_root: None,
            last_active_conversation_id: None,
        },
    })
    .unwrap();

    let projects = repo.list_projects().unwrap();

    assert_eq!(
        projects
            .iter()
            .map(|project| (project.display_name.as_str(), project.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Alpha", "/tmp/alpha-a"),
            ("Alpha", "/tmp/alpha-b"),
            ("Zeta", "/tmp/zeta"),
        ]
    );
}

#[test]
fn project_can_be_loaded_by_path() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let inserted = repo.insert_project(project("by-path")).unwrap();

    let found = repo
        .get_project_by_path("/tmp/jaco-by-path")
        .unwrap()
        .expect("project exists");

    assert_eq!(found.id, inserted.id);
    assert!(repo.get_project_by_path("/tmp/missing").unwrap().is_none());
}

#[test]
fn sidebar_projects_filter_scratch_and_removed_projects() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let visible = repo.insert_project(project("visible")).unwrap();
    repo.insert_project(NewProject {
        path: "/tmp/hidden-scratch".to_string(),
        display_name: "Hidden Scratch".to_string(),
        kind: ProjectKind::Scratch,
        pinned: false,
        removed: false,
        metadata: ProjectMetadata {
            scratch_reason: Some("no-project".to_string()),
            git_root: None,
            last_active_conversation_id: None,
        },
    })
    .unwrap();
    let removed = repo.insert_project(project("removed")).unwrap();
    repo.set_project_removed(&removed.id, true).unwrap();

    let projects = repo.list_sidebar_projects().unwrap();

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, visible.id);
}

#[test]
fn sidebar_project_and_conversation_metadata_can_be_updated() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let project = repo.insert_project(project("pin")).unwrap();
    let project = repo.set_project_pinned(&project.id, true).unwrap();
    assert!(project.pinned);

    let renamed = repo
        .rename_project(&project.id, "Renamed Project".to_string())
        .unwrap();
    assert_eq!(renamed.display_name, "Renamed Project");

    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let conversation = repo
        .set_conversation_pinned(&conversation.id, false)
        .unwrap();

    assert!(!conversation.pinned);
}

#[test]
fn conversation_can_be_renamed() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("rename-conversation")).unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let renamed = repo
        .rename_conversation(&conversation.id, "Renamed Conversation".to_string())
        .unwrap();

    assert_eq!(renamed.title, "Renamed Conversation");
    assert_eq!(
        repo.get_conversation(&conversation.id).unwrap(),
        Some(renamed)
    );
}

#[test]
fn presentation_metadata_and_deletion_changes_preserve_recency() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("preserve-recency")).unwrap();
    let record = repo.insert_conversation(conversation(&project)).unwrap();
    let recency_at = record.recency_at;

    let metadata = repo
        .update_conversation_metadata(
            &record.id,
            ConversationMetadata {
                summary: Some("updated".to_string()),
                tags: vec!["preserved".to_string()],
            },
        )
        .unwrap();
    assert_eq!(metadata.recency_at, recency_at);
    let renamed = repo
        .rename_conversation(&record.id, "Renamed".to_string())
        .unwrap();
    assert_eq!(renamed.recency_at, recency_at);
    let pinned = repo.set_conversation_pinned(&record.id, true).unwrap();
    assert_eq!(pinned.recency_at, recency_at);
    let deleted = repo.soft_delete_conversation(&record.id).unwrap();
    assert_eq!(deleted.recency_at, recency_at);

    let batch = repo.insert_conversation(conversation(&project)).unwrap();
    let batch_recency = batch.recency_at;
    let deleted = repo
        .soft_delete_active_project_conversations(&project.id)
        .unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].id, batch.id);
    assert_eq!(deleted[0].recency_at, batch_recency);
}

#[test]
fn sidebar_lists_order_by_recency_then_conversation_id() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo
        .insert_project(scratch_project("recency-order"))
        .unwrap();
    let newest = repo.insert_conversation(conversation(&project)).unwrap();
    let tie_a = repo.insert_conversation(conversation(&project)).unwrap();
    let tie_b = repo.insert_conversation(conversation(&project)).unwrap();
    let mut conn = store.pool().get().unwrap();
    sql_query(
        "UPDATE conversations SET recency_at = '2026-03-01 00:00:00',
         updated_at = '2026-01-01 00:00:00' WHERE id = ?",
    )
    .bind::<Text, _>(&newest.id)
    .execute(&mut conn)
    .unwrap();
    for id in [&tie_a.id, &tie_b.id] {
        sql_query(
            "UPDATE conversations SET recency_at = '2026-02-01 00:00:00',
             updated_at = '2026-04-01 00:00:00' WHERE id = ?",
        )
        .bind::<Text, _>(id)
        .execute(&mut conn)
        .unwrap();
    }
    drop(conn);

    let mut tied = [tie_a.id, tie_b.id];
    tied.sort();
    let expected = vec![newest.id, tied[0].clone(), tied[1].clone()];
    let sidebar = repo
        .list_sidebar_conversations()
        .unwrap()
        .into_iter()
        .map(|conversation| conversation.id)
        .collect::<Vec<_>>();
    let scratch = repo
        .list_no_project_conversations("")
        .unwrap()
        .into_iter()
        .map(|conversation| conversation.id)
        .collect::<Vec<_>>();
    assert_eq!(sidebar, expected);
    assert_eq!(scratch, expected);
}

#[test]
fn soft_delete_active_project_conversations_returns_empty_for_empty_project() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("empty-archive")).unwrap();

    assert_eq!(
        repo.soft_delete_active_project_conversations(&project.id)
            .unwrap(),
        Vec::new()
    );
}

#[test]
fn soft_delete_active_project_conversations_only_changes_active_rows() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project_record = repo.insert_project(project("batch-archive")).unwrap();
    let active = repo
        .insert_conversation(conversation(&project_record))
        .unwrap();
    let already_deleted = repo
        .insert_conversation(conversation(&project_record))
        .unwrap();
    let already_deleted = repo.soft_delete_conversation(&already_deleted.id).unwrap();
    let other_project = repo.insert_project(project("batch-archive-other")).unwrap();
    let other = repo
        .insert_conversation(conversation(&other_project))
        .unwrap();

    let archived = repo
        .soft_delete_active_project_conversations(&project_record.id)
        .unwrap();

    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, active.id);
    assert_eq!(archived[0].status, ConversationStatus::Deleted);
    assert_eq!(
        repo.get_conversation(&already_deleted.id).unwrap(),
        Some(already_deleted)
    );
    assert_eq!(repo.get_conversation(&other.id).unwrap(), Some(other));
}

#[test]
fn soft_delete_active_project_conversations_returns_ids_in_stable_order() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();
    let project = repo.insert_project(project("batch-order")).unwrap();
    let first = repo.insert_conversation(conversation(&project)).unwrap();
    let second = repo.insert_conversation(conversation(&project)).unwrap();

    let mut expected = vec![first.id.clone(), second.id.clone()];
    expected.sort();
    let actual = repo
        .soft_delete_active_project_conversations(&project.id)
        .unwrap()
        .into_iter()
        .map(|conversation| conversation.id)
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn sidebar_conversations_exclude_deleted_and_removed_project_conversations() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let visible_project = repo
        .insert_project(project("visible-conversation"))
        .unwrap();
    let removed_project = repo
        .insert_project(project("removed-conversation"))
        .unwrap();
    repo.set_project_removed(&removed_project.id, true).unwrap();
    let scratch_project = repo
        .insert_project(NewProject {
            path: "/tmp/scratch-conversation".to_string(),
            display_name: "Scratch".to_string(),
            kind: ProjectKind::Scratch,
            pinned: false,
            removed: false,
            metadata: ProjectMetadata {
                scratch_reason: Some("no-project".to_string()),
                git_root: None,
                last_active_conversation_id: None,
            },
        })
        .unwrap();

    let visible = repo
        .insert_conversation(conversation(&visible_project))
        .unwrap();
    let deleted = repo
        .insert_conversation(conversation(&visible_project))
        .unwrap();
    repo.soft_delete_conversation(&deleted.id).unwrap();
    repo.insert_conversation(conversation(&removed_project))
        .unwrap();
    let scratch = repo
        .insert_conversation(conversation(&scratch_project))
        .unwrap();

    let conversations = repo.list_sidebar_conversations().unwrap();
    let ids = conversations
        .iter()
        .map(|conversation| conversation.id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&visible.id.as_str()));
    assert!(ids.contains(&scratch.id.as_str()));
    assert!(!ids.contains(&deleted.id.as_str()));
    assert_eq!(ids.len(), 2);
}

#[test]
fn sidebar_search_matches_title_project_and_item_text_with_visibility_filters() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let searchable_project = repo.insert_project(project("searchable-project")).unwrap();
    let removed_project = repo.insert_project(project("removed-search")).unwrap();
    repo.set_project_removed(&removed_project.id, true).unwrap();

    let mut by_title = conversation(&searchable_project);
    by_title.title = "Release notes".to_string();
    let by_title = repo.insert_conversation(by_title).unwrap();

    let mut by_item = conversation(&searchable_project);
    by_item.title = "Chat".to_string();
    let by_item = repo.insert_conversation(by_item).unwrap();
    repo.append_conversation_entry(message_item(&by_item.id, "contains unique needle"))
        .unwrap();

    let by_project = repo
        .insert_conversation(conversation(&searchable_project))
        .unwrap();
    let removed = repo
        .insert_conversation(conversation(&removed_project))
        .unwrap();
    repo.append_conversation_entry(message_item(&removed.id, "unique needle"))
        .unwrap();
    let deleted = repo
        .insert_conversation(conversation(&searchable_project))
        .unwrap();
    repo.append_conversation_entry(message_item(&deleted.id, "unique needle"))
        .unwrap();
    repo.soft_delete_conversation(&deleted.id).unwrap();

    let title_matches = repo.search_sidebar_conversations("release", 10).unwrap();
    assert_eq!(title_matches.len(), 1);
    assert_eq!(title_matches[0].id, by_title.id);

    let item_matches = repo
        .search_sidebar_conversations("unique needle", 10)
        .unwrap();
    assert_eq!(item_matches.len(), 1);
    assert_eq!(item_matches[0].id, by_item.id);

    let project_matches = repo
        .search_sidebar_conversations("searchable-project", 10)
        .unwrap();
    assert!(
        project_matches
            .iter()
            .any(|conversation| conversation.id == by_project.id)
    );
    assert!(
        !project_matches
            .iter()
            .any(|conversation| conversation.id == removed.id)
    );
    assert!(
        !project_matches
            .iter()
            .any(|conversation| conversation.id == deleted.id)
    );
}

#[test]
fn no_project_conversations_only_include_visible_scratch_active_conversations() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let normal_project = repo.insert_project(project("normal-no-project")).unwrap();
    let visible_scratch_project = repo
        .insert_project(scratch_project("visible-no-project"))
        .unwrap();
    let removed_scratch_project = repo
        .insert_project(scratch_project("removed-no-project"))
        .unwrap();
    repo.set_project_removed(&removed_scratch_project.id, true)
        .unwrap();

    let scratch = repo
        .insert_conversation(conversation(&visible_scratch_project))
        .unwrap();
    let deleted = repo
        .insert_conversation(conversation(&visible_scratch_project))
        .unwrap();
    repo.soft_delete_conversation(&deleted.id).unwrap();
    let normal = repo
        .insert_conversation(conversation(&normal_project))
        .unwrap();
    let removed = repo
        .insert_conversation(conversation(&removed_scratch_project))
        .unwrap();

    let conversations = repo.list_no_project_conversations("").unwrap();
    let ids = conversations
        .iter()
        .map(|conversation| conversation.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec![scratch.id.as_str()]);
    assert!(!ids.contains(&deleted.id.as_str()));
    assert!(!ids.contains(&normal.id.as_str()));
    assert!(!ids.contains(&removed.id.as_str()));
}

#[test]
fn no_project_search_matches_title_and_item_text_but_not_normal_project_text() {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    let repo = store.repository();

    let normal_project = repo.insert_project(project("release-normal")).unwrap();
    let scratch_project = repo
        .insert_project(scratch_project("scratch-release-project"))
        .unwrap();

    let mut by_title = conversation(&scratch_project);
    by_title.title = "Release notes".to_string();
    let by_title = repo.insert_conversation(by_title).unwrap();

    let mut by_item = conversation(&scratch_project);
    by_item.title = "Scratch chat".to_string();
    let by_item = repo.insert_conversation(by_item).unwrap();
    repo.append_conversation_entry(message_item(&by_item.id, "contains unique needle"))
        .unwrap();

    let normal = repo
        .insert_conversation(conversation(&normal_project))
        .unwrap();
    repo.append_conversation_entry(message_item(&normal.id, "unique needle"))
        .unwrap();

    let title_matches = repo.list_no_project_conversations("release").unwrap();
    assert_eq!(title_matches.len(), 1);
    assert_eq!(title_matches[0].id, by_title.id);

    let item_matches = repo.list_no_project_conversations("unique needle").unwrap();
    assert_eq!(item_matches.len(), 1);
    assert_eq!(item_matches[0].id, by_item.id);

    let project_matches = repo
        .list_no_project_conversations("scratch-release-project")
        .unwrap();
    assert!(project_matches.is_empty());
}
