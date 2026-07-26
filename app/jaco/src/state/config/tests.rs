use super::*;

#[test]
fn toml_config_preserves_public_field_shape() {
    let config = JacoConfig {
        storage: StorageConfig {
            data_dir: Some(PathBuf::from("data")),
        },
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
    assert!(source.contains("[storage]"));
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
    let path = dir.path().join("config.toml");

    let data = load_for_operation(&path).unwrap();

    assert_eq!(&data.value, &JacoConfig::default());
    assert_eq!(data.source_bytes, fs::read(&path).unwrap());
    assert!(!data.source_bytes.is_empty());
}

#[test]
fn relative_data_dir_uses_config_parent_without_canonicalize() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let value = JacoConfig {
        storage: StorageConfig {
            data_dir: Some(PathBuf::from("nested/../database")),
        },
        ..Default::default()
    };
    let bytes = toml::to_string_pretty(&value).unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();

    let data = load_for_operation(&path).unwrap();

    assert_eq!(data.data_dir, dir.path().join("database"));
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
        data: data_from_value(path.clone(), value, bytes.clone()).unwrap(),
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
