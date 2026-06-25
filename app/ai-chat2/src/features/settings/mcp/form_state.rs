use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    foundation::I18n,
    state::config::{McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind},
};
use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::input::InputState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StringListField {
    Args,
    EnvVars,
}

impl StringListField {
    fn placeholder_key(self) -> &'static str {
        match self {
            Self::Args => "mcp-placeholder-arg",
            Self::EnvVars => "mcp-placeholder-env-var",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyValueField {
    Env,
    Headers,
    EnvHeaders,
}

impl KeyValueField {
    fn placeholder_keys(self) -> (&'static str, &'static str) {
        match self {
            Self::Env => ("mcp-placeholder-env-key", "mcp-placeholder-env-value"),
            Self::Headers => (
                "mcp-placeholder-header-name",
                "mcp-placeholder-header-value",
            ),
            Self::EnvHeaders => (
                "mcp-placeholder-header-name",
                "mcp-placeholder-env-header-var",
            ),
        }
    }
}

#[derive(Clone)]
pub(super) struct StringListDraftRow {
    pub(super) id: u64,
    pub(super) input: Entity<InputState>,
}

#[derive(Clone)]
pub(super) struct KeyValueDraftRow {
    pub(super) id: u64,
    pub(super) key_input: Entity<InputState>,
    pub(super) value_input: Entity<InputState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StringRowValue {
    pub(super) id: u64,
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeyValueRowValue {
    pub(super) id: u64,
    pub(super) key: String,
    pub(super) value: String,
}

pub(super) struct McpServerFormDraft {
    pub(super) transport: McpTransportKind,
    pub(super) server_id_input: Entity<InputState>,
    pub(super) command_input: Entity<InputState>,
    pub(super) cwd_input: Entity<InputState>,
    pub(super) args: Vec<StringListDraftRow>,
    pub(super) env: Vec<KeyValueDraftRow>,
    pub(super) env_vars: Vec<StringListDraftRow>,
    pub(super) url_input: Entity<InputState>,
    pub(super) bearer_token_env_var_input: Entity<InputState>,
    pub(super) headers: Vec<KeyValueDraftRow>,
    pub(super) env_headers: Vec<KeyValueDraftRow>,
    pub(super) oauth_enabled: bool,
}

impl McpServerFormDraft {
    pub(super) fn from_config(
        server_id: String,
        server: &McpServerTomlConfig,
        next_row_id: &mut u64,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let cwd = server
            .cwd
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            transport: server.transport,
            server_id_input: single_line_input(server_id, "mcp-field-name", window, cx),
            command_input: single_line_input(
                server.command.clone().unwrap_or_default(),
                "mcp-field-command",
                window,
                cx,
            ),
            cwd_input: single_line_input(cwd, "mcp-field-cwd", window, cx),
            args: string_rows(
                server.args.iter().cloned(),
                StringListField::Args,
                next_row_id,
                window,
                cx,
            ),
            env: key_value_rows(
                server
                    .env
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
                KeyValueField::Env,
                next_row_id,
                window,
                cx,
            ),
            env_vars: string_rows(
                server.env_vars.iter().cloned(),
                StringListField::EnvVars,
                next_row_id,
                window,
                cx,
            ),
            url_input: single_line_input(
                server.url.clone().unwrap_or_default(),
                "mcp-field-url",
                window,
                cx,
            ),
            bearer_token_env_var_input: single_line_input(
                server.bearer_token_env_var.clone().unwrap_or_default(),
                "mcp-field-bearer-token-env-var",
                window,
                cx,
            ),
            headers: key_value_rows(
                server
                    .headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
                KeyValueField::Headers,
                next_row_id,
                window,
                cx,
            ),
            env_headers: key_value_rows(
                server
                    .env_headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
                KeyValueField::EnvHeaders,
                next_row_id,
                window,
                cx,
            ),
            oauth_enabled: server.oauth.is_some(),
        }
    }

    pub(super) fn server_id(&self, original_server_id: Option<&str>, cx: &App) -> String {
        original_server_id
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| trim_input(&self.server_id_input, cx))
    }

    pub(super) fn merge_into_config(
        &self,
        original_config: Option<&McpServerTomlConfig>,
        cx: &App,
    ) -> McpServerTomlConfig {
        let mut server = original_config.cloned().unwrap_or_default();
        server.transport = self.transport;

        match self.transport {
            McpTransportKind::Stdio => {
                server.command = optional_input(&self.command_input, cx);
                server.args = string_values(&self.args, cx)
                    .into_iter()
                    .filter_map(|row| (!row.value.is_empty()).then_some(row.value))
                    .collect();
                server.env = key_value_map(&self.env, true, cx);
                server.env_vars = string_values(&self.env_vars, cx)
                    .into_iter()
                    .filter_map(|row| (!row.value.is_empty()).then_some(row.value))
                    .collect();
                server.cwd = optional_input(&self.cwd_input, cx).map(PathBuf::from);
                server.oauth = None;
            }
            McpTransportKind::StreamableHttp => {
                server.url = optional_input(&self.url_input, cx);
                server.bearer_token_env_var = optional_input(&self.bearer_token_env_var_input, cx);
                server.headers = key_value_map(&self.headers, false, cx);
                server.env_headers = key_value_map(&self.env_headers, false, cx);
                server.oauth = self.oauth_enabled.then(|| {
                    server.oauth.clone().unwrap_or_else(|| {
                        McpOAuthTomlConfig::AuthorizationCodePkce {
                            scopes: Vec::new(),
                            client_id: None,
                            client_metadata_url: None,
                            resource: None,
                            callback_port: None,
                            callback_url: None,
                        }
                    })
                });
            }
        }

        server
    }

    pub(super) fn set_oauth_enabled(&mut self, enabled: bool) {
        self.oauth_enabled = enabled;
    }

    pub(super) fn add_string_row(
        &mut self,
        field: StringListField,
        next_row_id: &mut u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let row = string_row(String::new(), field, next_row_id, window, cx);
        self.string_rows_mut(field).push(row);
    }

    pub(super) fn remove_string_row(
        &mut self,
        field: StringListField,
        row_id: u64,
        next_row_id: &mut u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rows = self.string_rows_mut(field);
        rows.retain(|row| row.id != row_id);
        if rows.is_empty() {
            rows.push(string_row(String::new(), field, next_row_id, window, cx));
        }
    }

    pub(super) fn add_key_value_row(
        &mut self,
        field: KeyValueField,
        next_row_id: &mut u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let row = key_value_row(String::new(), String::new(), field, next_row_id, window, cx);
        self.key_value_rows_mut(field).push(row);
    }

    pub(super) fn remove_key_value_row(
        &mut self,
        field: KeyValueField,
        row_id: u64,
        next_row_id: &mut u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rows = self.key_value_rows_mut(field);
        rows.retain(|row| row.id != row_id);
        if rows.is_empty() {
            rows.push(key_value_row(
                String::new(),
                String::new(),
                field,
                next_row_id,
                window,
                cx,
            ));
        }
    }

    fn string_rows_mut(&mut self, field: StringListField) -> &mut Vec<StringListDraftRow> {
        match field {
            StringListField::Args => &mut self.args,
            StringListField::EnvVars => &mut self.env_vars,
        }
    }

    fn key_value_rows_mut(&mut self, field: KeyValueField) -> &mut Vec<KeyValueDraftRow> {
        match field {
            KeyValueField::Env => &mut self.env,
            KeyValueField::Headers => &mut self.headers,
            KeyValueField::EnvHeaders => &mut self.env_headers,
        }
    }
}

pub(super) fn trim_input(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_string()
}

pub(super) fn optional_input(input: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = trim_input(input, cx);
    (!value.is_empty()).then_some(value)
}

pub(super) fn string_values(rows: &[StringListDraftRow], cx: &App) -> Vec<StringRowValue> {
    rows.iter()
        .map(|row| StringRowValue {
            id: row.id,
            value: trim_input(&row.input, cx),
        })
        .collect()
}

pub(super) fn key_value_values(rows: &[KeyValueDraftRow], cx: &App) -> Vec<KeyValueRowValue> {
    rows.iter()
        .map(|row| KeyValueRowValue {
            id: row.id,
            key: trim_input(&row.key_input, cx),
            value: trim_input(&row.value_input, cx),
        })
        .collect()
}

fn string_rows(
    values: impl Iterator<Item = String>,
    field: StringListField,
    next_row_id: &mut u64,
    window: &mut Window,
    cx: &mut App,
) -> Vec<StringListDraftRow> {
    let mut rows = values
        .map(|value| string_row(value, field, next_row_id, window, cx))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(string_row(String::new(), field, next_row_id, window, cx));
    }
    rows
}

fn string_row(
    value: String,
    field: StringListField,
    next_row_id: &mut u64,
    window: &mut Window,
    cx: &mut App,
) -> StringListDraftRow {
    StringListDraftRow {
        id: take_row_id(next_row_id),
        input: single_line_input(value, field.placeholder_key(), window, cx),
    }
}

fn key_value_rows(
    values: impl Iterator<Item = (String, String)>,
    field: KeyValueField,
    next_row_id: &mut u64,
    window: &mut Window,
    cx: &mut App,
) -> Vec<KeyValueDraftRow> {
    let mut rows = values
        .map(|(key, value)| key_value_row(key, value, field, next_row_id, window, cx))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(key_value_row(
            String::new(),
            String::new(),
            field,
            next_row_id,
            window,
            cx,
        ));
    }
    rows
}

fn key_value_row(
    key: String,
    value: String,
    field: KeyValueField,
    next_row_id: &mut u64,
    window: &mut Window,
    cx: &mut App,
) -> KeyValueDraftRow {
    let (key_placeholder, value_placeholder) = field.placeholder_keys();
    KeyValueDraftRow {
        id: take_row_id(next_row_id),
        key_input: single_line_input(key, key_placeholder, window, cx),
        value_input: single_line_input(value, value_placeholder, window, cx),
    }
}

fn key_value_map(
    rows: &[KeyValueDraftRow],
    allow_empty_values: bool,
    cx: &App,
) -> BTreeMap<String, String> {
    key_value_values(rows, cx)
        .into_iter()
        .filter_map(|row| {
            if row.key.is_empty() {
                return None;
            }
            if row.value.is_empty() && !allow_empty_values {
                return None;
            }
            Some((row.key, row.value))
        })
        .collect()
}

fn single_line_input(
    value: String,
    placeholder_key: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(cx.global::<I18n>().t(placeholder_key))
            .default_value(value)
    })
}

fn take_row_id(next_row_id: &mut u64) -> u64 {
    let id = *next_row_id;
    *next_row_id += 1;
    id
}

#[cfg(test)]
mod tests {
    use super::McpServerFormDraft;
    use crate::{
        foundation,
        state::config::{McpServerTomlConfig, McpToolApprovalMode, McpTransportKind},
    };
    use gpui::{
        AppContext as _, IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div,
    };

    #[gpui::test]
    fn merge_preserves_hidden_fields_when_editing_stdio(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            let mut original = McpServerTomlConfig {
                enabled: false,
                required: true,
                display_name: Some("Filesystem".to_string()),
                transport: McpTransportKind::Stdio,
                command: Some("old-command".to_string()),
                args: vec!["--old".to_string()],
                startup_timeout_ms: Some(10),
                tool_timeout_ms: Some(20),
                enabled_tools: Some(vec!["read".to_string()]),
                disabled_tools: vec!["write".to_string()],
                default_tools_approval_mode: Some(McpToolApprovalMode::Deny),
                ..Default::default()
            };
            original.url = Some("https://example.com/mcp".to_string());
            original.bearer_token_env_var = Some("MCP_TOKEN".to_string());

            let mut next_row_id = 1;
            let draft = McpServerFormDraft::from_config(
                "filesystem".to_string(),
                &original,
                &mut next_row_id,
                window,
                cx,
            );
            draft
                .command_input
                .update(cx, |input, cx| input.set_value("new-command", window, cx));
            draft.args[0]
                .input
                .update(cx, |input, cx| input.set_value("--new", window, cx));

            let merged = draft.merge_into_config(Some(&original), cx);

            assert!(!merged.enabled);
            assert!(merged.required);
            assert_eq!(merged.display_name.as_deref(), Some("Filesystem"));
            assert_eq!(merged.startup_timeout_ms, Some(10));
            assert_eq!(merged.tool_timeout_ms, Some(20));
            assert_eq!(merged.enabled_tools, Some(vec!["read".to_string()]));
            assert_eq!(merged.disabled_tools, vec!["write".to_string()]);
            assert_eq!(
                merged.default_tools_approval_mode,
                Some(McpToolApprovalMode::Deny)
            );
            assert_eq!(merged.command.as_deref(), Some("new-command"));
            assert_eq!(merged.args, vec!["--new".to_string()]);
            assert_eq!(merged.url.as_deref(), Some("https://example.com/mcp"));
            assert_eq!(merged.bearer_token_env_var.as_deref(), Some("MCP_TOKEN"));
        });
    }

    fn init_form_state_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            foundation::init_i18n(cx);
        });
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<gpui_component::Root> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| TestView);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("open mcp form state test window")
        })
    }

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
        }
    }
}
