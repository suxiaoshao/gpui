use std::collections::BTreeSet;

use crate::{
    foundation::I18n,
    state::config::{
        McpServerTomlConfig, McpTransportKind, is_reserved_mcp_header, is_valid_mcp_env_var_name,
        is_valid_mcp_server_id,
    },
};
use fluent_bundle::FluentArgs;
use gpui::{App, SharedString};
use gpui_form::FormItemId;

use super::form_state::McpServerFormDraft;

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

    pub(super) fn message(&self, cx: &App) -> SharedString {
        let i18n = cx.global::<I18n>();
        if self.args.is_empty() {
            return i18n.t(self.message_key).into();
        }

        let mut args = FluentArgs::new();
        for (key, value) in &self.args {
            args.set(*key, value.clone());
        }
        i18n.t_with_args(self.message_key, &args).into()
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

impl McpFormField {
    pub(super) fn same_location(&self, other: &Self) -> bool {
        self == other
    }
}

pub(super) fn validate_mcp_form(
    draft: &McpServerFormDraft,
    original_server_id: Option<&str>,
    _original_config: Option<&McpServerTomlConfig>,
    existing_server_ids: &[String],
    cx: &App,
) -> Vec<McpFormValidationError> {
    let mut errors = Vec::new();
    let server_id = draft.server_id(original_server_id, cx);

    validate_server_id(
        &server_id,
        original_server_id,
        existing_server_ids,
        &mut errors,
    );

    match draft.input(cx).transport {
        McpTransportKind::Stdio => validate_stdio(draft, &mut errors, cx),
        McpTransportKind::StreamableHttp => validate_http(draft, &mut errors, cx),
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

fn validate_stdio(draft: &McpServerFormDraft, errors: &mut Vec<McpFormValidationError>, cx: &App) {
    if draft.input(cx).command.trim().is_empty() {
        errors.push(McpFormValidationError::new(
            McpFormField::Command,
            "mcp-validation-command-required",
        ));
    }

    for row in draft.form.read(cx).args_values_with_id() {
        if !row.value.value.is_empty() && row.value.value.trim().is_empty() {
            errors.push(McpFormValidationError::new(
                McpFormField::Argument { row_id: row.id },
                "mcp-validation-arg-empty",
            ));
        }
    }

    validate_env_rows(draft, errors, cx);
    validate_env_var_rows(draft, errors, cx);
}

fn validate_env_rows(
    draft: &McpServerFormDraft,
    errors: &mut Vec<McpFormValidationError>,
    cx: &App,
) {
    let mut seen = BTreeSet::new();
    for row in draft.form.read(cx).env_values_with_id() {
        let key = row.value.key.trim();
        let value = row.value.value.trim();
        if key.is_empty() && value.is_empty() {
            continue;
        }
        if key.is_empty() {
            errors.push(McpFormValidationError::new(
                McpFormField::EnvKey { row_id: row.id },
                "mcp-validation-env-row-incomplete",
            ));
            continue;
        }
        if !is_valid_mcp_env_var_name(key) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvKey { row_id: row.id },
                    "mcp-validation-env-name-invalid",
                )
                .with_arg("name", key),
            );
            continue;
        }
        if !seen.insert(key.to_string()) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvKey { row_id: row.id },
                    "mcp-validation-env-name-duplicate",
                )
                .with_arg("name", key),
            );
        }
    }
}

fn validate_env_var_rows(
    draft: &McpServerFormDraft,
    errors: &mut Vec<McpFormValidationError>,
    cx: &App,
) {
    let mut seen = BTreeSet::new();
    for row in draft.form.read(cx).env_vars_values_with_id() {
        let value = row.value.value.trim();
        if value.is_empty() {
            continue;
        }
        if !is_valid_mcp_env_var_name(value) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvVar { row_id: row.id },
                    "mcp-validation-env-name-invalid",
                )
                .with_arg("name", value),
            );
            continue;
        }
        if !seen.insert(value.to_string()) {
            errors.push(
                McpFormValidationError::new(
                    McpFormField::EnvVar { row_id: row.id },
                    "mcp-validation-env-name-duplicate",
                )
                .with_arg("name", value),
            );
        }
    }
}

fn validate_http(draft: &McpServerFormDraft, errors: &mut Vec<McpFormValidationError>, cx: &App) {
    let input = draft.input(cx);
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
        draft,
        bearer_token_env_var.is_some() || input.oauth_enabled,
        errors,
        cx,
    );
}

fn validate_header_rows(
    draft: &McpServerFormDraft,
    authorization_managed: bool,
    errors: &mut Vec<McpFormValidationError>,
    cx: &App,
) {
    let mut seen = BTreeSet::new();
    for row in draft.form.read(cx).headers_values_with_id() {
        validate_header_row(
            row.id,
            row.value.name.trim().to_string(),
            row.value.value.trim().to_string(),
            false,
            authorization_managed,
            &mut seen,
            errors,
        );
    }
    for row in draft.form.read(cx).env_headers_values_with_id() {
        validate_header_row(
            row.id,
            row.value.name.trim().to_string(),
            row.value.env_var.trim().to_string(),
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
