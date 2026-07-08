use std::collections::BTreeSet;

use crate::state::config::{
    McpTransportKind, is_reserved_mcp_header, is_valid_mcp_env_var_name, is_valid_mcp_server_id,
};
use gpui_form::{
    SubmitTransform, TransformContext, TransformReport, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationScope, ValidationSource, ValidationTrigger,
};

use super::form_state::McpServerFormInput;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct McpServerValidationContext {
    pub(super) original_server_id: Option<String>,
    pub(super) existing_server_ids: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct McpServerValidator;

impl ValidationAdapter<McpServerFormInput> for McpServerValidator {
    type Context = McpServerValidationContext;

    fn validate(
        &self,
        draft: &McpServerFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &gpui::App,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::new(validate_mcp_issues(
            draft,
            context.external,
            trigger,
            &scope,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct McpServerTransform;

impl SubmitTransform<McpServerFormInput, McpServerFormInput> for McpServerTransform {
    fn preview(
        &self,
        draft: &McpServerFormInput,
        _context: &TransformContext,
    ) -> Result<McpServerFormInput, TransformReport> {
        Ok(normalize_mcp_input(draft))
    }

    fn transform_on_submit(
        &self,
        draft: &McpServerFormInput,
        context: &TransformContext,
    ) -> Result<McpServerFormInput, TransformReport> {
        self.preview(draft, context)
    }
}

fn normalize_mcp_input(input: &McpServerFormInput) -> McpServerFormInput {
    McpServerFormInput {
        transport: input.transport,
        server_id: input.server_id.trim().to_string(),
        command: input.command.trim().to_string(),
        cwd: input.cwd.trim().to_string(),
        args: input.args.clone(),
        env: input.env.clone(),
        env_vars: input.env_vars.clone(),
        url: input.url.trim().to_string(),
        bearer_token_env_var: input.bearer_token_env_var.trim().to_string(),
        headers: input.headers.clone(),
        env_headers: input.env_headers.clone(),
        oauth_enabled: input.oauth_enabled,
    }
}

fn validate_mcp_issues(
    output: &McpServerFormInput,
    context: &McpServerValidationContext,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let server_id = output.server_id(context.original_server_id.as_deref());

    validate_server_id_issue(
        &server_id,
        context.original_server_id.as_deref(),
        &context.existing_server_ids,
        trigger,
        scope,
        &mut issues,
    );

    match output.transport {
        McpTransportKind::Stdio => validate_stdio_issues(output, trigger, scope, &mut issues),
        McpTransportKind::StreamableHttp => {
            validate_http_issues(output, trigger, scope, &mut issues)
        }
    }

    issues
}

fn validate_server_id_issue(
    server_id: &str,
    original_server_id: Option<&str>,
    existing_server_ids: &[String],
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    let path = field_path("server_id");
    if server_id.is_empty() {
        push_mcp_issue(
            issues,
            path,
            trigger,
            scope,
            "mcp-validation-name-required",
            [],
        );
        return;
    }

    if !is_valid_mcp_server_id(server_id) {
        push_mcp_issue(
            issues,
            path,
            trigger,
            scope,
            "mcp-validation-name-invalid",
            [("name", server_id.to_string())],
        );
        return;
    }

    if original_server_id.is_none_or(|original_server_id| original_server_id != server_id)
        && existing_server_ids
            .iter()
            .any(|existing_server_id| existing_server_id == server_id)
    {
        push_mcp_issue(
            issues,
            path,
            trigger,
            scope,
            "mcp-validation-name-duplicate",
            [("name", server_id.to_string())],
        );
    }
}

fn validate_stdio_issues(
    input: &McpServerFormInput,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    if input.command.trim().is_empty() {
        push_mcp_issue(
            issues,
            field_path("command"),
            trigger,
            scope,
            "mcp-validation-command-required",
            [],
        );
    }

    for (index, row) in input.args.iter().enumerate() {
        if !row.value.is_empty() && row.value.trim().is_empty() {
            push_mcp_issue(
                issues,
                row_field_path("args", index, "value"),
                trigger,
                scope,
                "mcp-validation-arg-empty",
                [],
            );
        }
    }

    validate_env_row_issues(input, trigger, scope, issues);
    validate_env_var_row_issues(input, trigger, scope, issues);
}

fn validate_env_row_issues(
    input: &McpServerFormInput,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut seen = BTreeSet::new();
    for (index, row) in input.env.iter().enumerate() {
        let key = row.key.trim();
        let value = row.value.trim();
        if key.is_empty() && value.is_empty() {
            continue;
        }
        let key_path = row_field_path("env", index, "key");
        if key.is_empty() {
            push_mcp_issue(
                issues,
                key_path,
                trigger,
                scope,
                "mcp-validation-env-row-incomplete",
                [],
            );
            continue;
        }
        if !is_valid_mcp_env_var_name(key) {
            push_mcp_issue(
                issues,
                key_path,
                trigger,
                scope,
                "mcp-validation-env-name-invalid",
                [("name", key.to_string())],
            );
            continue;
        }
        if !seen.insert(key.to_string()) {
            push_mcp_issue(
                issues,
                key_path,
                trigger,
                scope,
                "mcp-validation-env-name-duplicate",
                [("name", key.to_string())],
            );
        }
    }
}

fn validate_env_var_row_issues(
    input: &McpServerFormInput,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut seen = BTreeSet::new();
    for (index, row) in input.env_vars.iter().enumerate() {
        let value = row.value.trim();
        if value.is_empty() {
            continue;
        }
        let value_path = row_field_path("env_vars", index, "value");
        if !is_valid_mcp_env_var_name(value) {
            push_mcp_issue(
                issues,
                value_path,
                trigger,
                scope,
                "mcp-validation-env-name-invalid",
                [("name", value.to_string())],
            );
            continue;
        }
        if !seen.insert(value.to_string()) {
            push_mcp_issue(
                issues,
                value_path,
                trigger,
                scope,
                "mcp-validation-env-name-duplicate",
                [("name", value.to_string())],
            );
        }
    }
}

fn validate_http_issues(
    input: &McpServerFormInput,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    let url = input.url.trim().to_string();
    if url.is_empty() {
        push_mcp_issue(
            issues,
            field_path("url"),
            trigger,
            scope,
            "mcp-validation-url-required",
            [],
        );
    } else {
        match url::Url::parse(&url) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => {}
            Ok(_) => push_mcp_issue(
                issues,
                field_path("url"),
                trigger,
                scope,
                "mcp-validation-url-scheme",
                [("url", url.to_string())],
            ),
            Err(err) => push_mcp_issue(
                issues,
                field_path("url"),
                trigger,
                scope,
                "mcp-validation-url-invalid",
                [("url", url), ("error", err.to_string())],
            ),
        }
    }

    let bearer_token_env_var = input.bearer_token_env_var.trim().to_string();
    let bearer_token_env_var = (!bearer_token_env_var.is_empty()).then_some(bearer_token_env_var);
    if let Some(env_var) = bearer_token_env_var.as_deref()
        && !is_valid_mcp_env_var_name(env_var)
    {
        push_mcp_issue(
            issues,
            field_path("bearer_token_env_var"),
            trigger,
            scope,
            "mcp-validation-bearer-env-invalid",
            [("name", env_var.to_string())],
        );
    }

    validate_header_row_issues(
        input,
        trigger,
        scope,
        bearer_token_env_var.is_some() || input.oauth_enabled,
        issues,
    );
}

fn validate_header_row_issues(
    input: &McpServerFormInput,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    authorization_managed: bool,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut seen = BTreeSet::new();
    for (index, row) in input.headers.iter().enumerate() {
        validate_header_row_issue(
            row_field_path("headers", index, "name"),
            row_field_path("headers", index, "value"),
            row.name.trim().to_string(),
            row.value.trim().to_string(),
            false,
            authorization_managed,
            &mut seen,
            trigger,
            scope,
            issues,
        );
    }
    for (index, row) in input.env_headers.iter().enumerate() {
        validate_header_row_issue(
            row_field_path("env_headers", index, "name"),
            row_field_path("env_headers", index, "env_var"),
            row.name.trim().to_string(),
            row.env_var.trim().to_string(),
            true,
            authorization_managed,
            &mut seen,
            trigger,
            scope,
            issues,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_header_row_issue(
    name_path: gpui_form::FieldPath,
    value_path: gpui_form::FieldPath,
    name: String,
    value: String,
    value_is_env_var: bool,
    authorization_managed: bool,
    seen: &mut BTreeSet<String>,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    issues: &mut Vec<ValidationIssue>,
) {
    if name.is_empty() && value.is_empty() {
        return;
    }

    if name.is_empty() || value.is_empty() {
        push_mcp_issue(
            issues,
            if name.is_empty() {
                name_path
            } else {
                value_path
            },
            trigger,
            scope,
            "mcp-validation-header-row-incomplete",
            [],
        );
        return;
    }

    let header_name = match http::HeaderName::from_bytes(name.as_bytes()) {
        Ok(header_name) => header_name,
        Err(err) => {
            push_mcp_issue(
                issues,
                name_path,
                trigger,
                scope,
                "mcp-validation-header-name-invalid",
                [("name", name), ("error", err.to_string())],
            );
            return;
        }
    };
    let normalized = header_name.as_str().to_ascii_lowercase();
    if is_reserved_mcp_header(&normalized)
        || (authorization_managed && normalized.eq_ignore_ascii_case("authorization"))
    {
        push_mcp_issue(
            issues,
            name_path,
            trigger,
            scope,
            "mcp-validation-header-reserved",
            [("name", name)],
        );
        return;
    }
    if !seen.insert(normalized) {
        push_mcp_issue(
            issues,
            name_path,
            trigger,
            scope,
            "mcp-validation-header-duplicate",
            [("name", name)],
        );
        return;
    }

    if value_is_env_var {
        if !is_valid_mcp_env_var_name(&value) {
            push_mcp_issue(
                issues,
                value_path,
                trigger,
                scope,
                "mcp-validation-env-name-invalid",
                [("name", value)],
            );
        }
    } else if let Err(err) = http::HeaderValue::from_str(&value) {
        push_mcp_issue(
            issues,
            value_path,
            trigger,
            scope,
            "mcp-validation-header-value-invalid",
            [("name", name), ("error", err.to_string())],
        );
    }
}

fn field_path(field: &'static str) -> gpui_form::FieldPath {
    gpui_form::FieldPath::from_static(field)
}

fn row_field_path(array: &'static str, index: usize, field: &'static str) -> gpui_form::FieldPath {
    field_path(array).join_index(index).join_field(field)
}

fn push_mcp_issue<const N: usize>(
    issues: &mut Vec<ValidationIssue>,
    path: gpui_form::FieldPath,
    trigger: ValidationTrigger,
    scope: &ValidationScope,
    message_key: &'static str,
    args: [(&'static str, String); N],
) {
    if !scope_includes_path(scope, &path) {
        return;
    }
    let mut issue = ValidationIssue::field(
        path,
        trigger,
        ValidationSource::App("jaco-mcp".into()),
        message_key,
        message_key,
    );
    for (key, value) in args {
        issue = issue.with_param(key, value);
    }
    issues.push(issue);
}

fn scope_includes_path(scope: &ValidationScope, path: &gpui_form::FieldPath) -> bool {
    match scope {
        ValidationScope::Form => true,
        ValidationScope::Field(field_path) => field_path == path || path.starts_with(field_path),
        ValidationScope::Group(group_path) => path.starts_with(group_path),
        ValidationScope::ArrayItem {
            path: array_path, ..
        } => path.starts_with(array_path),
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
