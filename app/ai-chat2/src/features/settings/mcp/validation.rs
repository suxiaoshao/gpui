use std::collections::BTreeSet;

#[cfg(test)]
use crate::state::config::McpServerTomlConfig;
use crate::state::config::{
    McpTransportKind, is_reserved_mcp_header, is_valid_mcp_env_var_name, is_valid_mcp_server_id,
};
use gpui_form::FormItemId;

#[cfg(test)]
use super::form_state::McpServerFormDraft;
use super::form_state::{McpServerFormInput, McpSubmitRowIds};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct McpFormValidationError {
    pub(super) field: McpFormField,
    pub(super) message_key: &'static str,
    pub(super) args: Vec<(&'static str, String)>,
}

impl McpFormValidationError {
    pub(super) fn new(field: McpFormField, message_key: &'static str) -> Self {
        Self {
            field,
            message_key,
            args: Vec::new(),
        }
    }

    pub(super) fn with_arg(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.args.push((key, value.into()));
        self
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum McpFormField {
    Form,
    ServerId,
    Command,
    Argument { row_id: FormItemId },
    EnvKey { row_id: FormItemId },
    EnvValue { row_id: FormItemId },
    EnvVar { row_id: FormItemId },
    Cwd,
    Url,
    BearerTokenEnvVar,
    HeaderName { row_id: FormItemId },
    HeaderValue { row_id: FormItemId },
    EnvHeaderName { row_id: FormItemId },
    EnvHeaderVar { row_id: FormItemId },
}

pub(super) struct McpSubmitValidationContext<'a> {
    pub(super) original_server_id: Option<&'a str>,
    pub(super) existing_server_ids: &'a [String],
    pub(super) row_ids: &'a McpSubmitRowIds,
}

#[cfg(test)]
pub(super) fn validate_mcp_form(
    draft: &McpServerFormDraft,
    original_server_id: Option<&str>,
    _original_config: Option<&McpServerTomlConfig>,
    existing_server_ids: &[String],
    cx: &gpui::App,
) -> Vec<McpFormValidationError> {
    let input = draft.input(cx);
    let row_ids = draft.submit_row_ids(cx);
    validate_mcp_submit_output(
        &input,
        McpSubmitValidationContext {
            original_server_id,
            existing_server_ids,
            row_ids: &row_ids,
        },
    )
}

pub(super) fn validate_mcp_submit_output(
    output: &McpServerFormInput,
    context: McpSubmitValidationContext<'_>,
) -> Vec<McpFormValidationError> {
    let mut errors = Vec::new();
    let server_id = output.server_id(context.original_server_id);

    validate_server_id(
        &server_id,
        context.original_server_id,
        context.existing_server_ids,
        &mut errors,
    );

    match output.transport {
        McpTransportKind::Stdio => validate_stdio(output, context.row_ids, &mut errors),
        McpTransportKind::StreamableHttp => validate_http(output, context.row_ids, &mut errors),
    }

    errors
}

fn validate_server_id(
    server_id: &str,
    original_server_id: Option<&str>,
    existing_server_ids: &[String],
    errors: &mut Vec<McpFormValidationError>,
) {
    if server_id.is_empty() {
        errors.push(McpFormValidationError::new(
            McpFormField::ServerId,
            "mcp-validation-name-required",
        ));
        return;
    }

    if !is_valid_mcp_server_id(server_id) {
        errors.push(
            McpFormValidationError::new(McpFormField::ServerId, "mcp-validation-name-invalid")
                .with_arg("name", server_id),
        );
        return;
    }

    if original_server_id.is_none_or(|original_server_id| original_server_id != server_id)
        && existing_server_ids
            .iter()
            .any(|existing_server_id| existing_server_id == server_id)
    {
        errors.push(
            McpFormValidationError::new(McpFormField::ServerId, "mcp-validation-name-duplicate")
                .with_arg("name", server_id),
        );
    }
}

fn validate_stdio(
    input: &McpServerFormInput,
    row_ids: &McpSubmitRowIds,
    errors: &mut Vec<McpFormValidationError>,
) {
    if input.command.trim().is_empty() {
        errors.push(McpFormValidationError::new(
            McpFormField::Command,
            "mcp-validation-command-required",
        ));
    }

    for (row_id, row) in row_ids.args.iter().copied().zip(input.args.iter()) {
        if !row.value.is_empty() && row.value.trim().is_empty() {
            errors.push(McpFormValidationError::new(
                McpFormField::Argument { row_id },
                "mcp-validation-arg-empty",
            ));
        }
    }

    validate_env_rows(input, row_ids, errors);
    validate_env_var_rows(input, row_ids, errors);
}

fn validate_env_rows(
    input: &McpServerFormInput,
    row_ids: &McpSubmitRowIds,
    errors: &mut Vec<McpFormValidationError>,
) {
    let mut seen = BTreeSet::new();
    for (row_id, row) in row_ids.env.iter().copied().zip(input.env.iter()) {
        let key = row.key.trim();
        let value = row.value.trim();
        if key.is_empty() && value.is_empty() {
            continue;
        }
        if key.is_empty() {
            errors.push(McpFormValidationError::new(
                McpFormField::EnvKey { row_id },
                "mcp-validation-env-row-incomplete",
            ));
            continue;
        }
        if !is_valid_mcp_env_var_name(key) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvKey { row_id },
                    "mcp-validation-env-name-invalid",
                )
                .with_arg("name", key),
            );
            continue;
        }
        if !seen.insert(key.to_string()) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvKey { row_id },
                    "mcp-validation-env-name-duplicate",
                )
                .with_arg("name", key),
            );
        }
    }
}

fn validate_env_var_rows(
    input: &McpServerFormInput,
    row_ids: &McpSubmitRowIds,
    errors: &mut Vec<McpFormValidationError>,
) {
    let mut seen = BTreeSet::new();
    for (row_id, row) in row_ids.env_vars.iter().copied().zip(input.env_vars.iter()) {
        let value = row.value.trim();
        if value.is_empty() {
            continue;
        }
        if !is_valid_mcp_env_var_name(value) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvVar { row_id },
                    "mcp-validation-env-name-invalid",
                )
                .with_arg("name", value),
            );
            continue;
        }
        if !seen.insert(value.to_string()) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvVar { row_id },
                    "mcp-validation-env-name-duplicate",
                )
                .with_arg("name", value),
            );
        }
    }
}

fn validate_http(
    input: &McpServerFormInput,
    row_ids: &McpSubmitRowIds,
    errors: &mut Vec<McpFormValidationError>,
) {
    let url = input.url.trim().to_string();
    if url.is_empty() {
        errors.push(McpFormValidationError::new(
            McpFormField::Url,
            "mcp-validation-url-required",
        ));
    } else {
        match url::Url::parse(&url) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => {}
            Ok(_) => errors.push(
                McpFormValidationError::new(McpFormField::Url, "mcp-validation-url-scheme")
                    .with_arg("url", url),
            ),
            Err(err) => errors.push(
                McpFormValidationError::new(McpFormField::Url, "mcp-validation-url-invalid")
                    .with_arg("url", url)
                    .with_arg("error", err.to_string()),
            ),
        }
    }

    let bearer_token_env_var = input.bearer_token_env_var.trim().to_string();
    let bearer_token_env_var = (!bearer_token_env_var.is_empty()).then_some(bearer_token_env_var);
    if let Some(env_var) = bearer_token_env_var.as_deref()
        && !is_valid_mcp_env_var_name(env_var)
    {
        errors.push(
            McpFormValidationError::new(
                McpFormField::BearerTokenEnvVar,
                "mcp-validation-bearer-env-invalid",
            )
            .with_arg("name", env_var),
        );
    }

    validate_header_rows(
        input,
        row_ids,
        bearer_token_env_var.is_some() || input.oauth_enabled,
        errors,
    );
}

fn validate_header_rows(
    input: &McpServerFormInput,
    row_ids: &McpSubmitRowIds,
    authorization_managed: bool,
    errors: &mut Vec<McpFormValidationError>,
) {
    let mut seen = BTreeSet::new();
    for (row_id, row) in row_ids.headers.iter().copied().zip(input.headers.iter()) {
        validate_header_row(
            row_id,
            row.name.trim().to_string(),
            row.value.trim().to_string(),
            false,
            authorization_managed,
            &mut seen,
            errors,
        );
    }
    for (row_id, row) in row_ids
        .env_headers
        .iter()
        .copied()
        .zip(input.env_headers.iter())
    {
        validate_header_row(
            row_id,
            row.name.trim().to_string(),
            row.env_var.trim().to_string(),
            true,
            authorization_managed,
            &mut seen,
            errors,
        );
    }
}

fn validate_header_row(
    row_id: FormItemId,
    name: String,
    value: String,
    value_is_env_var: bool,
    authorization_managed: bool,
    seen: &mut BTreeSet<String>,
    errors: &mut Vec<McpFormValidationError>,
) {
    if name.is_empty() && value.is_empty() {
        return;
    }
    let (name_field, value_field) = if value_is_env_var {
        (
            McpFormField::EnvHeaderName { row_id },
            McpFormField::EnvHeaderVar { row_id },
        )
    } else {
        (
            McpFormField::HeaderName { row_id },
            McpFormField::HeaderValue { row_id },
        )
    };

    if name.is_empty() || value.is_empty() {
        errors.push(McpFormValidationError::new(
            if name.is_empty() {
                name_field
            } else {
                value_field
            },
            "mcp-validation-header-row-incomplete",
        ));
        return;
    }

    let header_name = match http::HeaderName::from_bytes(name.as_bytes()) {
        Ok(header_name) => header_name,
        Err(err) => {
            errors.push(
                McpFormValidationError::new(name_field, "mcp-validation-header-name-invalid")
                    .with_arg("name", name)
                    .with_arg("error", err.to_string()),
            );
            return;
        }
    };
    let normalized = header_name.as_str().to_ascii_lowercase();
    if is_reserved_mcp_header(&normalized)
        || (authorization_managed && normalized.eq_ignore_ascii_case("authorization"))
    {
        errors.push(
            McpFormValidationError::new(name_field, "mcp-validation-header-reserved")
                .with_arg("name", name),
        );
        return;
    }
    if !seen.insert(normalized) {
        errors.push(
            McpFormValidationError::new(name_field, "mcp-validation-header-duplicate")
                .with_arg("name", name),
        );
        return;
    }

    if value_is_env_var {
        if !is_valid_mcp_env_var_name(&value) {
            errors.push(
                McpFormValidationError::new(value_field, "mcp-validation-env-name-invalid")
                    .with_arg("name", value),
            );
        }
    } else if let Err(err) = http::HeaderValue::from_str(&value) {
        errors.push(
            McpFormValidationError::new(value_field, "mcp-validation-header-value-invalid")
                .with_arg("name", name)
                .with_arg("error", err.to_string()),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::state::config::{
        is_reserved_mcp_header, is_valid_mcp_env_var_name, is_valid_mcp_server_id,
    };

    #[test]
    fn validates_server_id_shape() {
        assert!(is_valid_mcp_server_id("filesystem-1"));
        assert!(is_valid_mcp_server_id("github_mcp"));
        assert!(!is_valid_mcp_server_id(""));
        assert!(!is_valid_mcp_server_id("github.mcp"));
        assert!(!is_valid_mcp_server_id("github mcp"));
    }

    #[test]
    fn validates_env_var_name_shape() {
        assert!(is_valid_mcp_env_var_name("GITHUB_TOKEN"));
        assert!(is_valid_mcp_env_var_name("_TOKEN_1"));
        assert!(!is_valid_mcp_env_var_name("1TOKEN"));
        assert!(!is_valid_mcp_env_var_name("TOKEN-NAME"));
    }

    #[test]
    fn recognizes_reserved_headers_case_insensitively() {
        assert!(is_reserved_mcp_header("Accept"));
        assert!(is_reserved_mcp_header("mcp-protocol-version"));
        assert!(!is_reserved_mcp_header("x-client"));
    }
}
