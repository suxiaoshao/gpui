use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use gpui::{
    App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    StatefulInteractiveElement as _, Styled as _, Window, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    switch::Switch,
    v_flex,
};
use jaco_agent::McpServerConnectionState;

use super::tags::{connection_tag, display_name, stable_id, tool_count_tag, transport_icon};

type McpServerRowHandler = Box<dyn Fn(String, &mut Window, &mut App)>;
type McpServerToggleHandler = Box<dyn Fn(String, bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(super) struct McpServerRowView {
    row: state::mcp::McpServerStatusRow,
    selected: bool,
    on_click: McpServerRowHandler,
    on_test: McpServerRowHandler,
    on_edit: McpServerRowHandler,
    on_delete: McpServerRowHandler,
    on_toggle_enabled: McpServerToggleHandler,
}

impl McpServerRowView {
    pub(super) fn on_click(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Box::new(handler);
        self
    }

    pub(super) fn on_test(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_test = Box::new(handler);
        self
    }

    pub(super) fn on_edit(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Box::new(handler);
        self
    }

    pub(super) fn on_delete(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Box::new(handler);
        self
    }

    pub(super) fn on_toggle_enabled(
        mut self,
        handler: impl Fn(String, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_enabled = Box::new(handler);
        self
    }
}

impl RenderOnce for McpServerRowView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let row = self.row;
        let server_id = row.server_id.clone();
        let test_id = row.server_id.clone();
        let edit_id = row.server_id.clone();
        let delete_id = row.server_id.clone();
        let toggle_id = row.server_id.clone();
        let on_click = self.on_click;
        let on_test = self.on_test;
        let on_edit = self.on_edit;
        let on_delete = self.on_delete;
        let on_toggle_enabled = self.on_toggle_enabled;
        let test_label = cx.global::<I18n>().t("mcp-action-test-server");
        let edit_label = cx.global::<I18n>().t("mcp-action-edit-server");
        let delete_label = cx.global::<I18n>().t("mcp-action-delete-server");
        h_flex()
            .id(format!("mcp-server-row-{}", stable_id(&row.server_id)))
            .w_full()
            .min_w_0()
            .items_center()
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if self.selected {
                cx.theme().accent
            } else {
                cx.theme().border
            })
            .bg(if self.selected {
                cx.theme().accent.opacity(0.08)
            } else {
                cx.theme().background
            })
            .p_3()
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().muted.opacity(0.35)))
            .on_click(move |_, window, cx| on_click(server_id.clone(), window, cx))
            .child(
                Icon::new(transport_icon(&row))
                    .with_size(px(16.))
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(display_name(&row))
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(
                        Label::new(row.server_id.clone())
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(connection_tag(row.connection, cx.global::<I18n>()))
                            .child(tool_count_tag(row.tool_count, cx.global::<I18n>())),
                    ),
            )
            .child(
                Button::new(format!("mcp-server-test-{test_id}"))
                    .icon(IconName::RefreshCcw)
                    .ghost()
                    .tooltip(test_label)
                    .disabled(
                        !row.enabled || row.connection == McpServerConnectionState::Connecting,
                    )
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        on_test(test_id.clone(), window, cx);
                    }),
            )
            .child(
                Button::new(format!("mcp-server-edit-{edit_id}"))
                    .icon(IconName::Pencil)
                    .ghost()
                    .tooltip(edit_label)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        on_edit(edit_id.clone(), window, cx);
                    }),
            )
            .child(
                Button::new(format!("mcp-server-delete-{delete_id}"))
                    .icon(IconName::Trash)
                    .ghost()
                    .tooltip(delete_label)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        on_delete(delete_id.clone(), window, cx);
                    }),
            )
            .child(
                Switch::new(format!("mcp-server-enabled-{toggle_id}"))
                    .small()
                    .checked(row.enabled)
                    .on_click(move |checked, window, cx| {
                        cx.stop_propagation();
                        on_toggle_enabled(toggle_id.clone(), *checked, window, cx);
                    }),
            )
    }
}

pub(super) fn render_server_row(
    row: state::mcp::McpServerStatusRow,
    selected: bool,
) -> McpServerRowView {
    McpServerRowView {
        row,
        selected,
        on_click: Box::new(|_, _, _| {}),
        on_test: Box::new(|_, _, _| {}),
        on_edit: Box::new(|_, _, _| {}),
        on_delete: Box::new(|_, _, _| {}),
        on_toggle_enabled: Box::new(|_, _, _, _| {}),
    }
}
