use std::{collections::BTreeMap, path::PathBuf};

use crate::foundation::I18n;
use crate::state::config::{McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind};
use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use gpui_form::{FormItemId, FormStore, SubscriptionSet};

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpArgRowFormStore)]
pub(super) struct McpArgRowInput {
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvVarRowFormStore)]
pub(super) struct McpEnvVarRowInput {
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvRowFormStore)]
pub(super) struct McpEnvRowInput {
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) key: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpHeaderRowFormStore)]
pub(super) struct McpHeaderRowInput {
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = McpEnvHeaderRowFormStore)]
pub(super) struct McpEnvHeaderRowInput {
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) env_var: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(
    store = McpServerFormStore,
    validation(adapter = super::validation::McpServerValidator, context = super::validation::McpServerValidationContext),
    transform(adapter = super::validation::McpServerTransform)
)]
pub(super) struct McpServerFormInput {
    #[form(component = "value")]
    pub(super) transport: McpTransportKind,
    #[form(component = "value", required, validate(on_change, on_blur, on_submit))]
    pub(super) server_id: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) command: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) cwd: String,
    #[form(array(store = "McpArgRowFormStore"))]
    pub(super) args: Vec<McpArgRowInput>,
    #[form(array(store = "McpEnvRowFormStore"))]
    pub(super) env: Vec<McpEnvRowInput>,
    #[form(array(store = "McpEnvVarRowFormStore"))]
    pub(super) env_vars: Vec<McpEnvVarRowInput>,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) url: String,
    #[form(component = "value", validate(on_change, on_blur, on_submit))]
    pub(super) bearer_token_env_var: String,
    #[form(array(store = "McpHeaderRowFormStore"))]
    pub(super) headers: Vec<McpHeaderRowInput>,
    #[form(array(store = "McpEnvHeaderRowFormStore"))]
    pub(super) env_headers: Vec<McpEnvHeaderRowInput>,
    #[form(component = "value")]
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
}

fn new_mcp_input<T>(
    value: String,
    placeholder: String,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<InputState>
where
    T: 'static,
{
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    })
}

impl McpServerFormComponents {
    pub(super) fn bind<T>(
        form: &Entity<McpServerFormStore>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> (Self, SubscriptionSet)
    where
        T: 'static,
    {
        let mut subscriptions = SubscriptionSet::new();
        let (server_id, command, cwd, url, bearer_token_env_var) = {
            let form = form.read(cx);
            (
                form.server_id_draft(),
                form.command_draft(),
                form.cwd_draft(),
                form.url_draft(),
                form.bearer_token_env_var_draft(),
            )
        };
        let server_id_input = new_mcp_input(
            server_id,
            cx.global::<I18n>().t("mcp-placeholder-server-id"),
            window,
            cx,
        );
        let command_input = new_mcp_input(
            command,
            cx.global::<I18n>().t("mcp-placeholder-command"),
            window,
            cx,
        );
        let cwd_input = new_mcp_input(
            cwd,
            cx.global::<I18n>().t("mcp-placeholder-cwd"),
            window,
            cx,
        );
        let url_input = new_mcp_input(
            url,
            cx.global::<I18n>().t("mcp-placeholder-url"),
            window,
            cx,
        );
        let bearer_token_env_var_input = new_mcp_input(
            bearer_token_env_var,
            cx.global::<I18n>()
                .t("mcp-placeholder-bearer-token-env-var"),
            window,
            cx,
        );

        subscriptions.extend(
            gpui_form_gpui_component::bind_input(
                McpServerFormStore::server_id_handle(form),
                &server_id_input,
                window,
                cx,
            )
            .expect("bind MCP server id input"),
        );
        subscriptions.extend(
            gpui_form_gpui_component::bind_input(
                McpServerFormStore::command_handle(form),
                &command_input,
                window,
                cx,
            )
            .expect("bind MCP command input"),
        );
        subscriptions.extend(
            gpui_form_gpui_component::bind_input(
                McpServerFormStore::cwd_handle(form),
                &cwd_input,
                window,
                cx,
            )
            .expect("bind MCP cwd input"),
        );
        subscriptions.extend(
            gpui_form_gpui_component::bind_input(
                McpServerFormStore::url_handle(form),
                &url_input,
                window,
                cx,
            )
            .expect("bind MCP URL input"),
        );
        subscriptions.extend(
            gpui_form_gpui_component::bind_input(
                McpServerFormStore::bearer_token_env_var_handle(form),
                &bearer_token_env_var_input,
                window,
                cx,
            )
            .expect("bind MCP bearer token env input"),
        );

        let mut args = BTreeMap::new();
        let mut env = BTreeMap::new();
        let mut env_vars = BTreeMap::new();
        let mut headers = BTreeMap::new();
        let mut env_headers = BTreeMap::new();
        let (arg_stores, env_stores, env_var_stores, header_stores, env_header_stores) = {
            let form_state = form.read(cx);
            (
                form_state
                    .args_items()
                    .iter()
                    .map(|item| (item.id, item.item.store()))
                    .collect::<Vec<_>>(),
                form_state
                    .env_items()
                    .iter()
                    .map(|item| (item.id, item.item.store()))
                    .collect::<Vec<_>>(),
                form_state
                    .env_vars_items()
                    .iter()
                    .map(|item| (item.id, item.item.store()))
                    .collect::<Vec<_>>(),
                form_state
                    .headers_items()
                    .iter()
                    .map(|item| (item.id, item.item.store()))
                    .collect::<Vec<_>>(),
                form_state
                    .env_headers_items()
                    .iter()
                    .map(|item| (item.id, item.item.store()))
                    .collect::<Vec<_>>(),
            )
        };
        for (item_id, store) in arg_stores {
            let value = store.read(cx).value_draft();
            let input = new_mcp_input(
                value,
                cx.global::<I18n>().t("mcp-placeholder-arg"),
                window,
                cx,
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpArgRowFormStore::value_handle(&store),
                    &input,
                    window,
                    cx,
                )
                .expect("bind MCP arg input"),
            );
            args.insert(item_id, input);
        }
        for (item_id, store) in env_stores {
            let (key, value) = {
                let store = store.read(cx);
                (store.key_draft(), store.value_draft())
            };
            let key_input = new_mcp_input(
                key,
                cx.global::<I18n>().t("mcp-placeholder-env-key"),
                window,
                cx,
            );
            let value_input = new_mcp_input(
                value,
                cx.global::<I18n>().t("mcp-placeholder-env-value"),
                window,
                cx,
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpEnvRowFormStore::key_handle(&store),
                    &key_input,
                    window,
                    cx,
                )
                .expect("bind MCP env key input"),
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpEnvRowFormStore::value_handle(&store),
                    &value_input,
                    window,
                    cx,
                )
                .expect("bind MCP env value input"),
            );
            env.insert(item_id, (key_input, value_input));
        }
        for (item_id, store) in env_var_stores {
            let input = new_mcp_input(
                store.read(cx).value_draft(),
                cx.global::<I18n>().t("mcp-placeholder-env-var"),
                window,
                cx,
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpEnvVarRowFormStore::value_handle(&store),
                    &input,
                    window,
                    cx,
                )
                .expect("bind MCP env var input"),
            );
            env_vars.insert(item_id, input);
        }
        for (item_id, store) in header_stores {
            let (name, value) = {
                let store = store.read(cx);
                (store.name_draft(), store.value_draft())
            };
            let name_input = new_mcp_input(
                name,
                cx.global::<I18n>().t("mcp-placeholder-header-name"),
                window,
                cx,
            );
            let value_input = new_mcp_input(
                value,
                cx.global::<I18n>().t("mcp-placeholder-header-value"),
                window,
                cx,
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpHeaderRowFormStore::name_handle(&store),
                    &name_input,
                    window,
                    cx,
                )
                .expect("bind MCP header name input"),
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpHeaderRowFormStore::value_handle(&store),
                    &value_input,
                    window,
                    cx,
                )
                .expect("bind MCP header value input"),
            );
            headers.insert(item_id, (name_input, value_input));
        }
        for (item_id, store) in env_header_stores {
            let (name, env_var) = {
                let store = store.read(cx);
                (store.name_draft(), store.env_var_draft())
            };
            let name_input = new_mcp_input(
                name,
                cx.global::<I18n>().t("mcp-placeholder-header-name"),
                window,
                cx,
            );
            let env_var_input = new_mcp_input(
                env_var,
                cx.global::<I18n>().t("mcp-placeholder-env-header-var"),
                window,
                cx,
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpEnvHeaderRowFormStore::name_handle(&store),
                    &name_input,
                    window,
                    cx,
                )
                .expect("bind MCP env header name input"),
            );
            subscriptions.extend(
                gpui_form_gpui_component::bind_input(
                    McpEnvHeaderRowFormStore::env_var_handle(&store),
                    &env_var_input,
                    window,
                    cx,
                )
                .expect("bind MCP env header variable input"),
            );
            env_headers.insert(item_id, (name_input, env_var_input));
        }
        (
            Self {
                server_id: server_id_input,
                command: command_input,
                cwd: cwd_input,
                url: url_input,
                bearer_token_env_var: bearer_token_env_var_input,
                args,
                env,
                env_vars,
                headers,
                env_headers,
            },
            subscriptions,
        )
    }
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
        let form = cx.new(|cx| McpServerFormStore::from_value(input, window, cx));
        form.update(cx, |form, cx| {
            sync_transport_required_fields(form, window, cx);
        });
        Self { form }
    }

    pub(super) fn server_id(&self, _original_server_id: Option<&str>, cx: &App) -> String {
        self.input(cx).server_id(None)
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
            sync_transport_required_fields(form, window, cx);
        });
    }

    pub(super) fn merge_into_config(
        &self,
        original_config: Option<&McpServerTomlConfig>,
        cx: &App,
    ) -> McpServerTomlConfig {
        self.input(cx).merge_into_config(original_config)
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

fn sync_transport_required_fields(
    form: &mut McpServerFormStore,
    _window: &mut Window,
    cx: &mut gpui::Context<McpServerFormStore>,
) {
    match form.transport_value() {
        McpTransportKind::Stdio => {
            form.set_command_required(true, cx);
            form.set_url_required(false, cx);
        }
        McpTransportKind::StreamableHttp => {
            form.set_command_required(false, cx);
            form.set_url_required(true, cx);
        }
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
    use super::super::validation::McpServerValidationContext;
    use super::{
        McpArgRowFormStore, McpHeaderRowFormStore, McpServerFormDraft, McpServerFormStore,
    };
    use crate::{
        foundation,
        state::config::{
            McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind,
        },
    };
    use gpui::{
        AppContext as _, IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div,
    };
    use gpui_form::{FormStore as _, ValidationTrigger};
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
        let command_handle = cx.update(|_, _cx| McpServerFormStore::command_handle(&draft.form));
        set_form_text_value(command_handle, "new-command", &mut cx);
        let arg_handle = cx.update(|_, cx| {
            let store = draft.form.read(cx).args_items()[0].item.store();
            McpArgRowFormStore::value_handle(&store)
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
    fn required_flags_follow_transport(cx: &mut TestAppContext) {
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

        let (name_required, command_required, url_required) =
            draft.form.read_with(&cx, |form, _| {
                (
                    form.server_id_required(),
                    form.command_required(),
                    form.url_required(),
                )
            });
        assert!(name_required);
        assert!(command_required);
        assert!(!url_required);

        cx.update(|window, cx| {
            draft.set_transport(McpTransportKind::StreamableHttp, window, cx);
        });

        let (command_required, url_required) = draft.form.read_with(&cx, |form, _| {
            (form.command_required(), form.url_required())
        });
        assert!(!command_required);
        assert!(url_required);
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
            let store = draft.form.read(cx).headers_items()[0].item.store();
            (
                McpHeaderRowFormStore::name_handle(&store),
                McpHeaderRowFormStore::value_handle(&store),
            )
        });
        set_form_text_value(header_name, "Authorization", &mut cx);
        set_form_text_value(header_value, "Bearer token", &mut cx);

        cx.update(|window, cx| {
            let report = draft.form.update(cx, |form, cx| {
                form.set_validation_context(
                    McpServerValidationContext {
                        original_server_id: Some("server".to_string()),
                        existing_server_ids: Vec::new(),
                    },
                    cx,
                );
                form.validate(ValidationTrigger::Submit, window, cx)
            });
            assert!(
                report.is_valid(),
                "unexpected validation errors: {:?}",
                report.field_errors()
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
        let url_handle = cx.update(|_, _cx| McpServerFormStore::url_handle(&draft.form));
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

    fn set_form_text_value<Form>(
        handle: gpui_form::FormFieldHandle<Form, String>,
        value: &str,
        cx: &mut VisualTestContext,
    ) where
        Form: 'static,
    {
        cx.update(|_, cx| {
            handle
                .set_user_draft(value.to_string(), cx)
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
