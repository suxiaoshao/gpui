use super::{
    AiChat2AppSettings, AiChat2Config, AppSettingsConfig, ChatFormConfig, ChatFormModelConfig,
    McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind, StorageConfig,
    delete_mcp_server, install_for_test, set_mcp_server_enabled, update_app_settings,
    update_chat_form_config, upsert_mcp_server,
};
use ai_chat_agent::McpServerTransport;
use ai_chat_core::{
    AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings, ReasoningSelectionSnapshot,
    TokenBudgetSelectionMode, ToolApprovalMode,
};
use gpui::TestAppContext;
use std::{ffi::OsString, path::PathBuf};

use crate::state::config::override_dir_from_value;

#[test]
fn toml_config_stores_storage_and_app_settings() {
    let config = AiChat2Config {
        storage: StorageConfig {
            data_dir: Some(PathBuf::from("/tmp/ai-chat2")),
        },
        app_settings: AppSettingsPayload {
            language: AppLanguage::Chinese,
            theme: AppThemeSettings {
                mode: AppThemeMode::Light,
                light_theme: Some("preset:Default Light".to_string()),
                dark_theme: Some("preset:Default Dark".to_string()),
                custom_theme_colors: vec!["#3271AE".to_string()],
            },
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            http_proxy: Some("http://127.0.0.1:8080".to_string()),
            default_project_id: Some("project-1".to_string()),
        }
        .into(),
        chat_form: ChatFormConfig {
            model: Some(ChatFormModelConfig {
                provider_id: "provider-1".to_string(),
                model_id: "gpt-5".to_string(),
            }),
            reasoning_selection: Some(ReasoningSelectionSnapshot::TokenBudget {
                mode: TokenBudgetSelectionMode::Custom,
                value: Some(2048),
            }),
            approval_mode: ToolApprovalMode::FullAccess,
        },
        ..Default::default()
    };

    let serialized = toml::to_string(&config).unwrap();

    assert!(serialized.contains("[storage]"));
    assert!(serialized.contains("data_dir"));
    assert!(serialized.contains("[app_settings]"));
    assert!(serialized.contains(r#"language = "zh-CN""#));
    assert!(serialized.contains(r#"temporary_hotkey = "cmd+shift+j""#));
    assert!(serialized.contains("[app_settings.theme]"));
    assert!(serialized.contains("custom_theme_colors"));
    assert!(serialized.contains("[chat_form]"));
    assert!(serialized.contains(r#"approval_mode = "full_access""#));
    assert!(serialized.contains("[chat_form.model]"));
    assert!(serialized.contains(r#"provider_id = "provider-1""#));
    assert!(serialized.contains("[chat_form.reasoning_selection]"));
    assert!(serialized.contains(r#"type = "tokenBudget""#));
    assert_eq!(
        toml::from_str::<AiChat2Config>(&serialized).unwrap(),
        config
    );
}

#[test]
fn toml_config_ignores_unknown_fields_for_compatibility() {
    let config = toml::from_str::<AiChat2Config>(
        r#"
unknown_top_level = true

[storage]
data_dir = "/tmp/ai-chat2"
unknown_storage = "kept by newer app"

[app_settings]
language = "zh-CN"
unknown_app_setting = true

[app_settings.theme]
mode = "dark"
unknown_theme_setting = "kept by newer app"

[chat_form]
approval_mode = "auto_approve"
unknown_chat_form_setting = "kept by newer app"

[chat_form.model]
provider_id = "provider-1"
model_id = "gpt-5"
unknown_model_setting = true
"#,
    )
    .unwrap();

    assert_eq!(
        config.storage,
        StorageConfig {
            data_dir: Some(PathBuf::from("/tmp/ai-chat2")),
        }
    );
    assert_eq!(config.app_settings.language, AppLanguage::Chinese);
    assert_eq!(config.app_settings.theme.mode, AppThemeMode::Dark);
    assert_eq!(
        config.chat_form.approval_mode,
        ToolApprovalMode::AutoApprove
    );
    assert_eq!(
        config
            .chat_form
            .model
            .as_ref()
            .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
        Some(("provider-1", "gpt-5"))
    );
}

#[test]
fn new_toml_config_writes_default_app_settings() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");

    let config = AiChat2Config::load_or_create_from_path(&path).expect("create config");

    assert_eq!(config, AiChat2Config::default());
    let source = std::fs::read_to_string(&path).expect("read config");
    assert!(source.contains("[app_settings]"));
    assert!(source.contains(r#"language = "system""#));
    assert!(source.contains("[app_settings.theme]"));
    assert!(source.contains(r#"mode = "system""#));
    assert!(source.contains("[chat_form]"));
    assert!(source.contains(r#"approval_mode = "request_approval""#));
}

#[test]
fn config_directory_override_ignores_empty_env_value() {
    assert_eq!(override_dir_from_value(None), None);
    assert_eq!(override_dir_from_value(Some(OsString::new())), None);
    assert_eq!(
        override_dir_from_value(Some(OsString::from("/tmp/ai-chat2-config"))),
        Some(PathBuf::from("/tmp/ai-chat2-config"))
    );
}

#[test]
fn malformed_toml_config_preserves_file_and_returns_diagnostic_default() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "not = [valid").expect("write malformed config");

    let config = AiChat2Config::load_or_create_from_path(&path).expect("load fallback config");

    assert_eq!(config.storage, StorageConfig::default());
    assert_eq!(config.app_settings, AppSettingsConfig::default());
    assert_eq!(config.chat_form, ChatFormConfig::default());
    assert!(config.mcp_servers.is_empty());
    let load_error = config.load_error.expect("load error");
    assert_eq!(load_error.path_display(), path.display().to_string());
    assert!(!load_error.message().is_empty());
    let source = std::fs::read_to_string(&path).expect("read preserved config");
    assert_eq!(source, "not = [valid");
}

#[gpui::test]
fn malformed_toml_config_blocks_later_config_writes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");
    let malformed_source = "not = [valid";
    std::fs::write(&path, malformed_source).expect("write malformed config");
    let config = AiChat2Config::load_or_create_from_path(&path).expect("load fallback config");

    let (settings_error, chat_form_error) = cx.update(|cx| {
        install_for_test(cx, config).expect("install config store");
        let settings_error = update_app_settings(cx, |payload| {
            payload.http_proxy = Some("http://127.0.0.1:8080".to_string());
        })
        .expect_err("settings update should fail")
        .to_string();
        let chat_form_error = update_chat_form_config(cx, |config| {
            config.approval_mode = ToolApprovalMode::FullAccess;
        })
        .expect_err("chat form update should fail")
        .to_string();
        (settings_error, chat_form_error)
    });

    assert!(settings_error.contains("config.toml is invalid"));
    assert!(chat_form_error.contains("config.toml is invalid"));
    let source = std::fs::read_to_string(&path).expect("read preserved config");
    assert_eq!(source, malformed_source);
}

#[gpui::test]
fn mcp_server_crud_helpers_persist_config_toml(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");
    let config = AiChat2Config {
        config_path: Some(path.clone()),
        ..Default::default()
    };
    config.save_for_test().expect("save config");

    cx.update(|cx| {
        install_for_test(cx, config).expect("install config store");
        upsert_mcp_server(
            cx,
            None,
            "filesystem".to_string(),
            McpServerTomlConfig {
                display_name: Some("Filesystem".to_string()),
                command: Some("npx".to_string()),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ],
                default_tools_approval_mode: Some(McpToolApprovalMode::Prompt),
                ..Default::default()
            },
        )
        .expect("create mcp server");

        let duplicate = upsert_mcp_server(
            cx,
            None,
            "filesystem".to_string(),
            McpServerTomlConfig {
                command: Some("uvx".to_string()),
                ..Default::default()
            },
        )
        .expect_err("duplicate server id should fail")
        .to_string();
        assert!(duplicate.contains("already exists"));
    });

    let stored =
        AiChat2Config::load_or_create_from_path(&path).expect("load stored config after create");
    let filesystem = stored
        .mcp_servers
        .get("filesystem")
        .expect("filesystem server is stored");
    assert_eq!(filesystem.display_name.as_deref(), Some("Filesystem"));
    assert_eq!(filesystem.command.as_deref(), Some("npx"));
    assert_eq!(
        filesystem.default_tools_approval_mode,
        Some(McpToolApprovalMode::Prompt)
    );

    cx.update(|cx| {
        upsert_mcp_server(
            cx,
            Some("filesystem"),
            "filesystem".to_string(),
            McpServerTomlConfig {
                display_name: Some("Docs".to_string()),
                transport: McpTransportKind::StreamableHttp,
                url: Some("https://docs.example.com/mcp".to_string()),
                default_tools_approval_mode: Some(McpToolApprovalMode::Auto),
                ..Default::default()
            },
        )
        .expect("edit mcp server");
        set_mcp_server_enabled(cx, "filesystem", false).expect("disable mcp server");
    });

    let stored =
        AiChat2Config::load_or_create_from_path(&path).expect("load stored config after edit");
    let filesystem = stored
        .mcp_servers
        .get("filesystem")
        .expect("filesystem server is stored");
    assert!(!filesystem.enabled);
    assert_eq!(filesystem.display_name.as_deref(), Some("Docs"));
    assert_eq!(filesystem.transport, McpTransportKind::StreamableHttp);
    assert_eq!(
        filesystem.url.as_deref(),
        Some("https://docs.example.com/mcp")
    );
    assert_eq!(
        filesystem.default_tools_approval_mode,
        Some(McpToolApprovalMode::Auto)
    );

    cx.update(|cx| {
        assert!(delete_mcp_server(cx, "filesystem").expect("delete mcp server"));
    });

    let stored =
        AiChat2Config::load_or_create_from_path(&path).expect("load stored config after delete");
    assert!(stored.mcp_servers.is_empty());
}

#[test]
fn app_settings_expose_typed_config_preferences() {
    let settings = AiChat2AppSettings::new(AppSettingsPayload {
        language: AppLanguage::Chinese,
        theme: AppThemeSettings {
            mode: AppThemeMode::Light,
            ..Default::default()
        },
        temporary_hotkey: Some("cmd+shift+j".to_string()),
        http_proxy: Some("http://127.0.0.1:8080".to_string()),
        default_project_id: Some("project-1".to_string()),
    });

    assert_eq!(settings.language(), AppLanguage::Chinese);
    assert_eq!(settings.theme().mode, AppThemeMode::Light);
    assert_eq!(settings.temporary_hotkey(), Some("cmd+shift+j"));
    assert_eq!(settings.http_proxy(), Some("http://127.0.0.1:8080"));
    assert_eq!(
        settings.default_project_id().map(String::as_str),
        Some("project-1")
    );
}

#[test]
fn mcp_config_layer_filters_disabled_servers_and_maps_transports() {
    let config = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.filesystem]
enabled = true
required = true
display_name = "Filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
cwd = "/tmp"
startup_timeout_ms = 30000
tool_timeout_ms = 300000
env_vars = []
enabled_tools = ["read_file"]
disabled_tools = ["delete_file"]
default_tools_approval_mode = "prompt"

[mcp_servers.filesystem.env]
NODE_ENV = "production"

[mcp_servers.filesystem.tools.read_file]
approval_mode = "auto"

[mcp_servers.linear]
enabled = false
transport = "streamable_http"
url = "https://example.com/mcp"

[mcp_servers.linear.headers]
Authorization = "Bearer ${LINEAR_MCP_TOKEN}"

[mcp_servers.docs]
transport = "streamable_http"
url = "https://docs.example.com/mcp"

[mcp_servers.docs.headers]
X-Docs = "enabled"
"#,
    )
    .unwrap();

    let layer = config.mcp_config_layer().unwrap();

    assert_eq!(layer.servers.len(), 2);
    assert_eq!(layer.servers[0].server_id, "docs");
    assert_eq!(layer.servers[0].display_name, None);
    match &layer.servers[0].transport {
        McpServerTransport::StreamableHttp(http) => {
            assert_eq!(http.url, "https://docs.example.com/mcp");
            assert_eq!(
                http.headers.get("X-Docs").map(String::as_str),
                Some("enabled")
            );
            assert!(!http.headers.contains_key("Authorization"));
        }
        McpServerTransport::Stdio(_) => panic!("expected streamable HTTP transport"),
    }
    assert_eq!(layer.servers[1].server_id, "filesystem");
    assert_eq!(layer.servers[1].display_name.as_deref(), Some("Filesystem"));
    assert_eq!(
        layer.servers[1].env.get("NODE_ENV").map(String::as_str),
        Some("production")
    );
    match &layer.servers[1].transport {
        McpServerTransport::Stdio(stdio) => {
            assert_eq!(stdio.command, "npx");
            assert_eq!(stdio.args.len(), 3);
        }
        McpServerTransport::StreamableHttp(_) => panic!("expected stdio transport"),
    }
}

#[test]
fn mcp_config_ignores_stdio_env_for_http_servers() {
    let config = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.docs]
transport = "streamable_http"
url = "https://docs.example.com/mcp"
env_vars = ["AI_CHAT2_HTTP_STDIO_ENV_SHOULD_NOT_BE_READ"]
cwd = "/tmp/stdio-only"

[mcp_servers.docs.env]
NODE_ENV = "production"
"#,
    )
    .unwrap();

    let layer = config.mcp_config_layer().unwrap();

    assert_eq!(layer.servers.len(), 1);
    assert!(layer.servers[0].env.is_empty());
    assert!(layer.servers[0].cwd.is_none());
    match &layer.servers[0].transport {
        McpServerTransport::StreamableHttp(http) => {
            assert_eq!(http.url, "https://docs.example.com/mcp");
        }
        McpServerTransport::Stdio(_) => panic!("expected streamable HTTP transport"),
    }
}

#[test]
fn mcp_config_rejects_reserved_headers_and_oauth_authorization_header() {
    let reserved = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.bad]
transport = "streamable_http"
url = "https://example.com/mcp"

[mcp_servers.bad.headers]
Mcp-Session-Id = "session"
"#,
    )
    .unwrap();
    let err = reserved.mcp_config_layer().unwrap_err().to_string();
    assert!(err.contains("reserved"));

    let authorization = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.bad]
transport = "streamable_http"
url = "https://example.com/mcp"

[mcp_servers.bad.headers]
Authorization = "Bearer literal"

[mcp_servers.bad.oauth]
flow = "authorization_code_pkce"
"#,
    )
    .unwrap();
    let err = authorization.mcp_config_layer().unwrap_err().to_string();
    assert!(err.contains("Authorization"));

    let bearer_token = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.bad]
transport = "streamable_http"
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"

[mcp_servers.bad.oauth]
flow = "authorization_code_pkce"
"#,
    )
    .unwrap();
    let err = bearer_token.mcp_config_layer().unwrap_err().to_string();
    assert!(err.contains("bearer_token_env_var"));
}

#[test]
fn mcp_config_rejects_client_credentials_flow() {
    let config = toml::from_str::<AiChat2Config>(
        r#"
[mcp_servers.internal]
transport = "streamable_http"
url = "https://internal.example.com/mcp"

[mcp_servers.internal.oauth]
flow = "client_credentials"
client_id = "ai-chat2"
client_secret_env_var = "INTERNAL_MCP_CLIENT_SECRET"
scopes = ["tools"]
resource = "https://internal.example.com/mcp"
"#,
    )
    .unwrap();

    assert_eq!(
        config
            .mcp_servers
            .get("internal")
            .and_then(|server| server.oauth.as_ref()),
        Some(&McpOAuthTomlConfig::ClientCredentials {
            client_id: "ai-chat2".to_string(),
            client_secret_env_var: "INTERNAL_MCP_CLIENT_SECRET".to_string(),
            scopes: vec!["tools".to_string()],
            resource: Some("https://internal.example.com/mcp".to_string()),
        })
    );

    let err = config.mcp_config_layer().unwrap_err().to_string();
    assert!(err.contains("client_credentials"));
    assert!(err.contains("not supported"));
}

#[gpui::test]
fn noop_app_settings_update_does_not_persist(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");
    let payload = AppSettingsPayload::default();
    let config = AiChat2Config::with_app_settings_for_test(path.clone(), payload.clone());
    config.save_for_test().expect("save config");
    let before = std::fs::read_to_string(&path).expect("read config before");

    let result = cx.update(|cx| {
        install_for_test(cx, config).expect("install config store");
        update_app_settings(cx, |_| {}).expect("update settings")
    });

    let after = std::fs::read_to_string(&path).expect("read config after");
    assert_eq!(result, payload);
    assert_eq!(after, before);
}

#[gpui::test]
fn noop_chat_form_update_does_not_persist(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.toml");
    let mut config = AiChat2Config {
        config_path: Some(path.clone()),
        ..Default::default()
    };
    config.chat_form.approval_mode = ToolApprovalMode::FullAccess;
    config.save_for_test().expect("save config");
    let before = std::fs::read_to_string(&path).expect("read config before");

    let chat_form = cx.update(|cx| {
        install_for_test(cx, config).expect("install config store");
        update_chat_form_config(cx, |config| {
            config.approval_mode = ToolApprovalMode::FullAccess;
        })
        .expect("update chat form")
    });

    let after = std::fs::read_to_string(&path).expect("read config after");
    assert_eq!(chat_form.approval_mode, ToolApprovalMode::FullAccess);
    assert_eq!(after, before);
}
