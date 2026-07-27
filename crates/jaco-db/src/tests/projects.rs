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
