use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled as _, div, px, relative,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, h_flex, label::Label, tag::Tag, v_flex,
};

use super::tags::{enabled_label, transport_label};

pub(super) fn render_config_summary(
    row: &state::mcp::McpServerStatusRow,
    cx: &mut App,
) -> AnyElement {
    let i18n = cx.global::<I18n>();
    v_flex()
        .w_full()
        .gap_2()
        .child(
            Label::new(i18n.t("mcp-section-effective-config"))
                .text_sm()
                .font_medium(),
        )
        .child(summary_line(
            i18n.t("mcp-field-enabled"),
            enabled_label(row.enabled, i18n),
            cx,
        ))
        .child(summary_line(
            i18n.t("mcp-field-required"),
            enabled_label(row.required, i18n),
            cx,
        ))
        .child(summary_line(
            i18n.t("mcp-field-transport"),
            transport_label(row.transport, i18n),
            cx,
        ))
        .child(summary_line(
            i18n.t("mcp-config-source"),
            i18n.t("mcp-config-source-toml"),
            cx,
        ))
        .into_any_element()
}

pub(super) fn render_server_info(
    info: jaco_agent::McpServerInfoSnapshot,
    cx: &mut App,
) -> AnyElement {
    let i18n = cx.global::<I18n>();
    let version = if info.version.is_empty() {
        i18n.t("mcp-value-empty")
    } else {
        info.version
    };
    v_flex()
        .w_full()
        .gap_2()
        .child(
            Label::new(i18n.t("mcp-section-server-info"))
                .text_sm()
                .font_medium(),
        )
        .child(summary_line(i18n.t("mcp-field-name"), info.name, cx))
        .child(summary_line(i18n.t("mcp-field-version"), version, cx))
        .child(summary_line(
            i18n.t("mcp-field-protocol-version"),
            info.protocol_version,
            cx,
        ))
        .children(
            info.instructions.map(|instructions| {
                summary_line(i18n.t("mcp-field-instructions"), instructions, cx)
            }),
        )
        .into_any_element()
}

pub(super) fn render_tools(tools: Vec<jaco_agent::McpToolSnapshot>, cx: &mut App) -> AnyElement {
    let body = if tools.is_empty() {
        Label::new(cx.global::<I18n>().t("mcp-empty-no-tools"))
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .into_any_element()
    } else {
        v_flex()
            .w_full()
            .gap_2()
            .children(tools.into_iter().map(|tool| render_tool(tool, cx)))
            .into_any_element()
    };
    v_flex()
        .w_full()
        .gap_2()
        .child(
            Label::new(cx.global::<I18n>().t("mcp-section-tools"))
                .text_sm()
                .font_medium(),
        )
        .child(body)
        .into_any_element()
}

pub(super) fn render_error(error: String, cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_2()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().danger)
        .bg(cx.theme().danger.opacity(0.08))
        .text_color(cx.theme().danger)
        .p_3()
        .child(Icon::new(IconName::CircleAlert).with_size(px(16.)))
        .child(Label::new(error).text_sm().line_height(relative(1.4)))
        .into_any_element()
}

fn render_tool(tool: jaco_agent::McpToolSnapshot, cx: &mut App) -> AnyElement {
    let name = tool.name;
    let title = tool
        .title
        .filter(|title| title.trim() != name.as_str() && !title.trim().is_empty());
    v_flex()
        .w_full()
        .min_w_0()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .p_3()
        .gap_1()
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(Icon::new(IconName::Wrench).with_size(px(14.)))
                .children(title.map(|title| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Label::new(title).text_sm().font_medium().truncate())
                }))
                .child(
                    Tag::secondary()
                        .small()
                        .outline()
                        .child(Label::new(name).text_xs()),
                ),
        )
        .children(tool.description.map(|description| {
            Label::new(description)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .line_height(relative(1.35))
        }))
        .into_any_element()
}

fn summary_line(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .child(
            Label::new(label.into())
                .w(px(160.))
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
        .child(Label::new(value.into()).flex_1().min_w_0().text_sm())
        .into_any_element()
}
