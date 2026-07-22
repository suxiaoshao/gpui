use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::foundation::I18n;
use crate::state::config::{McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind};
use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use gpui_form::typed::FormItemId;
use gpui_form_gpui_component::FormInput;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpArgRowFormStore)]
pub(super) struct McpArgRowInput {
    pub(super) row_id: FormItemId,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvVarRowFormStore)]
pub(super) struct McpEnvVarRowInput {
    pub(super) row_id: FormItemId,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvRowFormStore)]
pub(super) struct McpEnvRowInput {
    pub(super) row_id: FormItemId,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) key: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpHeaderRowFormStore)]
pub(super) struct McpHeaderRowInput {
    pub(super) row_id: FormItemId,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvHeaderRowFormStore)]
pub(super) struct McpEnvHeaderRowInput {
    pub(super) row_id: FormItemId,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) env_var: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = McpServerFormStore,
    validation(adapter = "garde", messages = crate::features::settings::form_validation::JacoGardeMessageProvider),
    transform(adapter = super::validation::McpServerTransform)
)]
pub(super) struct McpServerFormInput {
    pub(super) transport: McpTransportKind,
    #[form(required, validate(on_change, on_blur, on_submit))]
    pub(super) server_id: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) command: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) cwd: String,
    #[form(array(id = "row_id"), validate(on_change, on_blur, on_submit))]
    pub(super) args: Vec<McpArgRowInput>,
    #[form(array(id = "row_id"), validate(on_change, on_blur, on_submit))]
    pub(super) env: Vec<McpEnvRowInput>,
    #[form(array(id = "row_id"), validate(on_change, on_blur, on_submit))]
    pub(super) env_vars: Vec<McpEnvVarRowInput>,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) url: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) bearer_token_env_var: String,
    #[form(array(id = "row_id"), validate(on_change, on_blur, on_submit))]
    pub(super) headers: Vec<McpHeaderRowInput>,
    #[form(array(id = "row_id"), validate(on_change, on_blur, on_submit))]
    pub(super) env_headers: Vec<McpEnvHeaderRowInput>,
    pub(super) oauth_enabled: bool,
}

impl McpServerFormInput {
    pub(super) fn server_id(&self, _original_server_id: Option<&str>) -> String {
        self.server_id.trim().to_string()
    }

    pub(super) fn merge_into_config(
        self,
        original_config: Option<&McpServerTomlConfig>,
    ) -> McpServerTomlConfig {
        let mut server = original_config.cloned().unwrap_or_default();
        server.transport = self.transport;

        match self.transport {
            McpTransportKind::Stdio => {
                server.command = optional_string(self.command);
                server.args = self
                    .args
                    .into_iter()
                    .filter_map(|row| optional_string(row.value))
                    .collect();
                server.env =
                    pair_input_map(self.env.into_iter().map(|row| (row.key, row.value)), true);
                server.env_vars = self
                    .env_vars
                    .into_iter()
                    .filter_map(|row| optional_string(row.value))
                    .collect();
                server.cwd = optional_string(self.cwd).map(PathBuf::from);
                server.oauth = None;
            }
            McpTransportKind::StreamableHttp => {
                server.command = None;
                server.args.clear();
                server.env.clear();
                server.env_vars.clear();
                server.cwd = None;
                server.url = optional_string(self.url);
                server.bearer_token_env_var = optional_string(self.bearer_token_env_var);
                server.headers = pair_input_map(
                    self.headers.into_iter().map(|row| (row.name, row.value)),
                    false,
                );
                server.env_headers = pair_input_map(
                    self.env_headers
                        .into_iter()
                        .map(|row| (row.name, row.env_var)),
                    false,
                );
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
}

pub(super) struct McpServerFormDraft {
    pub(super) form: Entity<McpServerFormStore>,
}

pub(super) struct McpServerFormComponents {
    pub(super) server_id: Entity<InputState>,
    pub(super) command: Entity<InputState>,
    pub(super) cwd: Entity<InputState>,
    pub(super) url: Entity<InputState>,
    pub(super) bearer_token_env_var: Entity<InputState>,
    pub(super) args: BTreeMap<FormItemId, Entity<InputState>>,
    pub(super) env: BTreeMap<FormItemId, (Entity<InputState>, Entity<InputState>)>,
    pub(super) env_vars: BTreeMap<FormItemId, Entity<InputState>>,
    pub(super) headers: BTreeMap<FormItemId, (Entity<InputState>, Entity<InputState>)>,
    pub(super) env_headers: BTreeMap<FormItemId, (Entity<InputState>, Entity<InputState>)>,
    _controls: Vec<FormInput>,
}

impl McpServerFormComponents {
    pub(super) fn bind<T>(
        form: &Entity<McpServerFormStore>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self
    where
        T: 'static,
    {
        fn bind_field<T>(
            field: gpui_form::typed::FormField<McpServerFormStore, String>,
            placeholder: String,
            controls: &mut Vec<FormInput>,
            window: &mut Window,
            cx: &mut Context<T>,
        ) -> Entity<InputState>
        where
            T: 'static,
        {
            let control = FormInput::new(
                field,
                move |window, cx| InputState::new(window, cx).placeholder(placeholder),
                window,
                cx,
            )
            .expect("bind typed MCP input");
            let input = (*control).clone();
            controls.push(control);
            input
        }

        let i18n = cx.global::<I18n>().clone();
        let mut controls = Vec::new();
        let server_id = bind_field(
            McpServerFormStore::server_id_field(form),
            i18n.t("mcp-placeholder-server-id"),
            &mut controls,
            window,
            cx,
        );
        let command = bind_field(
            McpServerFormStore::command_field(form),
            i18n.t("mcp-placeholder-command"),
            &mut controls,
            window,
            cx,
        );
        let cwd = bind_field(
            McpServerFormStore::cwd_field(form),
            i18n.t("mcp-placeholder-cwd"),
            &mut controls,
            window,
            cx,
        );
        let url = bind_field(
            McpServerFormStore::url_field(form),
            i18n.t("mcp-placeholder-url"),
            &mut controls,
            window,
            cx,
        );
        let bearer_token_env_var = bind_field(
            McpServerFormStore::bearer_token_env_var_field(form),
            i18n.t("mcp-placeholder-bearer-token-env-var"),
            &mut controls,
            window,
            cx,
        );

        let mut args = BTreeMap::new();
        for row in McpServerFormStore::args_field(form)
            .value(cx)
            .unwrap_or_default()
        {
            let item =
                McpServerFormStore::args_field(form).identified_item(row.row_id, |row| &row.row_id);
            let input = bind_field(
                item.project("value", |row| &row.value, |row, value| row.value = value),
                i18n.t("mcp-placeholder-arg"),
                &mut controls,
                window,
                cx,
            );
            args.insert(row.row_id, input);
        }
        let mut env = BTreeMap::new();
        for row in McpServerFormStore::env_field(form)
            .value(cx)
            .unwrap_or_default()
        {
            let item =
                McpServerFormStore::env_field(form).identified_item(row.row_id, |row| &row.row_id);
            let key = bind_field(
                item.project("key", |row| &row.key, |row, value| row.key = value),
                i18n.t("mcp-placeholder-env-key"),
                &mut controls,
                window,
                cx,
            );
            let value = bind_field(
                item.project("value", |row| &row.value, |row, value| row.value = value),
                i18n.t("mcp-placeholder-env-value"),
                &mut controls,
                window,
                cx,
            );
            env.insert(row.row_id, (key, value));
        }
        let mut env_vars = BTreeMap::new();
        for row in McpServerFormStore::env_vars_field(form)
            .value(cx)
            .unwrap_or_default()
        {
            let item = McpServerFormStore::env_vars_field(form)
                .identified_item(row.row_id, |row| &row.row_id);
            let input = bind_field(
                item.project("value", |row| &row.value, |row, value| row.value = value),
                i18n.t("mcp-placeholder-env-var"),
                &mut controls,
                window,
                cx,
            );
            env_vars.insert(row.row_id, input);
        }
        let mut headers = BTreeMap::new();
        for row in McpServerFormStore::headers_field(form)
            .value(cx)
            .unwrap_or_default()
        {
            let item = McpServerFormStore::headers_field(form)
                .identified_item(row.row_id, |row| &row.row_id);
            let name = bind_field(
                item.project("name", |row| &row.name, |row, value| row.name = value),
                i18n.t("mcp-placeholder-header-name"),
                &mut controls,
                window,
                cx,
            );
            let value = bind_field(
                item.project("value", |row| &row.value, |row, value| row.value = value),
                i18n.t("mcp-placeholder-header-value"),
                &mut controls,
                window,
                cx,
            );
            headers.insert(row.row_id, (name, value));
        }
        let mut env_headers = BTreeMap::new();
        for row in McpServerFormStore::env_headers_field(form)
            .value(cx)
            .unwrap_or_default()
        {
            let item = McpServerFormStore::env_headers_field(form)
                .identified_item(row.row_id, |row| &row.row_id);
            let name = bind_field(
                item.project("name", |row| &row.name, |row, value| row.name = value),
                i18n.t("mcp-placeholder-header-name"),
                &mut controls,
                window,
                cx,
            );
            let env_var = bind_field(
                item.project(
                    "env_var",
                    |row| &row.env_var,
                    |row, value| row.env_var = value,
                ),
                i18n.t("mcp-placeholder-env-header-var"),
                &mut controls,
                window,
                cx,
            );
            env_headers.insert(row.row_id, (name, env_var));
        }

        Self {
            server_id,
            command,
            cwd,
            url,
            bearer_token_env_var,
            args,
            env,
            env_vars,
            headers,
            env_headers,
            _controls: controls,
        }
    }
}

impl McpServerFormDraft {
    pub(super) fn from_config(
        server_id: String,
        server: &McpServerTomlConfig,
        _window: &mut Window,
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
        let form = cx.new(|cx| {
            McpServerFormStore::from_value_with_validation_context(
                input,
                super::validation::mcp_validation_context(None, Vec::new()),
                cx,
            )
        });
        Self { form }
    }

    pub(super) fn server_id(&self, _original_server_id: Option<&str>, cx: &App) -> String {
        self.input(cx).server_id(None)
    }

    pub(super) fn input(&self, cx: &App) -> McpServerFormInput {
        McpServerFormInput {
            transport: McpServerFormStore::transport_field(&self.form)
                .value(cx)
                .expect("MCP transport field is available"),
            server_id: McpServerFormStore::server_id_field(&self.form)
                .value(cx)
                .expect("MCP server ID field is available"),
            command: McpServerFormStore::command_field(&self.form)
                .value(cx)
                .expect("MCP command field is available"),
            cwd: McpServerFormStore::cwd_field(&self.form)
                .value(cx)
                .expect("MCP cwd field is available"),
            args: McpServerFormStore::args_field(&self.form)
                .value(cx)
                .expect("MCP args field is available"),
            env: McpServerFormStore::env_field(&self.form)
                .value(cx)
                .expect("MCP env field is available"),
            env_vars: McpServerFormStore::env_vars_field(&self.form)
                .value(cx)
                .expect("MCP env vars field is available"),
            url: McpServerFormStore::url_field(&self.form)
                .value(cx)
                .expect("MCP URL field is available"),
            bearer_token_env_var: McpServerFormStore::bearer_token_env_var_field(&self.form)
                .value(cx)
                .expect("MCP bearer token field is available"),
            headers: McpServerFormStore::headers_field(&self.form)
                .value(cx)
                .expect("MCP headers field is available"),
            env_headers: McpServerFormStore::env_headers_field(&self.form)
                .value(cx)
                .expect("MCP env headers field is available"),
            oauth_enabled: McpServerFormStore::oauth_enabled_field(&self.form)
                .value(cx)
                .expect("MCP OAuth field is available"),
        }
    }

    pub(super) fn set_transport(
        &mut self,
        transport: McpTransportKind,
        _window: &mut Window,
        cx: &mut App,
    ) {
        let _ = McpServerFormStore::transport_field(&self.form).set_user_value(transport, cx);
    }

    pub(super) fn merge_into_config(
        &self,
        original_config: Option<&McpServerTomlConfig>,
        cx: &App,
    ) -> McpServerTomlConfig {
        self.input(cx).merge_into_config(original_config)
    }

    pub(super) fn set_oauth_enabled(&mut self, enabled: bool, _window: &mut Window, cx: &mut App) {
        let _ = McpServerFormStore::oauth_enabled_field(&self.form).set_user_value(enabled, cx);
    }

    pub(super) fn add_arg_row(&mut self, _window: &mut Window, cx: &mut App) {
        append_row(
            McpServerFormStore::args_field(&self.form),
            empty_arg_input(),
            cx,
        );
    }

    pub(super) fn remove_arg_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        remove_row(
            McpServerFormStore::args_field(&self.form),
            row_id,
            |row| row.row_id,
            cx,
        );
    }

    pub(super) fn add_env_var_row(&mut self, _window: &mut Window, cx: &mut App) {
        append_row(
            McpServerFormStore::env_vars_field(&self.form),
            empty_env_var_input(),
            cx,
        );
    }

    pub(super) fn remove_env_var_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        remove_row(
            McpServerFormStore::env_vars_field(&self.form),
            row_id,
            |row| row.row_id,
            cx,
        );
    }

    pub(super) fn add_env_row(&mut self, _window: &mut Window, cx: &mut App) {
        append_row(
            McpServerFormStore::env_field(&self.form),
            empty_env_input(),
            cx,
        );
    }

    pub(super) fn remove_env_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        remove_row(
            McpServerFormStore::env_field(&self.form),
            row_id,
            |row| row.row_id,
            cx,
        );
    }

    pub(super) fn add_header_row(&mut self, _window: &mut Window, cx: &mut App) {
        append_row(
            McpServerFormStore::headers_field(&self.form),
            empty_header_input(),
            cx,
        );
    }

    pub(super) fn remove_header_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        remove_row(
            McpServerFormStore::headers_field(&self.form),
            row_id,
            |row| row.row_id,
            cx,
        );
    }

    pub(super) fn add_env_header_row(&mut self, _window: &mut Window, cx: &mut App) {
        append_row(
            McpServerFormStore::env_headers_field(&self.form),
            empty_env_header_input(),
            cx,
        );
    }

    pub(super) fn remove_env_header_row(
        &mut self,
        row_id: FormItemId,
        _window: &mut Window,
        cx: &mut App,
    ) {
        remove_row(
            McpServerFormStore::env_headers_field(&self.form),
            row_id,
            |row| row.row_id,
            cx,
        );
    }
}

fn append_row<Row: Clone + PartialEq + 'static>(
    field: gpui_form::typed::FormField<McpServerFormStore, Vec<Row>>,
    row: Row,
    cx: &mut App,
) {
    let Ok(mut rows) = field.value(cx) else {
        return;
    };
    rows.push(row);
    let _ = field.set_user_value(rows, cx);
}

fn remove_row<Row: Clone + PartialEq + 'static>(
    field: gpui_form::typed::FormField<McpServerFormStore, Vec<Row>>,
    id: FormItemId,
    row_id: impl Fn(&Row) -> FormItemId,
    cx: &mut App,
) {
    let Ok(mut rows) = field.value(cx) else {
        return;
    };
    rows.retain(|row| row_id(row) != id);
    let _ = field.set_user_value(rows, cx);
}

static NEXT_FORM_ITEM_ID: AtomicU64 = AtomicU64::new(1);

fn next_form_item_id() -> FormItemId {
    FormItemId::new(NEXT_FORM_ITEM_ID.fetch_add(1, Ordering::Relaxed))
}

fn arg_inputs(values: impl Iterator<Item = String>) -> Vec<McpArgRowInput> {
    let mut rows = values
        .map(|value| McpArgRowInput {
            row_id: next_form_item_id(),
            value,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_arg_input());
    }
    rows
}

fn env_var_inputs(values: impl Iterator<Item = String>) -> Vec<McpEnvVarRowInput> {
    let mut rows = values
        .map(|value| McpEnvVarRowInput {
            row_id: next_form_item_id(),
            value,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_var_input());
    }
    rows
}

fn env_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpEnvRowInput> {
    let mut rows = values
        .map(|(key, value)| McpEnvRowInput {
            row_id: next_form_item_id(),
            key,
            value,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_input());
    }
    rows
}

fn header_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpHeaderRowInput> {
    let mut rows = values
        .map(|(name, value)| McpHeaderRowInput {
            row_id: next_form_item_id(),
            name,
            value,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_header_input());
    }
    rows
}

fn env_header_inputs(values: impl Iterator<Item = (String, String)>) -> Vec<McpEnvHeaderRowInput> {
    let mut rows = values
        .map(|(name, env_var)| McpEnvHeaderRowInput {
            row_id: next_form_item_id(),
            name,
            env_var,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(empty_env_header_input());
    }
    rows
}

fn empty_arg_input() -> McpArgRowInput {
    McpArgRowInput {
        row_id: next_form_item_id(),
        value: String::new(),
    }
}

fn empty_env_var_input() -> McpEnvVarRowInput {
    McpEnvVarRowInput {
        row_id: next_form_item_id(),
        value: String::new(),
    }
}

fn empty_env_input() -> McpEnvRowInput {
    McpEnvRowInput {
        row_id: next_form_item_id(),
        key: String::new(),
        value: String::new(),
    }
}

fn empty_header_input() -> McpHeaderRowInput {
    McpHeaderRowInput {
        row_id: next_form_item_id(),
        name: String::new(),
        value: String::new(),
    }
}

fn empty_env_header_input() -> McpEnvHeaderRowInput {
    McpEnvHeaderRowInput {
        row_id: next_form_item_id(),
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
    use super::{McpServerFormDraft, McpServerFormStore};
    use crate::{
        foundation,
        state::config::{
            McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind,
        },
    };
    use gpui::{
        AppContext as _, IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div,
    };
    use gpui_form::typed::{FormField, FormStore as _, ValidationScope, ValidationTrigger};
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
        let command_handle = cx.update(|_, _cx| McpServerFormStore::command_field(&draft.form));
        set_form_text_value(command_handle, "new-command", &mut cx);
        let arg_handle = cx.update(|_, cx| {
            let row_id = McpServerFormStore::args_field(&draft.form)
                .value(cx)
                .unwrap()[0]
                .row_id;
            McpServerFormStore::args_field(&draft.form)
                .identified_item(row_id, |row| &row.row_id)
                .project("value", |row| &row.value, |row, value| row.value = value)
        });
        set_form_text_value(arg_handle, "--new", &mut cx);

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
    fn transport_validation_requires_the_active_endpoint(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let mut draft = cx.update(|window, cx| {
            McpServerFormDraft::from_config(
                "filesystem".to_string(),
                &McpServerTomlConfig {
                    transport: McpTransportKind::Stdio,
                    ..Default::default()
                },
                window,
                cx,
            )
        });

        cx.update(|_, cx| {
            draft.form.update(cx, |form, cx| {
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
            });
        });
        let (command_has_errors, url_has_errors) = cx.update(|_, cx| {
            (
                !McpServerFormStore::command_field(&draft.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty(),
                !McpServerFormStore::url_field(&draft.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty(),
            )
        });
        assert!(command_has_errors);
        assert!(!url_has_errors);

        cx.update(|window, cx| {
            draft.set_transport(McpTransportKind::StreamableHttp, window, cx);
        });

        cx.update(|_, cx| {
            draft.form.update(cx, |form, cx| {
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
            });
        });
        let (command_has_errors, url_has_errors) = cx.update(|_, cx| {
            (
                !McpServerFormStore::command_field(&draft.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty(),
                !McpServerFormStore::url_field(&draft.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty(),
            )
        });
        assert!(!command_has_errors);
        assert!(url_has_errors);
    }

    #[gpui::test]
    fn validation_allows_authorization_header_when_draft_oauth_disabled(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let mut draft = cx.update(|window, cx| {
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

            McpServerFormDraft::from_config("server".to_string(), &original, window, cx)
        });
        cx.update(|window, cx| {
            draft.set_oauth_enabled(false, window, cx);
        });
        let (header_name, header_value) = cx.update(|_, cx| {
            let row_id = McpServerFormStore::headers_field(&draft.form)
                .value(cx)
                .unwrap()[0]
                .row_id;
            let row = McpServerFormStore::headers_field(&draft.form)
                .identified_item(row_id, |row| &row.row_id);
            (
                row.project("name", |row| &row.name, |row, name| row.name = name),
                row.project("value", |row| &row.value, |row, value| row.value = value),
            )
        });
        set_form_text_value(header_name, "Authorization", &mut cx);
        set_form_text_value(header_value, "Bearer token", &mut cx);

        cx.update(|_window, cx| {
            let report = draft.form.update(cx, |form, cx| {
                form.set_validation_context(
                    super::super::validation::mcp_validation_context(
                        Some("server".to_string()),
                        Vec::new(),
                    ),
                    cx,
                );
                form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx);
                form.validation_report()
            });
            assert!(
                report.is_valid(),
                "unexpected validation errors: {:?}",
                report.issues()
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
        let url_handle = cx.update(|_, _cx| McpServerFormStore::url_field(&draft.form));
        set_form_text_value(url_handle, "https://example.com/mcp", &mut cx);

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
        let row_id = cx.update(|_, cx| {
            McpServerFormStore::args_field(&draft.form)
                .value(cx)
                .unwrap()[0]
                .row_id
        });

        cx.update(|window, cx| {
            draft.remove_arg_row(row_id, window, cx);
        });

        cx.update(|_, cx| {
            let form = draft.form.read(cx);
            assert!(form.value().args.is_empty());
        });
    }

    fn init_form_state_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            foundation::init_i18n(cx);
        });
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let _ = window;
                cx.new(|_| TestView)
            })
            .expect("open mcp form state test window")
        })
    }

    fn set_form_text_value<Form>(
        handle: FormField<Form, String>,
        value: &str,
        cx: &mut VisualTestContext,
    ) where
        Form: gpui_form::typed::FormStore,
    {
        cx.update(|_, cx| {
            handle
                .set_user_value(value.to_string(), cx)
                .expect("form field is alive");
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
