mod detail;
mod dialog;
mod form_rows;
mod form_state;
mod row;
mod tags;
mod validation;

use crate::{
    foundation::{I18n, assets::IconName, search::field_matches_query},
    state,
};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};

use self::{
    detail::{render_config_summary, render_error, render_server_info, render_tools},
    row::render_server_row,
    tags::{
        auth_tag, connection_tag, display_name, mcp_row_search_text, transport_icon, transport_tag,
    },
};

pub(super) struct McpSettingsPage {
    search_input: Entity<InputState>,
    selected_server_id: Option<String>,
    delete_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl McpSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(cx.global::<I18n>().t("mcp-search-placeholder"))
        });
        let runtime = state::mcp::runtime(cx);
        let config_store = state::config::store(cx);
        let _subscriptions = vec![
            cx.subscribe_in(&search_input, window, Self::on_search_input_event),
            cx.subscribe(
                &runtime,
                |_page, _store, _event: &state::mcp::McpRuntimeStoreEvent, cx| {
                    cx.notify();
                },
            ),
            config_store.observe_select_in(
                cx,
                window,
                |config| config.mcp_servers.clone(),
                |page, _, _window, cx| {
                    page.selected_server_id = None;
                    cx.notify();
                },
            ),
        ];
        Self {
            search_input,
            selected_server_id: None,
            delete_task: None,
            _subscriptions,
        }
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn filtered_rows(&self, cx: &App) -> Vec<state::mcp::McpServerStatusRow> {
        let query = self.current_query(cx);
        state::mcp::runtime(cx)
            .read(cx)
            .rows(cx)
            .into_iter()
            .filter(|row| {
                query.is_empty()
                    || field_matches_query(&mcp_row_search_text(row, cx.global::<I18n>()), &query)
            })
            .collect()
    }

    fn selected_row(
        &self,
        rows: &[state::mcp::McpServerStatusRow],
    ) -> Option<state::mcp::McpServerStatusRow> {
        self.selected_server_id
            .as_deref()
            .and_then(|server_id| rows.iter().find(|row| row.server_id == server_id))
            .or_else(|| rows.first())
            .cloned()
    }

    fn select_server(&mut self, server_id: String, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_server_id = Some(server_id);
        cx.notify();
    }

    fn test_server(&mut self, server_id: String, window: &mut Window, cx: &mut Context<Self>) {
        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.test_server(server_id, window, cx);
        });
    }

    fn refresh_servers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let server_ids = state::mcp::runtime(cx)
            .read(cx)
            .rows(cx)
            .into_iter()
            .filter(|row| row.enabled)
            .map(|row| row.server_id)
            .collect::<Vec<_>>();
        for server_id in server_ids {
            state::mcp::runtime(cx).update(cx, |runtime, cx| {
                runtime.test_server(server_id, window, cx);
            });
        }
    }

    fn open_create_server_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        dialog::open_mcp_server_edit_dialog(dialog::McpServerEditMode::Create, None, window, cx);
    }

    fn open_edit_server_dialog(
        &mut self,
        server_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let server = state::config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
        dialog::open_mcp_server_edit_dialog(
            dialog::McpServerEditMode::Edit {
                original_server_id: server_id,
            },
            server,
            window,
            cx,
        );
    }

    fn open_delete_server_dialog(
        &mut self,
        server_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.delete_task.is_some() {
            return;
        }
        dialog::open_mcp_server_delete_confirm(server_id, cx.entity().downgrade(), window, cx);
    }

    fn toggle_server_enabled(
        &mut self,
        server_id: String,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::config::set_mcp_server_enabled(cx, &server_id, enabled) {
            Ok(()) => {
                if !enabled {
                    state::mcp::runtime(cx).update(cx, |runtime, cx| {
                        runtime.disconnect_server(server_id, window, cx);
                    });
                }
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                super::push_settings_error(window, cx, title, err);
            }
        }
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                Input::new(&self.search_input)
                    .flex_1()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .cleanable(true),
            )
            .child(
                Button::new("mcp-settings-add")
                    .primary()
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("mcp-action-add-server"))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_create_server_dialog(window, cx);
                    })),
            )
            .child(
                Button::new("mcp-settings-refresh")
                    .icon(IconName::RefreshCcw)
                    .label(cx.global::<I18n>().t("mcp-action-refresh-servers"))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.refresh_servers(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = if state::mcp::runtime(cx).read(cx).rows(cx).is_empty() {
            "mcp-empty-no-servers"
        } else {
            "mcp-empty-search"
        };
        v_flex()
            .size_full()
            .min_h(px(260.))
            .items_center()
            .justify_center()
            .child(
                Label::new(cx.global::<I18n>().t(message))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element()
    }

    fn render_server_list(
        &self,
        rows: &[state::mcp::McpServerStatusRow],
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_id = self
            .selected_row(rows)
            .map(|row| row.server_id)
            .unwrap_or_default();
        let page = cx.entity().downgrade();
        div()
            .w(px(320.))
            .min_w(px(260.))
            .h_full()
            .overflow_y_scrollbar()
            .border_r_1()
            .border_color(cx.theme().border)
            .pr_3()
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(rows.iter().cloned().map(move |row| {
                        let selected = row.server_id == selected_id;
                        let select_page = page.clone();
                        let test_page = page.clone();
                        let edit_page = page.clone();
                        let delete_page = page.clone();
                        let toggle_page = page.clone();
                        render_server_row(row, selected)
                            .on_click(move |server_id, window, cx| {
                                let _ = select_page.update(cx, |page, cx| {
                                    page.select_server(server_id, window, cx);
                                });
                            })
                            .on_test(move |server_id, window, cx| {
                                let _ = test_page.update(cx, |page, cx| {
                                    page.test_server(server_id, window, cx);
                                });
                            })
                            .on_edit(move |server_id, window, cx| {
                                let _ = edit_page.update(cx, |page, cx| {
                                    page.open_edit_server_dialog(server_id, window, cx);
                                });
                            })
                            .on_delete(move |server_id, window, cx| {
                                let _ = delete_page.update(cx, |page, cx| {
                                    page.open_delete_server_dialog(server_id, window, cx);
                                });
                            })
                            .on_toggle_enabled(move |server_id, enabled, window, cx| {
                                let _ = toggle_page.update(cx, |page, cx| {
                                    page.toggle_server_enabled(server_id, enabled, window, cx);
                                });
                            })
                    })),
            )
            .into_any_element()
    }

    fn render_detail(
        &self,
        row: Option<state::mcp::McpServerStatusRow>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = row else {
            return self.render_empty(cx);
        };
        let i18n = cx.global::<I18n>();
        v_flex()
            .size_full()
            .min_w_0()
            .overflow_y_scrollbar()
            .pl_4()
            .gap_4()
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(transport_icon(&row))
                                    .with_size(px(18.))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    Label::new(display_name(&row))
                                        .text_lg()
                                        .font_medium()
                                        .truncate(),
                                ),
                            )
                            .child(transport_tag(&row, i18n))
                            .child(connection_tag(row.connection, i18n))
                            .child(auth_tag(&row.auth, i18n)),
                    )
                    .child(
                        Label::new(row.server_id.clone())
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(render_config_summary(&row, cx))
            .children(row.last_error.clone().map(|error| render_error(error, cx)))
            .children(
                row.server_info
                    .clone()
                    .map(|info| render_server_info(info, cx)),
            )
            .child(render_tools(row.tools, cx))
            .into_any_element()
    }
}

impl Render for McpSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.filtered_rows(cx);
        let selected = self.selected_row(&rows);
        let last_error = state::mcp::runtime(cx)
            .read(cx)
            .last_error()
            .map(ToOwned::to_owned);
        v_flex()
            .w_full()
            .h_full()
            .min_h_0()
            .gap_3()
            .child(self.render_toolbar(window, cx))
            .children(last_error.map(|error| render_error(error, cx)))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .when(rows.is_empty(), |this| this.child(self.render_empty(cx)))
                    .when(!rows.is_empty(), |this| {
                        this.child(self.render_server_list(&rows, window, cx))
                            .child(self.render_detail(selected, cx))
                    }),
            )
    }
}
