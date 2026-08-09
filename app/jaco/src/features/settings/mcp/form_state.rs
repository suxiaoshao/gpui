use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
};

use crate::foundation::I18n;
use crate::state::config::{McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind};
use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use gpui_form::{
    Form, FormSchema, GardeValidator, ItemPath, MutationError, PathKey, ResolveError,
    TotalItemsPath,
};
use gpui_form_gpui_component::FormInput;

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpArgRowInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpEnvVarRowInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpEnvRowInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) key: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpHeaderRowInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpEnvHeaderRowInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) name: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) env_var: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
pub(super) struct McpServerFormInput {
    pub(super) transport: McpTransportKind,
    #[form(required, validate(on_change, on_blur, on_submit))]
    pub(super) server_id: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) command: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) cwd: String,
    #[form(items)]
    pub(super) args: Vec<McpArgRowInput>,
    #[form(items)]
    pub(super) env: Vec<McpEnvRowInput>,
    #[form(items)]
    pub(super) env_vars: Vec<McpEnvVarRowInput>,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) url: String,
    #[form(validate(on_change, on_blur, on_submit))]
    pub(super) bearer_token_env_var: String,
    #[form(items)]
    pub(super) headers: Vec<McpHeaderRowInput>,
    #[form(items)]
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
    pub(super) form: Entity<Form<McpServerFormInput>>,
}

pub(super) struct McpTextRow<Row: FormSchema> {
    pub(super) item: ItemPath<McpServerFormInput, Row>,
    pub(super) input: Entity<InputState>,
    _control: FormInput,
}

pub(super) struct McpKeyValueRow<Row: FormSchema> {
    pub(super) item: ItemPath<McpServerFormInput, Row>,
    pub(super) key: Entity<InputState>,
    pub(super) value: Entity<InputState>,
    _key_control: FormInput,
    _value_control: FormInput,
}

struct McpFixedFormControls {
    _server_id: FormInput,
    _command: FormInput,
    _cwd: FormInput,
    _url: FormInput,
    _bearer_token_env_var: FormInput,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct McpCollectionImpact {
    pub(super) args: bool,
    pub(super) env: bool,
    pub(super) env_vars: bool,
    pub(super) headers: bool,
    pub(super) env_headers: bool,
}

impl McpCollectionImpact {
    pub(super) const fn all() -> Self {
        Self {
            args: true,
            env: true,
            env_vars: true,
            headers: true,
            env_headers: true,
        }
    }

    pub(super) const fn is_empty(self) -> bool {
        !self.args && !self.env && !self.env_vars && !self.headers && !self.env_headers
    }
}

pub(super) struct McpServerFormComponents {
    pub(super) server_id: Entity<InputState>,
    pub(super) command: Entity<InputState>,
    pub(super) cwd: Entity<InputState>,
    pub(super) url: Entity<InputState>,
    pub(super) bearer_token_env_var: Entity<InputState>,
    pub(super) args: HashMap<PathKey, McpTextRow<McpArgRowInput>>,
    pub(super) env: HashMap<PathKey, McpKeyValueRow<McpEnvRowInput>>,
    pub(super) env_vars: HashMap<PathKey, McpTextRow<McpEnvVarRowInput>>,
    pub(super) headers: HashMap<PathKey, McpKeyValueRow<McpHeaderRowInput>>,
    pub(super) env_headers: HashMap<PathKey, McpKeyValueRow<McpEnvHeaderRowInput>>,
    _fixed_controls: McpFixedFormControls,
}

impl McpServerFormComponents {
    pub(super) fn try_bind<T>(
        form: &Entity<Form<McpServerFormInput>>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Result<Self, MutationError>
    where
        T: 'static,
    {
        fn bind_field<T>(
            form: &Entity<Form<McpServerFormInput>>,
            field: gpui_form::FieldDef<McpServerFormInput, String>,
            placeholder: String,
            window: &mut Window,
            cx: &mut Context<T>,
        ) -> (Entity<InputState>, FormInput)
        where
            T: 'static,
        {
            let control = FormInput::new(
                form,
                field,
                move |window, cx| InputState::new(window, cx).placeholder(placeholder),
                window,
                cx,
            );
            let input = (*control).clone();
            (input, control)
        }

        let i18n = cx.global::<I18n>().clone();
        let (server_id, server_id_control) = bind_field(
            form,
            McpServerFormInput::SERVER_ID,
            i18n.t("mcp-placeholder-server-id"),
            window,
            cx,
        );
        let (command, command_control) = bind_field(
            form,
            McpServerFormInput::COMMAND,
            i18n.t("mcp-placeholder-command"),
            window,
            cx,
        );
        let (cwd, cwd_control) = bind_field(
            form,
            McpServerFormInput::CWD,
            i18n.t("mcp-placeholder-cwd"),
            window,
            cx,
        );
        let (url, url_control) = bind_field(
            form,
            McpServerFormInput::URL,
            i18n.t("mcp-placeholder-url"),
            window,
            cx,
        );
        let (bearer_token_env_var, bearer_token_env_var_control) = bind_field(
            form,
            McpServerFormInput::BEARER_TOKEN_ENV_VAR,
            i18n.t("mcp-placeholder-bearer-token-env-var"),
            window,
            cx,
        );

        let mut components = Self {
            server_id,
            command,
            cwd,
            url,
            bearer_token_env_var,
            args: HashMap::new(),
            env: HashMap::new(),
            env_vars: HashMap::new(),
            headers: HashMap::new(),
            env_headers: HashMap::new(),
            _fixed_controls: McpFixedFormControls {
                _server_id: server_id_control,
                _command: command_control,
                _cwd: cwd_control,
                _url: url_control,
                _bearer_token_env_var: bearer_token_env_var_control,
            },
        };
        components.reconcile(form, McpCollectionImpact::all(), window, cx)?;
        Ok(components)
    }

    pub(super) fn reconcile<T>(
        &mut self,
        form: &Entity<Form<McpServerFormInput>>,
        impact: McpCollectionImpact,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Result<(), MutationError>
    where
        T: 'static,
    {
        let i18n = cx.global::<I18n>().clone();
        let args = impact
            .args
            .then(|| {
                prepare_text_rows(
                    &self.args,
                    McpServerFormInput::ARGS.items(form, cx),
                    McpArgRowInput::VALUE,
                    i18n.t("mcp-placeholder-arg"),
                    form,
                    window,
                    cx,
                )
            })
            .transpose()?;
        let env = impact
            .env
            .then(|| {
                prepare_key_value_rows(
                    &self.env,
                    KeyValueRowSpec {
                        items: McpServerFormInput::ENV.items(form, cx),
                        key_field: McpEnvRowInput::KEY,
                        value_field: McpEnvRowInput::VALUE,
                        key_placeholder: i18n.t("mcp-placeholder-env-key"),
                        value_placeholder: i18n.t("mcp-placeholder-env-value"),
                    },
                    form,
                    window,
                    cx,
                )
            })
            .transpose()?;
        let env_vars = impact
            .env_vars
            .then(|| {
                prepare_text_rows(
                    &self.env_vars,
                    McpServerFormInput::ENV_VARS.items(form, cx),
                    McpEnvVarRowInput::VALUE,
                    i18n.t("mcp-placeholder-env-var"),
                    form,
                    window,
                    cx,
                )
            })
            .transpose()?;
        let headers = impact
            .headers
            .then(|| {
                prepare_key_value_rows(
                    &self.headers,
                    KeyValueRowSpec {
                        items: McpServerFormInput::HEADERS.items(form, cx),
                        key_field: McpHeaderRowInput::NAME,
                        value_field: McpHeaderRowInput::VALUE,
                        key_placeholder: i18n.t("mcp-placeholder-header-name"),
                        value_placeholder: i18n.t("mcp-placeholder-header-value"),
                    },
                    form,
                    window,
                    cx,
                )
            })
            .transpose()?;
        let env_headers = impact
            .env_headers
            .then(|| {
                prepare_key_value_rows(
                    &self.env_headers,
                    KeyValueRowSpec {
                        items: McpServerFormInput::ENV_HEADERS.items(form, cx),
                        key_field: McpEnvHeaderRowInput::NAME,
                        value_field: McpEnvHeaderRowInput::ENV_VAR,
                        key_placeholder: i18n.t("mcp-placeholder-header-name"),
                        value_placeholder: i18n.t("mcp-placeholder-env-header-var"),
                    },
                    form,
                    window,
                    cx,
                )
            })
            .transpose()?;

        if let Some(plan) = args {
            plan.apply(&mut self.args);
        }
        if let Some(plan) = env {
            plan.apply(&mut self.env);
        }
        if let Some(plan) = env_vars {
            plan.apply(&mut self.env_vars);
        }
        if let Some(plan) = headers {
            plan.apply(&mut self.headers);
        }
        if let Some(plan) = env_headers {
            plan.apply(&mut self.env_headers);
        }
        Ok(())
    }
}

struct RowReconcilePlan<Row> {
    active: HashSet<PathKey>,
    additions: Vec<(PathKey, Row)>,
}

impl<Row> RowReconcilePlan<Row> {
    fn apply(self, rows: &mut HashMap<PathKey, Row>) {
        rows.retain(|key, _| self.active.contains(key));
        rows.extend(self.additions);
    }
}

fn prepare_text_rows<T, Row>(
    existing: &HashMap<PathKey, McpTextRow<Row>>,
    items: Vec<ItemPath<McpServerFormInput, Row>>,
    field: gpui_form::FieldDef<Row, String>,
    placeholder: String,
    form: &Entity<Form<McpServerFormInput>>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Result<RowReconcilePlan<McpTextRow<Row>>, ResolveError>
where
    T: 'static,
    Row: FormSchema,
{
    let active = items.iter().map(ItemPath::key).collect::<HashSet<_>>();
    let mut additions = Vec::new();
    for item in items {
        let key = item.key();
        if existing.contains_key(&key) {
            continue;
        }
        let control = FormInput::try_new(
            form,
            item.clone().then(field),
            {
                let placeholder = placeholder.clone();
                move |window, cx| InputState::new(window, cx).placeholder(placeholder)
            },
            window,
            cx,
        )?;
        let input = (*control).clone();
        additions.push((
            key,
            McpTextRow {
                item,
                input,
                _control: control,
            },
        ));
    }
    Ok(RowReconcilePlan { active, additions })
}

struct KeyValueRowSpec<Row: FormSchema> {
    items: Vec<ItemPath<McpServerFormInput, Row>>,
    key_field: gpui_form::FieldDef<Row, String>,
    value_field: gpui_form::FieldDef<Row, String>,
    key_placeholder: String,
    value_placeholder: String,
}

fn prepare_key_value_rows<T, Row>(
    existing: &HashMap<PathKey, McpKeyValueRow<Row>>,
    spec: KeyValueRowSpec<Row>,
    form: &Entity<Form<McpServerFormInput>>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Result<RowReconcilePlan<McpKeyValueRow<Row>>, ResolveError>
where
    T: 'static,
    Row: FormSchema,
{
    let active = spec.items.iter().map(ItemPath::key).collect::<HashSet<_>>();
    let mut additions = Vec::new();
    for item in spec.items {
        let item_key = item.key();
        if existing.contains_key(&item_key) {
            continue;
        }
        let key_control = FormInput::try_new(
            form,
            item.clone().then(spec.key_field),
            {
                let placeholder = spec.key_placeholder.clone();
                move |window, cx| InputState::new(window, cx).placeholder(placeholder)
            },
            window,
            cx,
        )?;
        let value_control = FormInput::try_new(
            form,
            item.clone().then(spec.value_field),
            {
                let placeholder = spec.value_placeholder.clone();
                move |window, cx| InputState::new(window, cx).placeholder(placeholder)
            },
            window,
            cx,
        )?;
        let key = (*key_control).clone();
        let value = (*value_control).clone();
        additions.push((
            item_key,
            McpKeyValueRow {
                item,
                key,
                value,
                _key_control: key_control,
                _value_control: value_control,
            },
        ));
    }
    Ok(RowReconcilePlan { active, additions })
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
        let form = cx.new(|_| {
            Form::new(input).with_validator(GardeValidator::<
                McpServerFormInput,
                crate::features::settings::form_validation::JacoGardeMessageProvider,
            >::new(
                super::validation::mcp_validation_context(None, Vec::new()),
            ))
        });
        Self { form }
    }

    pub(super) fn server_id(&self, _original_server_id: Option<&str>, cx: &App) -> String {
        self.input(cx).server_id(None)
    }

    pub(super) fn input(&self, cx: &App) -> McpServerFormInput {
        McpServerFormInput::ROOT.get(&self.form, cx)
    }

    pub(super) fn set_transport(
        &mut self,
        transport: McpTransportKind,
        _window: &mut Window,
        cx: &mut App,
    ) {
        McpServerFormInput::TRANSPORT.set(&self.form, transport, cx);
    }

    pub(super) fn merge_into_config(
        &self,
        original_config: Option<&McpServerTomlConfig>,
        cx: &App,
    ) -> McpServerTomlConfig {
        self.input(cx).merge_into_config(original_config)
    }

    pub(super) fn set_oauth_enabled(&mut self, enabled: bool, _window: &mut Window, cx: &mut App) {
        McpServerFormInput::OAUTH_ENABLED.set(&self.form, enabled, cx);
    }

    pub(super) fn add_arg_row(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        McpServerFormInput::ROOT
            .then(McpServerFormInput::ARGS)
            .append(&self.form, empty_arg_input(), cx)
            .map(|_| ())
    }

    pub(super) fn remove_arg_row(
        &mut self,
        row: ItemPath<McpServerFormInput, McpArgRowInput>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        self.remove_row(
            McpServerFormInput::ROOT.then(McpServerFormInput::ARGS),
            row,
            cx,
        )
    }

    pub(super) fn add_env_var_row(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        McpServerFormInput::ROOT
            .then(McpServerFormInput::ENV_VARS)
            .append(&self.form, empty_env_var_input(), cx)
            .map(|_| ())
    }

    pub(super) fn remove_env_var_row(
        &mut self,
        row: ItemPath<McpServerFormInput, McpEnvVarRowInput>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        self.remove_row(
            McpServerFormInput::ROOT.then(McpServerFormInput::ENV_VARS),
            row,
            cx,
        )
    }

    pub(super) fn add_env_row(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        McpServerFormInput::ROOT
            .then(McpServerFormInput::ENV)
            .append(&self.form, empty_env_input(), cx)
            .map(|_| ())
    }

    pub(super) fn remove_env_row(
        &mut self,
        row: ItemPath<McpServerFormInput, McpEnvRowInput>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        self.remove_row(
            McpServerFormInput::ROOT.then(McpServerFormInput::ENV),
            row,
            cx,
        )
    }

    pub(super) fn add_header_row(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        McpServerFormInput::ROOT
            .then(McpServerFormInput::HEADERS)
            .append(&self.form, empty_header_input(), cx)
            .map(|_| ())
    }

    pub(super) fn remove_header_row(
        &mut self,
        row: ItemPath<McpServerFormInput, McpHeaderRowInput>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        self.remove_row(
            McpServerFormInput::ROOT.then(McpServerFormInput::HEADERS),
            row,
            cx,
        )
    }

    pub(super) fn add_env_header_row(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        McpServerFormInput::ROOT
            .then(McpServerFormInput::ENV_HEADERS)
            .append(&self.form, empty_env_header_input(), cx)
            .map(|_| ())
    }

    pub(super) fn remove_env_header_row(
        &mut self,
        row: ItemPath<McpServerFormInput, McpEnvHeaderRowInput>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        self.remove_row(
            McpServerFormInput::ROOT.then(McpServerFormInput::ENV_HEADERS),
            row,
            cx,
        )
    }

    pub(super) fn move_row_before<Row: FormSchema>(
        &mut self,
        collection: TotalItemsPath<McpServerFormInput, Row>,
        row: &ItemPath<McpServerFormInput, Row>,
        anchor: &ItemPath<McpServerFormInput, Row>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        match collection.move_before(&self.form, row, anchor, cx) {
            Ok(()) => Ok(true),
            Err(error) if is_retired_row_mutation(&error) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn remove_row<Row: FormSchema>(
        &mut self,
        collection: TotalItemsPath<McpServerFormInput, Row>,
        row: ItemPath<McpServerFormInput, Row>,
        cx: &mut App,
    ) -> Result<bool, MutationError> {
        match collection.remove(&self.form, row, cx) {
            Ok(_) => Ok(true),
            Err(error) if is_retired_row_mutation(&error) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn is_retired_row_mutation(error: &MutationError) -> bool {
    matches!(
        error,
        MutationError::Resolve(ResolveError::Retired { .. } | ResolveError::MissingItem { .. })
    )
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
    use super::{McpArgRowInput, McpHeaderRowInput, McpServerFormDraft, McpServerFormInput};
    use crate::{
        foundation,
        state::config::{
            McpOAuthTomlConfig, McpServerTomlConfig, McpToolApprovalMode, McpTransportKind,
        },
    };
    use gpui::{
        AppContext as _, IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div,
    };
    use gpui_form::{
        DynamicPath, Form, GardeValidator, IntoTotalPath, TotalPath, ValidationTrigger,
    };
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
        let command_handle = cx.update(|_, _cx| McpServerFormInput::COMMAND.into_total_path());
        set_form_text_value(&draft.form, command_handle, "new-command", &mut cx);
        let arg_handle = cx.update(|_, cx| {
            McpServerFormInput::ARGS
                .items(&draft.form, cx)
                .remove(0)
                .then(McpArgRowInput::VALUE)
        });
        set_partial_form_text_value(&draft.form, arg_handle, "--new", &mut cx);

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
                form.validate(ValidationTrigger::Submit, cx);
            });
        });
        let (command_has_errors, url_has_errors) = cx.update(|_, cx| {
            (
                !McpServerFormInput::COMMAND
                    .errors(&draft.form, cx)
                    .is_empty(),
                !McpServerFormInput::URL.errors(&draft.form, cx).is_empty(),
            )
        });
        assert!(command_has_errors);
        assert!(!url_has_errors);

        cx.update(|window, cx| {
            draft.set_transport(McpTransportKind::StreamableHttp, window, cx);
        });

        cx.update(|_, cx| {
            draft.form.update(cx, |form, cx| {
                form.validate(ValidationTrigger::Submit, cx);
            });
        });
        let (command_has_errors, url_has_errors) = cx.update(|_, cx| {
            (
                !McpServerFormInput::COMMAND
                    .errors(&draft.form, cx)
                    .is_empty(),
                !McpServerFormInput::URL.errors(&draft.form, cx).is_empty(),
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
            let row = McpServerFormInput::HEADERS.items(&draft.form, cx).remove(0);
            (
                row.clone().then(McpHeaderRowInput::NAME),
                row.then(McpHeaderRowInput::VALUE),
            )
        });
        set_partial_form_text_value(&draft.form, header_name, "Authorization", &mut cx);
        set_partial_form_text_value(&draft.form, header_value, "Bearer token", &mut cx);

        cx.update(|_window, cx| {
            let report = draft.form.update(cx, |form, cx| {
                form.replace_validator(
                    GardeValidator::<
                        McpServerFormInput,
                        crate::features::settings::form_validation::JacoGardeMessageProvider,
                    >::new(super::super::validation::mcp_validation_context(
                        Some("server".to_string()),
                        Vec::new(),
                    )),
                    cx,
                );
                form.validate(ValidationTrigger::Submit, cx);
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
    fn incomplete_header_error_is_attached_only_to_the_missing_field(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let draft = cx.update(|window, cx| {
            McpServerFormDraft::from_config(
                "server".to_string(),
                &McpServerTomlConfig {
                    transport: McpTransportKind::StreamableHttp,
                    url: Some("https://example.com/mcp".to_string()),
                    headers: BTreeMap::from([(String::new(), "value".to_string())]),
                    ..Default::default()
                },
                window,
                cx,
            )
        });
        let (name, value) = cx.update(|_, cx| {
            let row = McpServerFormInput::HEADERS.items(&draft.form, cx).remove(0);
            (
                row.clone().then(McpHeaderRowInput::NAME),
                row.then(McpHeaderRowInput::VALUE),
            )
        });

        cx.update(|_, cx| {
            draft.form.update(cx, |form, cx| {
                form.validate(ValidationTrigger::Submit, cx);
            });
            assert_eq!(name.try_errors(&draft.form, cx).unwrap().len(), 1);
            assert!(value.try_errors(&draft.form, cx).unwrap().is_empty());
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
        let url_handle = cx.update(|_, _cx| McpServerFormInput::URL.into_total_path());
        set_form_text_value(&draft.form, url_handle, "https://example.com/mcp", &mut cx);

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
        let row = cx.update(|_, cx| McpServerFormInput::ARGS.items(&draft.form, cx).remove(0));

        cx.update(|window, cx| {
            assert!(draft.remove_arg_row(row, window, cx).unwrap());
        });

        cx.update(|_, cx| {
            assert!(
                McpServerFormInput::ROOT
                    .get(&draft.form, cx)
                    .args
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn stale_remove_is_a_noop_and_reinsert_gets_a_new_key(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let mut draft = cx.update(|window, cx| {
            McpServerFormDraft::from_config(
                "server".to_string(),
                &McpServerTomlConfig {
                    transport: McpTransportKind::Stdio,
                    args: vec!["--same".to_string()],
                    ..Default::default()
                },
                window,
                cx,
            )
        });
        let old_row = cx.update(|_, cx| McpServerFormInput::ARGS.items(&draft.form, cx).remove(0));
        let old_key = old_row.key();

        cx.update(|window, cx| {
            assert!(draft.remove_arg_row(old_row.clone(), window, cx).unwrap());
            assert!(!draft.remove_arg_row(old_row, window, cx).unwrap());
            draft.add_arg_row(window, cx).unwrap();
        });

        let new_row = cx.update(|_, cx| McpServerFormInput::ARGS.items(&draft.form, cx).remove(0));
        assert_ne!(old_key, new_row.key());
    }

    #[gpui::test]
    fn same_list_reorder_preserves_row_keys(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let mut draft = cx.update(|window, cx| {
            McpServerFormDraft::from_config(
                "server".to_string(),
                &McpServerTomlConfig {
                    transport: McpTransportKind::Stdio,
                    args: vec![
                        "first".to_string(),
                        "second".to_string(),
                        "third".to_string(),
                    ],
                    ..Default::default()
                },
                window,
                cx,
            )
        });
        let rows = cx.update(|_, cx| McpServerFormInput::ARGS.items(&draft.form, cx));
        let original_keys = rows.iter().map(|row| row.key()).collect::<Vec<_>>();

        cx.update(|window, cx| {
            assert!(
                draft
                    .move_row_before(
                        McpServerFormInput::ROOT.then(McpServerFormInput::ARGS),
                        &rows[2],
                        &rows[0],
                        window,
                        cx,
                    )
                    .unwrap()
            );
        });

        cx.update(|_, cx| {
            let reordered = McpServerFormInput::ARGS.items(&draft.form, cx);
            assert_eq!(
                reordered.iter().map(|row| row.key()).collect::<Vec<_>>(),
                vec![
                    original_keys[2].clone(),
                    original_keys[0].clone(),
                    original_keys[1].clone()
                ]
            );
            assert_eq!(
                McpServerFormInput::ROOT
                    .get(&draft.form, cx)
                    .args
                    .iter()
                    .map(|row| row.value.as_str())
                    .collect::<Vec<_>>(),
                vec!["third", "first", "second"]
            );
        });
    }

    #[gpui::test]
    fn wrong_session_remove_keeps_the_model_unchanged(cx: &mut TestAppContext) {
        init_form_state_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let (mut draft, other) = cx.update(|window, cx| {
            let server = McpServerTomlConfig {
                transport: McpTransportKind::Stdio,
                args: vec!["--kept".to_string()],
                ..Default::default()
            };
            (
                McpServerFormDraft::from_config("server".to_string(), &server, window, cx),
                McpServerFormDraft::from_config("other".to_string(), &server, window, cx),
            )
        });
        let other_row =
            cx.update(|_, cx| McpServerFormInput::ARGS.items(&other.form, cx).remove(0));
        let before = cx.update(|_, cx| draft.input(cx));

        cx.update(|window, cx| {
            assert!(draft.remove_arg_row(other_row, window, cx).is_err());
            assert_eq!(draft.input(cx), before);
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

    fn set_form_text_value(
        form: &gpui::Entity<Form<McpServerFormInput>>,
        handle: TotalPath<McpServerFormInput, String>,
        value: &str,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|_, cx| {
            handle.set(form, value.to_string(), cx);
        });
    }

    fn set_partial_form_text_value(
        form: &gpui::Entity<Form<McpServerFormInput>>,
        handle: DynamicPath<McpServerFormInput, String>,
        value: &str,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|_, cx| {
            handle
                .try_set(form, value.to_string(), cx)
                .expect("identified MCP row is available");
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
