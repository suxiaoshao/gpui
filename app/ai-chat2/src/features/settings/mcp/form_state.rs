use std::{collections::BTreeMap, path::PathBuf};

use crate::state::config::{McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind};
use gpui::{App, AppContext as _, Entity, Window};
use gpui_form::{FormItemId, FormStore};

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpArgRowFormStore)]
pub(super) struct McpArgRowInput {
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-arg",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvVarRowFormStore)]
pub(super) struct McpEnvVarRowInput {
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-env-var",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvRowFormStore)]
pub(super) struct McpEnvRowInput {
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-env-key",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) key: String,
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-env-value",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpHeaderRowFormStore)]
pub(super) struct McpHeaderRowInput {
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-header-name",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) name: String,
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-header-value",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvHeaderRowFormStore)]
pub(super) struct McpEnvHeaderRowInput {
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-header-name",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) name: String,
    #[form(
        component = "input",
        placeholder = "mcp-placeholder-env-header-var",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) env_var: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpServerFormStore)]
pub(super) struct McpServerFormInput {
    pub(super) transport: McpTransportKind,
    #[form(
        component = "input",
        placeholder = "mcp-field-name",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) server_id: String,
    #[form(
        component = "input",
        placeholder = "mcp-field-command",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) command: String,
    #[form(
        component = "input",
        placeholder = "mcp-field-cwd",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) cwd: String,
    #[form(component = "array", store = "McpArgRowFormStore")]
    pub(super) args: Vec<McpArgRowInput>,
    #[form(component = "array", store = "McpEnvRowFormStore")]
    pub(super) env: Vec<McpEnvRowInput>,
    #[form(component = "array", store = "McpEnvVarRowFormStore")]
    pub(super) env_vars: Vec<McpEnvVarRowInput>,
    #[form(
        component = "input",
        placeholder = "mcp-field-url",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) url: String,
    #[form(
        component = "input",
        placeholder = "mcp-field-bearer-token-env-var",
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) bearer_token_env_var: String,
    #[form(component = "array", store = "McpHeaderRowFormStore")]
    pub(super) headers: Vec<McpHeaderRowInput>,
    #[form(component = "array", store = "McpEnvHeaderRowFormStore")]
    pub(super) env_headers: Vec<McpEnvHeaderRowInput>,
    #[form(component = "bool")]
    pub(super) oauth_enabled: bool,
}

pub(super) struct McpServerFormDraft {
    pub(super) form: Entity<McpServerFormStore>,
}

impl McpServerFormDraft {
    pub(super) fn from_config(
        server_id: String,
        server: &McpServerTomlConfig,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let cwd = server
            .cwd
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();

        let input = McpServerFormInput {
            transport: server.transport,
            server_id,
            command: server.command.clone().unwrap_or_default(),
            cwd,
            args: arg_inputs(server.args.iter().cloned()),
            env: env_inputs(
                server
                    .env
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            ),
            env_vars: env_var_inputs(server.env_vars.iter().cloned()),
            url: server.url.clone().unwrap_or_default(),
            bearer_token_env_var: server.bearer_token_env_var.clone().unwrap_or_default(),
            headers: header_inputs(
                server
                    .headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            ),
            env_headers: env_header_inputs(
                server
                    .env_headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            ),
            oauth_enabled: server.oauth.is_some(),
        };
        Self {
            form: cx.new(|cx| McpServerFormStore::from_value(input, window, cx)),
        }
    }

    pub(super) fn server_id(&self, _original_server_id: Option<&str>, cx: &App) -> String {
        self.input(cx).server_id.trim().to_string()
    }

    pub(super) fn input(&self, cx: &App) -> McpServerFormInput {
        self.form.read(cx).draft()
    }

    pub(super) fn set_transport(
        &mut self,
        transport: McpTransportKind,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            form.set_transport_value(
                transport,
                gpui_form::FieldChangeCause::UserInput,
                window,
                cx,
            );
        });
    }

    pub(super) fn merge_into_config(
        &self,
        original_config: Option<&McpServerTomlConfig>,
        cx: &App,
    ) -> McpServerTomlConfig {
        let mut server = original_config.cloned().unwrap_or_default();
        let input = self.input(cx);
        server.transport = input.transport;

        match input.transport {
            McpTransportKind::Stdio => {
                server.command = optional_string(input.command);
                server.args = input
                    .args
                    .into_iter()
                    .filter_map(|row| optional_string(row.value))
                    .collect();
                server.env =
                    pair_input_map(input.env.into_iter().map(|row| (row.key, row.value)), true);
                server.env_vars = input
                    .env_vars
                    .into_iter()
                    .filter_map(|row| optional_string(row.value))
                    .collect();
                server.cwd = optional_string(input.cwd).map(PathBuf::from);
                server.oauth = None;
            }
            McpTransportKind::StreamableHttp => {
                server.command = None;
                server.args.clear();
                server.env.clear();
                server.env_vars.clear();
                server.cwd = None;
                server.url = optional_string(input.url);
                server.bearer_token_env_var = optional_string(input.bearer_token_env_var);
                server.headers = pair_input_map(
                    input.headers.into_iter().map(|row| (row.name, row.value)),
                    false,
                );
                server.env_headers = pair_input_map(
                    input
                        .env_headers
                        .into_iter()
                        .map(|row| (row.name, row.env_var)),
                    false,
                );
                server.oauth = input.oauth_enabled.then(|| {
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

    pub(super) fn set_oauth_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.set_oauth_enabled_value(
                enabled,
                gpui_form::FieldChangeCause::UserInput,
                window,
                cx,
            );
        });
    }

    pub(super) fn add_arg_row(&mut self, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.args_append(empty_arg_input(), window, cx);
        });
    }

    pub(super) fn remove_arg_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            let _ = form.args_remove_id(row_id, cx);
        });
    }

    pub(super) fn add_env_var_row(&mut self, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.env_vars_append(empty_env_var_input(), window, cx);
        });
    }

    pub(super) fn remove_env_var_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            let _ = form.env_vars_remove_id(row_id, cx);
        });
    }

    pub(super) fn add_env_row(&mut self, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.env_append(empty_env_input(), window, cx);
        });
    }

    pub(super) fn remove_env_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            let _ = form.env_remove_id(row_id, cx);
        });
    }

    pub(super) fn add_header_row(&mut self, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.headers_append(empty_header_input(), window, cx);
        });
    }

    pub(super) fn remove_header_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            let _ = form.headers_remove_id(row_id, cx);
        });
    }

    pub(super) fn add_env_header_row(&mut self, window: &mut Window, cx: &mut App) {
        self.form.update(cx, |form, cx| {
            form.env_headers_append(empty_env_header_input(), window, cx);
        });
    }

    pub(super) fn remove_env_header_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.form.update(cx, |form, cx| {
            let _ = form.env_headers_remove_id(row_id, cx);
        });
    }
}

fn arg_inputs(values: impl Iterator<Item = String>) -> Vec<McpArgRowInput> {
    let mut rows = values
        .map(|value| McpArgRowInput { value })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_arg_input());
    }
    rows
}

fn env_var_inputs(values: impl Iterator<Item = String>) -> Vec<McpEnvVarRowInput> {
    let mut rows = values
        .map(|value| McpEnvVarRowInput { value })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_var_input());
    }
    rows
}

fn env_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpEnvRowInput> {
    let mut rows = values
        .map(|(key, value)| McpEnvRowInput { key, value })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_input());
    }
    rows
}

fn header_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpHeaderRowInput> {
    let mut rows = values
        .map(|(name, value)| McpHeaderRowInput { name, value })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_header_input());
    }
    rows
}

fn env_header_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpEnvHeaderRowInput> {
    let mut rows = values
        .map(|(name, env_var)| McpEnvHeaderRowInput { name, env_var })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_header_input());
    }
    rows
}

fn empty_arg_input() -> McpArgRowInput {
    McpArgRowInput {
        value: String::new(),
    }
}

fn empty_env_var_input() -> McpEnvVarRowInput {
    McpEnvVarRowInput {
        value: String::new(),
    }
}

fn empty_env_input() -> McpEnvRowInput {
    McpEnvRowInput {
        key: String::new(),
        value: String::new(),
    }
}

fn empty_header_input() -> McpHeaderRowInput {
    McpHeaderRowInput {
        name: String::new(),
        value: String::new(),
    }
}

fn empty_env_header_input() -> McpEnvHeaderRowInput {
    McpEnvHeaderRowInput {
        name: String::new(),
        env_var: String::new(),
    }
}

fn pair_input_map(
    rows: impl IntoIterator<Item = (String, String)>,
    allow_empty_values: bool,
) -> BTreeMap<String, String> {
    rows.into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.is_empty() {
                return None;
            }
            if value.is_empty() && !allow_empty_values {
                return None;
            }
            Some((key, value))
        })
        .collect()
}

fn optional_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::super::validation::validate_mcp_form;
    use super::McpServerFormDraft;
    use crate::{
        foundation,
        state::config::{
            McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind,
        },
    };
    use gpui::{
        AppContext as _, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
        WindowHandle, div,
    };
    use gpui_component::input::{InputEvent, InputState};
    use std::{collections::BTreeMap, path::PathBuf};

    #[gpui::test]
    fn merge_preserves_hidden_fields_when_editing_stdio(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let (draft, original) = cx.update(|window, cx| {
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

            (
                McpServerFormDraft::from_config("filesystem".to_string(), &original, window, cx),
                original,
            )
        });
        let command_input = cx.update(|_, cx| draft.form.read(cx).command_input_state());
        set_input_value(command_input, "new-command", &mut cx);
        let arg_input = cx.update(|_, cx| {
            draft.form.read(cx).args_items()[0]
                .item
                .store()
                .read(cx)
                .value_input_state()
        });
        set_input_value(arg_input, "--new", &mut cx);

        cx.update(|_, cx| {
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

    #[gpui::test]
    fn validation_allows_authorization_header_when_draft_oauth_disabled(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let (mut draft, original) = cx.update(|window, cx| {
            let original = McpServerTomlConfig {
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

            let draft =
                McpServerFormDraft::from_config("server".to_string(), &original, window, cx);
            (draft, original)
        });
        cx.update(|window, cx| {
            draft.set_oauth_enabled(false, window, cx);
        });
        let header_name = cx.update(|_, cx| {
            draft.form.read(cx).headers_items()[0]
                .item
                .store()
                .read(cx)
                .name_input_state()
        });
        set_input_value(header_name, "Authorization", &mut cx);
        let header_value = cx.update(|_, cx| {
            draft.form.read(cx).headers_items()[0]
                .item
                .store()
                .read(cx)
                .value_input_state()
        });
        set_input_value(header_value, "Bearer token", &mut cx);

        cx.update(|_, cx| {
            let errors = validate_mcp_form(&draft, Some("server"), Some(&original), &[], cx);
            assert!(
                errors.is_empty(),
                "unexpected validation errors: {errors:?}"
            );
        });
    }

    #[gpui::test]
    fn merge_clears_stdio_only_fields_when_saving_http(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let (mut draft, original) = cx.update(|window, cx| {
            let original = McpServerTomlConfig {
                transport: McpTransportKind::Stdio,
                command: Some("old-command".to_string()),
                args: vec!["--old".to_string()],
                env: BTreeMap::from([("OLD_ENV".to_string(), "value".to_string())]),
                env_vars: vec!["OLD_SECRET".to_string()],
                cwd: Some(PathBuf::from("/tmp/old")),
                ..Default::default()
            };

            let draft =
                McpServerFormDraft::from_config("server".to_string(), &original, window, cx);
            (draft, original)
        });
        cx.update(|window, cx| {
            draft.set_transport(McpTransportKind::StreamableHttp, window, cx);
        });
        let url_input = cx.update(|_, cx| draft.form.read(cx).url_input_state());
        set_input_value(url_input, "https://example.com/mcp", &mut cx);

        cx.update(|_, cx| {
            let merged = draft.merge_into_config(Some(&original), cx);

            assert_eq!(merged.transport, McpTransportKind::StreamableHttp);
            assert_eq!(merged.url.as_deref(), Some("https://example.com/mcp"));
            assert!(merged.command.is_none());
            assert!(merged.args.is_empty());
            assert!(merged.env.is_empty());
            assert!(merged.env_vars.is_empty());
            assert!(merged.cwd.is_none());
        });
    }

    #[gpui::test]
    fn remove_last_array_row_leaves_empty_list(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let mut draft = cx.update(|window, cx| {
            let original = McpServerTomlConfig {
                transport: McpTransportKind::Stdio,
                args: vec!["--old".to_string()],
                ..Default::default()
            };
            McpServerFormDraft::from_config("server".to_string(), &original, window, cx)
        });
        let row_id = cx.update(|_, cx| draft.form.read(cx).args_items()[0].id);

        cx.update(|window, cx| {
            draft.remove_arg_row(row_id, window, cx);
        });

        cx.update(|_, cx| {
            let form = draft.form.read(cx);
            assert!(form.args_items().is_empty());
            assert!(form.draft().args.is_empty());
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

    fn set_input_value(input: Entity<InputState>, value: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
                cx.emit(InputEvent::Change);
            });
        });
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
