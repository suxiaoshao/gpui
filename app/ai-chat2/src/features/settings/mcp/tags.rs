use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use ai_chat_agent::{
    McpOAuthStatusSnapshot, McpServerConnectionState, McpServerTransportKindSnapshot,
};
use gpui::{ParentElement as _, SharedString};
use gpui_component::{Sizable, tag::Tag};

pub(super) fn display_name(row: &state::mcp::McpServerStatusRow) -> SharedString {
    row.display_name
        .clone()
        .unwrap_or_else(|| row.server_id.clone())
        .into()
}

pub(super) fn transport_icon(row: &state::mcp::McpServerStatusRow) -> IconName {
    match state::mcp::transport_icon_kind(row) {
        McpServerTransportKindSnapshot::Stdio => IconName::Terminal,
        McpServerTransportKindSnapshot::StreamableHttp => IconName::Cloud,
    }
}

pub(super) fn transport_tag(row: &state::mcp::McpServerStatusRow, i18n: &I18n) -> Tag {
    Tag::secondary()
        .small()
        .outline()
        .child(transport_label(row.transport, i18n))
}

pub(super) fn connection_tag(connection: McpServerConnectionState, i18n: &I18n) -> Tag {
    let label = match connection {
        McpServerConnectionState::Disabled => "mcp-status-disabled",
        McpServerConnectionState::NotConnected => "mcp-status-not-connected",
        McpServerConnectionState::Connecting => "mcp-status-connecting",
        McpServerConnectionState::Connected => "mcp-status-connected",
        McpServerConnectionState::Failed => "mcp-status-failed",
    };
    match connection {
        McpServerConnectionState::Connected => Tag::success(),
        McpServerConnectionState::Failed => Tag::danger(),
        McpServerConnectionState::Connecting => Tag::warning(),
        McpServerConnectionState::Disabled | McpServerConnectionState::NotConnected => {
            Tag::secondary()
        }
    }
    .small()
    .outline()
    .child(i18n.t(label))
}

pub(super) fn auth_tag(auth: &McpOAuthStatusSnapshot, i18n: &I18n) -> Tag {
    let label = match auth {
        McpOAuthStatusSnapshot::NotConfigured => "mcp-auth-not-configured",
        McpOAuthStatusSnapshot::SignedOut => "mcp-auth-signed-out",
        McpOAuthStatusSnapshot::SigningIn => "mcp-auth-signing-in",
        McpOAuthStatusSnapshot::Authorized { .. } => "mcp-auth-authorized",
        McpOAuthStatusSnapshot::AuthorizationRequired => "mcp-auth-authorization-required",
        McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. } => "mcp-auth-scope-upgrade-required",
        McpOAuthStatusSnapshot::Failed { .. } => "mcp-auth-failed",
    };
    match auth {
        McpOAuthStatusSnapshot::Authorized { .. } => Tag::success(),
        McpOAuthStatusSnapshot::Failed { .. }
        | McpOAuthStatusSnapshot::AuthorizationRequired
        | McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. } => Tag::warning(),
        McpOAuthStatusSnapshot::SigningIn => Tag::info(),
        McpOAuthStatusSnapshot::NotConfigured | McpOAuthStatusSnapshot::SignedOut => {
            Tag::secondary()
        }
    }
    .small()
    .outline()
    .child(i18n.t(label))
}

pub(super) fn tool_count_tag(tool_count: usize, i18n: &I18n) -> Tag {
    Tag::secondary().small().outline().child(format!(
        "{} {}",
        tool_count,
        i18n.t("mcp-tools-count-suffix")
    ))
}

pub(super) fn transport_label(
    transport: state::config::McpTransportKind,
    i18n: &I18n,
) -> SharedString {
    i18n.t(match transport {
        state::config::McpTransportKind::Stdio => "mcp-transport-stdio",
        state::config::McpTransportKind::StreamableHttp => "mcp-transport-streamable-http",
    })
    .into()
}

pub(super) fn enabled_label(enabled: bool, i18n: &I18n) -> SharedString {
    i18n.t(if enabled {
        "mcp-value-yes"
    } else {
        "mcp-value-no"
    })
    .into()
}

pub(super) fn mcp_row_search_text(row: &state::mcp::McpServerStatusRow, i18n: &I18n) -> String {
    format!(
        "{} {} {} {} mcp model context protocol oauth tools",
        row.server_id,
        row.display_name.clone().unwrap_or_default(),
        transport_label(row.transport, i18n),
        row.last_error.clone().unwrap_or_default(),
    )
    .to_lowercase()
}

pub(super) fn stable_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}
