use super::*;
use gpui::TestAppContext;
use std::{cell::Cell, rc::Rc};

#[test]
fn toml_config_preserves_public_field_shape() {
    let config = JacoConfig {
        app_settings: AppSettingsConfig {
            language: AppLanguage::Chinese,
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            ..Default::default()
        },
        chat_form: ChatFormConfig {
            model: Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            }),
            approval_mode: ToolApprovalMode::FullAccess,
            ..Default::default()
        },
        ..Default::default()
    };

    let source = toml::to_string_pretty(&config).unwrap();
    assert!(!source.contains("[storage]"));
    assert!(source.contains("[app_settings]"));
    assert!(source.contains("[chat_form.model]"));
    assert!(!source.contains("source_bytes"));
    assert!(!source.contains("config_path"));
    assert_eq!(toml::from_str::<JacoConfig>(&source).unwrap(), config);
}

#[test]
fn malformed_config_is_unavailable_without_default_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let source = b"not = [valid".to_vec();
    fs::write(&path, &source).unwrap();

    let error = load_for_operation(&path).unwrap_err();
    assert!(matches!(error, ConfigProblem::Parse { .. }));
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn missing_config_is_atomically_created_as_ready() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/config.toml");

    let data = load_for_operation(&path).unwrap();

    assert_eq!(&data.value, &JacoConfig::default());
    assert_eq!(data.source_bytes, fs::read(&path).unwrap());
    assert!(!data.source_bytes.is_empty());
}

#[test]
fn observer_persistent_delete_requires_confirmation_before_default_recreation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        toml::to_string_pretty(&JacoConfig::default()).unwrap(),
    )
    .unwrap();
    fs::remove_file(&path).unwrap();

    assert!(read_for_observer(&path).unwrap().is_none());
    assert!(!path.exists());

    let recreated = load_or_create(&path).unwrap();
    assert_eq!(recreated.value, JacoConfig::default());
    assert_eq!(recreated.source_bytes, fs::read(path).unwrap());
}

#[test]
fn observer_transient_remove_uses_replacement_that_wins_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        toml::to_string_pretty(&JacoConfig::default()).unwrap(),
    )
    .unwrap();
    fs::remove_file(&path).unwrap();

    assert!(read_for_observer(&path).unwrap().is_none());

    let replacement = JacoConfig {
        app_settings: AppSettingsConfig {
            language: AppLanguage::Chinese,
            ..Default::default()
        },
        ..Default::default()
    };
    let replacement_bytes = toml::to_string_pretty(&replacement).unwrap().into_bytes();
    fs::write(&path, &replacement_bytes).unwrap();

    let observed = load_or_create(&path).unwrap();
    assert_eq!(observed.value, replacement);
    assert_eq!(observed.source_bytes, replacement_bytes);
    assert_eq!(fs::read(path).unwrap(), replacement_bytes);
}

#[test]
fn locked_initial_config_retains_a_retryable_pending_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let lock = persistence::FileLock::acquire(&path.with_extension("toml.lock")).unwrap();

    let ConfigProblem::Locked { pending, .. } = load_for_operation(&path).unwrap_err() else {
        panic!("expected locked config");
    };
    drop(lock);

    let data = write_pending_at(&path, None, pending).unwrap();
    assert_eq!(data.value, JacoConfig::default());
    assert_eq!(data.source_bytes, fs::read(path).unwrap());
}

#[test]
fn initial_operation_is_settled_before_installation() {
    let dir = tempfile::tempdir().unwrap();
    let valid_path = dir.path().join("valid.toml");
    let ready = initial_operation(load_for_operation(&valid_path));
    assert!(matches!(ready, ConfigOperation::Ready(_)));
    assert!(!ready.is_running());

    let invalid_path = dir.path().join("invalid.toml");
    fs::write(&invalid_path, b"not = [valid").unwrap();
    let unavailable = initial_operation(load_for_operation(&invalid_path));
    assert!(matches!(unavailable, ConfigOperation::Unavailable(_)));
    assert!(!unavailable.is_running());
}

#[test]
fn legacy_storage_table_is_ignored_and_not_written_back() {
    let config: JacoConfig = toml::from_str("[storage]\ndata_dir = 'legacy'\n").unwrap();
    assert!(
        !toml::to_string_pretty(&config)
            .unwrap()
            .contains("[storage]")
    );
}

#[test]
fn external_change_before_replace_preserves_disk_and_pending() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let current = load_for_operation(&path).unwrap();
    let external = b"# changed outside Jaco\n".to_vec();
    fs::write(&path, &external).unwrap();
    let value = JacoConfig {
        app_settings: AppSettingsConfig {
            language: AppLanguage::Chinese,
            ..Default::default()
        },
        ..Default::default()
    };
    let bytes = toml::to_string_pretty(&value).unwrap().into_bytes();
    let pending = Arc::new(PendingConfig {
        data: data_from_value(path.clone(), value, bytes.clone()),
        bytes,
    });

    let error = write_pending(&current, pending).unwrap_err();

    assert!(matches!(error, ConfigProblem::ExternalChange { .. }));
    assert_eq!(fs::read(path).unwrap(), external);
}

#[test]
fn backup_bytes_are_exact_and_collision_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let invalid = b"not = [valid".to_vec();
    fs::write(&path, &invalid).unwrap();
    let occupied = dir.path().join("config.invalid.toml");
    fs::write(&occupied, b"keep me").unwrap();

    let data = backup_and_replace(&path, ConfigBackupIntent::CreateDefault, None).unwrap();

    assert_eq!(data.value, JacoConfig::default());
    assert_eq!(fs::read(&occupied).unwrap(), b"keep me");
    assert_eq!(
        fs::read(dir.path().join("config.invalid-1.toml")).unwrap(),
        invalid
    );
}

#[test]
fn repair_support_matrix_is_explicit() {
    let path = PathBuf::from("config.toml");
    let parse = ConfigProblem::Parse {
        path: path.clone(),
        message: "bad TOML".to_string(),
    };
    assert!(parse.supports(ConfigRepair::Reload));
    assert!(parse.supports(ConfigRepair::BackupAndCreateDefault));
    assert!(!parse.supports(ConfigRepair::RetryWrite));
    assert!(!parse.supports(ConfigRepair::BackupAndOverwritePending));
}

#[gpui::test]
fn chat_preferences_commit_synchronously_without_leaving_ready(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        update_chat_form_config(cx, |config| {
            config.model = Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            });
        })
        .unwrap();

        store(cx).read(cx, |operation| {
            assert!(matches!(operation, ConfigOperation::Ready(_)));
            assert!(!operation.is_running());
        });
    });

    let persisted = JacoConfig::load_from_path_for_test(&path).unwrap();
    assert_eq!(
        persisted
            .chat_form
            .model
            .as_ref()
            .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
        Some(("provider-1", "gpt-5"))
    );
}

#[gpui::test]
fn external_change_during_synchronous_commit_degrades_with_old_and_pending_data(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();
    let external = b"# changed outside Jaco\n".to_vec();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        fs::write(&path, &external).unwrap();

        let result = update_chat_form_config(cx, |config| {
            config.model = Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            });
        });

        assert!(result.is_err());
        store(cx).read(cx, |operation| {
            let ConfigOperation::Degraded(degraded) = operation else {
                panic!("expected degraded config");
            };
            assert_eq!(degraded.data().chat_form.model, None);
            let ConfigProblem::ExternalChange { pending, .. } = degraded.problem() else {
                panic!("expected external-change problem");
            };
            assert_eq!(
                pending
                    .data
                    .chat_form
                    .model
                    .as_ref()
                    .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
                Some(("provider-1", "gpt-5"))
            );
        });
    });

    assert_eq!(fs::read(path).unwrap(), external);
}

#[gpui::test]
fn locked_synchronous_commit_degrades_with_old_and_pending_data(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        let _lock = persistence::FileLock::acquire(&path.with_extension("toml.lock")).unwrap();

        let result = update_chat_form_config(cx, |config| {
            config.model = Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            });
        });

        assert!(result.is_err());
        store(cx).read(cx, |operation| {
            let ConfigOperation::Degraded(degraded) = operation else {
                panic!("expected degraded config");
            };
            assert_eq!(degraded.data().chat_form.model, None);
            let ConfigProblem::Locked { pending, .. } = degraded.problem() else {
                panic!("expected locked problem");
            };
            assert!(degraded.problem().supports(ConfigRepair::RetryWrite));
            assert_eq!(
                pending
                    .data
                    .chat_form
                    .model
                    .as_ref()
                    .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
                Some(("provider-1", "gpt-5"))
            );
        });
    });
}

#[gpui::test]
fn mcp_fragment_cas_rejects_changed_entry_without_publishing_or_writing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let baseline = McpServerTomlConfig {
        display_name: Some("baseline".to_string()),
        command: Some("mcp".to_string()),
        ..Default::default()
    };
    let externally_changed = McpServerTomlConfig {
        display_name: Some("external".to_string()),
        command: Some("mcp".to_string()),
        ..Default::default()
    };
    let mut initial = JacoConfig::default();
    initial
        .mcp_servers
        .insert("server".to_string(), externally_changed);
    let bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        let result = upsert_mcp_server_if_unchanged(
            cx,
            Some("server"),
            Some(&baseline),
            "server".to_string(),
            McpServerTomlConfig {
                display_name: Some("draft".to_string()),
                command: Some("mcp".to_string()),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(JacoError::ConfigEditConflict(
                crate::errors::ConfigEditConflict::Changed { .. }
            ))
        ));
        assert!(store(cx).read(cx, |operation| matches!(
            operation,
            ConfigOperation::Ready(_)
        )));
    });

    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[gpui::test]
fn mcp_fragment_cas_preserves_unrelated_external_entries(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let baseline = McpServerTomlConfig {
        display_name: Some("baseline".to_string()),
        command: Some("mcp".to_string()),
        ..Default::default()
    };
    let unrelated = McpServerTomlConfig {
        display_name: Some("unrelated".to_string()),
        command: Some("mcp".to_string()),
        ..Default::default()
    };
    let mut initial = JacoConfig::default();
    initial
        .mcp_servers
        .insert("server".to_string(), baseline.clone());
    initial
        .mcp_servers
        .insert("other".to_string(), unrelated.clone());
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        upsert_mcp_server_if_unchanged(
            cx,
            Some("server"),
            Some(&baseline),
            "renamed".to_string(),
            McpServerTomlConfig {
                display_name: Some("saved".to_string()),
                command: Some("mcp".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
    });

    let saved = JacoConfig::load_from_path_for_test(&path).unwrap();
    assert!(!saved.mcp_servers.contains_key("server"));
    assert_eq!(saved.mcp_servers.get("other"), Some(&unrelated));
    assert_eq!(
        saved
            .mcp_servers
            .get("renamed")
            .and_then(|server| server.display_name.as_deref()),
        Some("saved")
    );
}

#[gpui::test]
fn mcp_fragment_cas_classifies_removed_and_occupied_entries(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let server = McpServerTomlConfig {
        command: Some("mcp".to_string()),
        ..Default::default()
    };
    let mut initial = JacoConfig::default();
    initial
        .mcp_servers
        .insert("occupied".to_string(), server.clone());
    let bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        let occupied =
            upsert_mcp_server_if_unchanged(cx, None, None, "occupied".to_string(), server.clone());
        assert!(matches!(
            occupied,
            Err(JacoError::ConfigEditConflict(
                crate::errors::ConfigEditConflict::IdOccupied { .. }
            ))
        ));

        let removed = upsert_mcp_server_if_unchanged(
            cx,
            Some("removed"),
            Some(&server),
            "removed".to_string(),
            server.clone(),
        );
        assert!(matches!(
            removed,
            Err(JacoError::ConfigEditConflict(
                crate::errors::ConfigEditConflict::Removed { .. }
            ))
        ));
    });

    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[gpui::test]
fn credential_cleanup_blocks_mcp_mutation_and_defers_external_config(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();
    let server = McpServerTomlConfig {
        transport: McpTransportKind::StreamableHttp,
        url: Some("https://example.com/mcp".to_string()),
        oauth: Some(McpOAuthTomlConfig::AuthorizationCodePkce {
            scopes: Vec::new(),
            client_id: None,
            client_metadata_url: None,
            resource: None,
            callback_port: None,
            callback_url: None,
        }),
        ..Default::default()
    };
    let key = crate::state::mcp::oauth::credentials_key_for_server("server-a", &server)
        .unwrap()
        .unwrap();

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        let observer = cx.new(|_| ConfigFileObserver {
            _binding: None,
            _config_subscription: None,
            probe_task: None,
            probe_basis_epoch: 0,
            pending_dirty: false,
        });
        cx.set_global(ConfigFileObserverGlobal {
            _observer: observer.clone(),
        });
        crate::state::mcp::oauth::schedule_credential_cleanup(
            vec![key],
            |result, _| {
                assert_eq!(result.failure_count, 0);
            },
            cx,
        );
        assert!(crate::state::mcp::oauth::credential_cleanup_in_progress(cx));
        observer
    });

    let mut external = JacoConfig::default();
    external
        .mcp_servers
        .insert("server-a".to_string(), server.clone());
    let external_bytes = toml::to_string_pretty(&external).unwrap().into_bytes();
    fs::write(&path, &external_bytes).unwrap();
    cx.update(|cx| {
        cx.set_global(ConfigProbeResultForTest(Ok(data_from_value(
            path.clone(),
            external,
            external_bytes,
        ))));
        observer.update(cx, |observer, cx| {
            observer.on_dirty(cx);
            assert!(observer.pending_dirty);
            assert!(observer.probe_task.is_none());
            observer.pending_dirty = false;
        });
        request_reload(cx);
        assert!(observer.read(cx).pending_dirty);
        let result = upsert_mcp_server_if_unchanged(cx, None, None, "server-b".to_string(), server);
        assert!(matches!(result, Err(JacoError::McpSubmissionInProgress)));
        assert!(store(cx).read(cx, |operation| {
            operation
                .data()
                .is_some_and(|data| data.mcp_servers.is_empty())
        }));
    });

    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!crate::state::mcp::oauth::credential_cleanup_in_progress(
            cx
        ));
        assert!(
            store(cx).read(cx, |operation| operation.data().is_some_and(|data| {
                data.mcp_servers.contains_key("server-a")
                    && !data.mcp_servers.contains_key("server-b")
            }))
        );
        assert!(!observer.read(cx).pending_dirty);
    });
}

#[gpui::test]
fn observed_same_bytes_do_not_publish_config(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    let bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();
    let deliveries = Rc::new(Cell::new(0));

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial.clone()).unwrap();
        let observed_deliveries = deliveries.clone();
        cx.new(|cx| ConfigFileObserver {
            _binding: None,
            _config_subscription: Some(store(cx).observe(cx, move |_, _, _| {
                observed_deliveries.set(observed_deliveries.get() + 1);
            })),
            probe_task: None,
            probe_basis_epoch: 0,
            pending_dirty: false,
        })
    });
    cx.run_until_parked();
    assert_eq!(deliveries.get(), 1);

    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: 0,
                    operation_phase: repair::Phase::Ready,
                    source_bytes: Some(bytes.clone()),
                },
                Ok(data_from_value(path, initial, bytes)),
                cx,
            );
        });
    });
    cx.run_until_parked();

    assert_eq!(deliveries.get(), 1);
}

#[gpui::test]
fn observed_invalid_config_retains_last_good_data(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig {
        app_settings: AppSettingsConfig {
            language: AppLanguage::Chinese,
            ..Default::default()
        },
        ..Default::default()
    };
    fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();

    cx.update(|cx| {
        install_for_test(cx, path.clone(), initial).unwrap();
        apply_observed_probe(
            Err(ConfigProblem::Parse {
                path,
                message: "invalid".to_string(),
            }),
            cx,
        );
    });
    cx.run_until_parked();

    cx.update(|cx| {
        store(cx).read(cx, |operation| {
            let ConfigOperation::Degraded(degraded) = operation else {
                panic!("expected degraded config");
            };
            assert_eq!(degraded.data().app_settings.language, AppLanguage::Chinese);
            assert!(matches!(degraded.problem(), ConfigProblem::Parse { .. }));
        });
    });
}

#[gpui::test]
fn observed_restored_last_good_bytes_recovers_degraded_config(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig {
        app_settings: AppSettingsConfig {
            language: AppLanguage::Chinese,
            ..Default::default()
        },
        ..Default::default()
    };
    let bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial.clone()).unwrap();
        cx.new(|_| ConfigFileObserver {
            _binding: None,
            _config_subscription: None,
            probe_task: None,
            probe_basis_epoch: 0,
            pending_dirty: false,
        })
    });

    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: 0,
                    operation_phase: repair::Phase::Ready,
                    source_bytes: Some(bytes.clone()),
                },
                Err(ConfigProblem::Parse {
                    path: path.clone(),
                    message: "invalid".to_string(),
                }),
                cx,
            );
        });
    });
    cx.run_until_parked();

    cx.update(|cx| {
        store(cx).read(cx, |operation| {
            assert!(matches!(operation, ConfigOperation::Degraded(_)));
        });
        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: 0,
                    operation_phase: repair::Phase::Degraded,
                    source_bytes: Some(bytes.clone()),
                },
                Ok(data_from_value(
                    path.clone(),
                    initial.clone(),
                    bytes.clone(),
                )),
                cx,
            );
        });
        store(cx).read(cx, |operation| {
            assert!(matches!(operation, ConfigOperation::RepairingDegraded(_)));
        });
    });
    cx.run_until_parked();

    cx.update(|cx| {
        store(cx).read(cx, |operation| {
            let ConfigOperation::Ready(ready) = operation else {
                panic!("expected restored config to be ready");
            };
            assert_eq!(ready.data().app_settings.language, AppLanguage::Chinese);
            assert_eq!(ready.data().source_bytes, bytes);
            assert!(operation.problem().is_none());
        });
    });
}

#[gpui::test]
fn observed_restored_source_retries_external_change_pending(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    let initial_bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &initial_bytes).unwrap();
    let pending_model = ChatFormModelConfig {
        provider_id: "provider-1".to_string(),
        model_id: "gpt-5".to_string(),
    };
    let mut pending_value = initial.clone();
    pending_value.chat_form.model = Some(pending_model.clone());
    let pending_bytes = toml::to_string_pretty(&pending_value).unwrap().into_bytes();

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial.clone()).unwrap();
        let observer = cx.new(|cx| {
            let mut observer = ConfigFileObserver {
                _binding: None,
                _config_subscription: None,
                probe_task: None,
                probe_basis_epoch: 0,
                pending_dirty: false,
            };
            observer._config_subscription = Some(store(cx).observe(
                cx,
                |observer: &mut ConfigFileObserver, operation, cx| {
                    observer.on_config_operation_changed(operation, cx);
                },
            ));
            observer
        });
        cx.set_global(ConfigFileObserverGlobal {
            _observer: observer.clone(),
        });
        observer
    });
    cx.run_until_parked();

    fs::write(&path, b"# external edit\n").unwrap();
    cx.update(|cx| {
        let result = update_chat_form_config(cx, |config| {
            config.model = Some(pending_model);
        });
        assert!(result.is_err());
        assert!(store(cx).read(cx, |operation| matches!(
            operation,
            ConfigOperation::Degraded(degraded)
                if matches!(degraded.problem(), ConfigProblem::ExternalChange { .. })
        )));
    });

    fs::write(&path, &initial_bytes).unwrap();
    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: observer.probe_basis_epoch,
                    operation_phase: repair::Phase::Degraded,
                    source_bytes: Some(initial_bytes.clone()),
                },
                Ok(data_from_value(
                    path.clone(),
                    initial.clone(),
                    initial_bytes.clone(),
                )),
                cx,
            );
        });
        store(cx).read(cx, |operation| {
            assert!(matches!(operation, ConfigOperation::RepairingDegraded(_)));
            assert_eq!(operation.active_repair(), Some(&ConfigRepair::RetryWrite));
        });
    });
    cx.run_until_parked();

    cx.update(|cx| {
        store(cx).read(cx, |operation| {
            let ConfigOperation::Ready(ready) = operation else {
                panic!("expected pending config to be committed");
            };
            assert_eq!(ready.data().value, pending_value);
            assert_eq!(ready.data().source_bytes, pending_bytes);
        });
    });
    assert_eq!(fs::read(path).unwrap(), pending_bytes);
}

#[gpui::test]
fn observed_external_change_retry_preserves_pending_after_another_race(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    let initial_bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &initial_bytes).unwrap();
    let pending_model = ChatFormModelConfig {
        provider_id: "provider-1".to_string(),
        model_id: "gpt-5".to_string(),
    };

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial.clone()).unwrap();
        let observer = cx.new(|cx| {
            let mut observer = ConfigFileObserver {
                _binding: None,
                _config_subscription: None,
                probe_task: None,
                probe_basis_epoch: 0,
                pending_dirty: false,
            };
            observer._config_subscription = Some(store(cx).observe(
                cx,
                |observer: &mut ConfigFileObserver, operation, cx| {
                    observer.on_config_operation_changed(operation, cx);
                },
            ));
            observer
        });
        cx.set_global(ConfigFileObserverGlobal {
            _observer: observer.clone(),
        });
        observer
    });
    cx.run_until_parked();

    fs::write(&path, b"# first external edit\n").unwrap();
    cx.update(|cx| {
        assert!(
            update_chat_form_config(cx, |config| {
                config.model = Some(pending_model.clone());
            })
            .is_err()
        );
    });

    fs::write(&path, &initial_bytes).unwrap();
    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: observer.probe_basis_epoch,
                    operation_phase: repair::Phase::Degraded,
                    source_bytes: Some(initial_bytes.clone()),
                },
                Ok(data_from_value(
                    path.clone(),
                    initial.clone(),
                    initial_bytes.clone(),
                )),
                cx,
            );
        });
        assert!(store(cx).read(cx, |operation| matches!(
            operation,
            ConfigOperation::RepairingDegraded(_)
        )));
    });

    let raced_bytes = b"# second external edit\n".to_vec();
    fs::write(&path, &raced_bytes).unwrap();
    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            observer.on_dirty(cx);
            assert!(observer.pending_dirty);
        });
    });
    cx.run_until_parked();

    cx.update(|cx| {
        observer.update(cx, |observer, cx| {
            assert!(!observer.pending_dirty);
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: observer.probe_basis_epoch,
                    operation_phase: repair::Phase::Degraded,
                    source_bytes: Some(initial_bytes.clone()),
                },
                Ok(data_from_value(
                    path.clone(),
                    initial.clone(),
                    raced_bytes.clone(),
                )),
                cx,
            );
            observer.finish_probe(
                ConfigProbeStart {
                    basis_epoch: observer.probe_basis_epoch,
                    operation_phase: repair::Phase::Degraded,
                    source_bytes: Some(initial_bytes.clone()),
                },
                Err(ConfigProblem::Parse {
                    path: path.clone(),
                    message: "invalid external edit".to_string(),
                }),
                cx,
            );
        });
        store(cx).read(cx, |operation| {
            let ConfigOperation::Degraded(degraded) = operation else {
                panic!("expected the second conflict to remain degraded");
            };
            assert_eq!(degraded.data().source_bytes, initial_bytes);
            let ConfigProblem::ExternalChange { pending, .. } = degraded.problem() else {
                panic!("expected another external-change problem");
            };
            assert_eq!(pending.data.chat_form.model, Some(pending_model));
        });
    });
    assert_eq!(fs::read(path).unwrap(), raced_bytes);
}

#[gpui::test]
fn stale_probe_cannot_clear_failed_save_problem(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let initial = JacoConfig::default();
    let bytes = toml::to_string_pretty(&initial).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();

    let observer = cx.update(|cx| {
        install_for_test(cx, path.clone(), initial.clone()).unwrap();
        let observer = cx.new(|_| ConfigFileObserver {
            _binding: None,
            _config_subscription: None,
            probe_task: None,
            probe_basis_epoch: 0,
            pending_dirty: false,
        });
        cx.set_global(ConfigFileObserverGlobal {
            _observer: observer.clone(),
        });
        observer
    });
    let stale_start = ConfigProbeStart {
        basis_epoch: 0,
        operation_phase: repair::Phase::Ready,
        source_bytes: Some(bytes.clone()),
    };
    let _lock = persistence::FileLock::acquire(&path.with_extension("toml.lock")).unwrap();

    cx.update(|cx| {
        let result = update_chat_form_config(cx, |config| {
            config.model = Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            });
        });
        assert!(result.is_err());

        observer.update(cx, |observer, cx| {
            observer.finish_probe(
                stale_start,
                Ok(data_from_value(path.clone(), initial, bytes.clone())),
                cx,
            );
        });
    });
    cx.run_until_parked();

    cx.update(|cx| {
        store(cx).read(cx, |operation| {
            let ConfigOperation::Degraded(degraded) = operation else {
                panic!("expected failed save to remain degraded");
            };
            let ConfigProblem::Locked { pending, .. } = degraded.problem() else {
                panic!("expected failed save problem to be retained");
            };
            assert_eq!(degraded.data().source_bytes, bytes);
            assert_eq!(
                pending
                    .data
                    .chat_form
                    .model
                    .as_ref()
                    .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
                Some(("provider-1", "gpt-5"))
            );
        });
    });
}

#[gpui::test]
fn observer_shutdown_drops_owned_task_and_subscription(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let observer = cx.new(|_| ConfigFileObserver {
            _binding: None,
            _config_subscription: Some(Subscription::new(|| {})),
            probe_task: Some(Task::ready(())),
            probe_basis_epoch: 0,
            pending_dirty: true,
        });
        cx.set_global(ConfigFileObserverGlobal {
            _observer: observer.clone(),
        });

        shutdown_file_observer(cx);

        let observer = observer.read(cx);
        assert!(observer._config_subscription.is_none());
        assert!(observer.probe_task.is_none());
        assert!(!observer.pending_dirty);
    });
}
