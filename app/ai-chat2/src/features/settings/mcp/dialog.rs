use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state,
    state::config::{McpServerTomlConfig, McpTransportKind},
};
use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::Input,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    v_flex,
};

use super::{
    super::push_settings_error,
    form_rows::{render_key_value_list_field, render_string_list_field, validation_error_list},
    form_state::{KeyValueField, McpServerFormDraft, StringListField},
    validation::{McpFormField, McpFormValidationError, validate_mcp_form},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum McpServerEditMode {
    Create,
    Edit { original_server_id: String },
}

impl McpServerEditMode {
    fn title_key(&self) -> &'static str {
        match self {
            Self::Create => "mcp-dialog-create-title",
            Self::Edit { .. } => "mcp-dialog-edit-title",
        }
    }

    fn original_server_id(&self) -> Option<&str> {
        match self {
            Self::Create => None,
            Self::Edit { original_server_id } => Some(original_server_id),
        }
    }

    fn is_edit(&self) -> bool {
        matches!(self, Self::Edit { .. })
    }
}

pub(super) struct McpServerEditDialogState {
    mode: McpServerEditMode,
    original_config: Option<McpServerTomlConfig>,
    draft: McpServerFormDraft,
    validation_errors: Vec<McpFormValidationError>,
    next_row_id: u64,
    content_scroll_handle: ScrollHandle,
}

impl McpServerEditDialogState {
    fn new(
        mode: McpServerEditMode,
        server: Option<McpServerTomlConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut next_row_id = 1;
        let server_id = mode.original_server_id().unwrap_or_default().to_string();
        let server_for_draft = server.clone().unwrap_or_default();
        let draft = McpServerFormDraft::from_config(
            server_id,
            &server_for_draft,
            &mut next_row_id,
            window,
            cx,
        );

        Self {
            mode,
            original_config: server,
            draft,
            validation_errors: Vec::new(),
            next_row_id,
            content_scroll_handle: ScrollHandle::default(),
        }
    }

    fn focus_primary_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if !self.mode.is_edit() {
            self.draft.server_id_input.clone()
        } else {
            match self.draft.transport {
                McpTransportKind::Stdio => self.draft.command_input.clone(),
                McpTransportKind::StreamableHttp => self.draft.url_input.clone(),
            }
        };
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let original_server_id = self.mode.original_server_id().map(ToOwned::to_owned);
        let existing_server_ids = state::config::read(cx, |config| {
            config.mcp_servers.keys().cloned().collect::<Vec<_>>()
        });
        let validation_errors = validate_mcp_form(
            &self.draft,
            original_server_id.as_deref(),
            self.original_config.as_ref(),
            &existing_server_ids,
            cx,
        );
        if !validation_errors.is_empty() {
            self.validation_errors = validation_errors;
            cx.notify();
            return false;
        }

        let server_id = self.draft.server_id(original_server_id.as_deref(), cx);
        let server = self
            .draft
            .merge_into_config(self.original_config.as_ref(), cx);

        match state::config::upsert_mcp_server(cx, original_server_id.as_deref(), server_id, server)
        {
            Ok(()) => {
                if let Some(original_server_id) = original_server_id {
                    disconnect_server(original_server_id, window, cx);
                }
                self.validation_errors.clear();
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t(match self.mode {
                            McpServerEditMode::Create => "mcp-notify-server-created",
                            McpServerEditMode::Edit { .. } => "mcp-notify-server-saved",
                        }))
                        .with_type(NotificationType::Success),
                    cx,
                );
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                push_settings_error(window, cx, title, err);
                false
            }
        }
    }

    fn render_transport_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        ToggleGroup::new("mcp-dialog-transport")
            .segmented()
            .outline()
            .w_full()
            .children([
                Toggle::new("mcp-dialog-transport-stdio")
                    .label(cx.global::<I18n>().t("mcp-transport-stdio"))
                    .checked(self.draft.transport == McpTransportKind::Stdio)
                    .flex_1()
                    .h(px(36.)),
                Toggle::new("mcp-dialog-transport-http")
                    .label(cx.global::<I18n>().t("mcp-transport-streamable-http"))
                    .checked(self.draft.transport == McpTransportKind::StreamableHttp)
                    .flex_1()
                    .h(px(36.)),
            ])
            .on_click(cx.listener(|this, states: &Vec<bool>, _window, cx| {
                this.draft.transport = transport_from_toggle_states(this.draft.transport, states);
                this.validation_errors.clear();
                cx.notify();
            }))
            .into_any_element()
    }

    fn add_string_row(
        &mut self,
        field: StringListField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft
            .add_string_row(field, &mut self.next_row_id, window, cx);
        cx.notify();
    }

    fn remove_string_row(
        &mut self,
        field: StringListField,
        row_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft
            .remove_string_row(field, row_id, &mut self.next_row_id, window, cx);
        cx.notify();
    }

    fn add_key_value_row(
        &mut self,
        field: KeyValueField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft
            .add_key_value_row(field, &mut self.next_row_id, window, cx);
        cx.notify();
    }

    fn remove_key_value_row(
        &mut self,
        field: KeyValueField,
        row_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft
            .remove_key_value_row(field, row_id, &mut self.next_row_id, window, cx);
        cx.notify();
    }

    fn field_error_messages(
        &self,
        predicate: impl Fn(&McpFormField) -> bool,
        cx: &mut App,
    ) -> Vec<SharedString> {
        self.validation_errors
            .iter()
            .filter(|error| predicate(&error.field))
            .map(|error| error.message(cx))
            .collect()
    }
}

impl Render for McpServerEditDialogState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let name_label = i18n.t("mcp-field-name");
        let transport_label = i18n.t("mcp-field-transport");
        let command_label = i18n.t("mcp-field-command");
        let args_label = i18n.t("mcp-field-args");
        let cwd_label = i18n.t("mcp-field-cwd");
        let env_label = i18n.t("mcp-field-env");
        let env_vars_label = i18n.t("mcp-field-env-vars");
        let url_label = i18n.t("mcp-field-url");
        let bearer_token_env_var_label = i18n.t("mcp-field-bearer-token-env-var");
        let headers_label = i18n.t("mcp-field-headers");
        let env_headers_label = i18n.t("mcp-field-env-headers");
        let stdio_section_label = i18n.t("mcp-section-stdio");
        let http_section_label = i18n.t("mcp-section-http");
        let add_arg_label = i18n.t("mcp-action-add-arg");
        let add_env_label = i18n.t("mcp-action-add-env");
        let add_env_var_label = i18n.t("mcp-action-add-env-var");
        let add_header_label = i18n.t("mcp-action-add-header");
        let add_env_header_label = i18n.t("mcp-action-add-env-header");
        let remove_label = i18n.t("button-delete");
        let transport = self.draft.transport;
        let scroll_handle = self.content_scroll_handle.clone();
        let dialog = cx.entity().downgrade();

        div()
            .w_full()
            .h_full()
            .relative()
            .overflow_hidden()
            .child(
                v_flex()
                    .id("mcp-server-edit-dialog-scroll")
                    .size_full()
                    .track_scroll(&scroll_handle)
                    .overflow_y_scroll()
                    .gap_4()
                    .pr_2()
                    .when(!self.validation_errors.is_empty(), |this| {
                        this.child(render_validation_summary(&self.validation_errors, cx))
                    })
                    .child(form_field(
                        name_label,
                        Input::new(&self.draft.server_id_input)
                            .w_full()
                            .disabled(self.mode.is_edit())
                            .into_any_element(),
                        self.field_error_messages(
                            |field| field.same_location(&McpFormField::ServerId),
                            cx,
                        ),
                        cx,
                    ))
                    .child(form_field(
                        transport_label,
                        self.render_transport_toggle(cx),
                        Vec::new(),
                        cx,
                    ))
                    .when(transport == McpTransportKind::Stdio, |this| {
                        let add_args_dialog = dialog.clone();
                        let remove_args_dialog = dialog.clone();
                        let add_env_dialog = dialog.clone();
                        let remove_env_dialog = dialog.clone();
                        let add_env_vars_dialog = dialog.clone();
                        let remove_env_vars_dialog = dialog.clone();

                        this.child(section_label(stdio_section_label, cx))
                            .child(form_field(
                                command_label,
                                Input::new(&self.draft.command_input)
                                    .w_full()
                                    .into_any_element(),
                                self.field_error_messages(
                                    |field| field.same_location(&McpFormField::Command),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                render_string_list_field(
                                    "mcp-dialog-args",
                                    args_label,
                                    self.draft.args.clone(),
                                    add_arg_label,
                                    remove_label.clone(),
                                )
                                .on_add(move |window, cx| {
                                    let _ = add_args_dialog.update(cx, |dialog, cx| {
                                        dialog.add_string_row(StringListField::Args, window, cx);
                                    });
                                })
                                .on_remove(
                                    move |row_id, window, cx| {
                                        let _ = remove_args_dialog.update(cx, |dialog, cx| {
                                            dialog.remove_string_row(
                                                StringListField::Args,
                                                row_id,
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                ),
                                self.field_error_messages(
                                    |field| matches!(field, McpFormField::Argument { .. }),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                render_key_value_list_field(
                                    "mcp-dialog-env",
                                    env_label,
                                    self.draft.env.clone(),
                                    add_env_label,
                                    remove_label.clone(),
                                )
                                .on_add(move |window, cx| {
                                    let _ = add_env_dialog.update(cx, |dialog, cx| {
                                        dialog.add_key_value_row(KeyValueField::Env, window, cx);
                                    });
                                })
                                .on_remove(
                                    move |row_id, window, cx| {
                                        let _ = remove_env_dialog.update(cx, |dialog, cx| {
                                            dialog.remove_key_value_row(
                                                KeyValueField::Env,
                                                row_id,
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                ),
                                self.field_error_messages(
                                    |field| {
                                        matches!(
                                            field,
                                            McpFormField::EnvKey { .. }
                                                | McpFormField::EnvValue { .. }
                                        )
                                    },
                                    cx,
                                ),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                render_string_list_field(
                                    "mcp-dialog-env-vars",
                                    env_vars_label,
                                    self.draft.env_vars.clone(),
                                    add_env_var_label,
                                    remove_label.clone(),
                                )
                                .on_add(move |window, cx| {
                                    let _ = add_env_vars_dialog.update(cx, |dialog, cx| {
                                        dialog.add_string_row(StringListField::EnvVars, window, cx);
                                    });
                                })
                                .on_remove(
                                    move |row_id, window, cx| {
                                        let _ = remove_env_vars_dialog.update(cx, |dialog, cx| {
                                            dialog.remove_string_row(
                                                StringListField::EnvVars,
                                                row_id,
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                ),
                                self.field_error_messages(
                                    |field| matches!(field, McpFormField::EnvVar { .. }),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(form_field(
                                cwd_label,
                                Input::new(&self.draft.cwd_input)
                                    .w_full()
                                    .into_any_element(),
                                self.field_error_messages(
                                    |field| field.same_location(&McpFormField::Cwd),
                                    cx,
                                ),
                                cx,
                            ))
                    })
                    .when(transport == McpTransportKind::StreamableHttp, |this| {
                        let add_headers_dialog = dialog.clone();
                        let remove_headers_dialog = dialog.clone();
                        let add_env_headers_dialog = dialog.clone();
                        let remove_env_headers_dialog = dialog.clone();

                        this.child(section_label(http_section_label, cx))
                            .child(form_field(
                                url_label,
                                Input::new(&self.draft.url_input)
                                    .w_full()
                                    .into_any_element(),
                                self.field_error_messages(
                                    |field| field.same_location(&McpFormField::Url),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(form_field(
                                bearer_token_env_var_label,
                                Input::new(&self.draft.bearer_token_env_var_input)
                                    .w_full()
                                    .into_any_element(),
                                self.field_error_messages(
                                    |field| field.same_location(&McpFormField::BearerTokenEnvVar),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                render_key_value_list_field(
                                    "mcp-dialog-headers",
                                    headers_label,
                                    self.draft.headers.clone(),
                                    add_header_label,
                                    remove_label.clone(),
                                )
                                .on_add(move |window, cx| {
                                    let _ = add_headers_dialog.update(cx, |dialog, cx| {
                                        dialog.add_key_value_row(
                                            KeyValueField::Headers,
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .on_remove(
                                    move |row_id, window, cx| {
                                        let _ = remove_headers_dialog.update(cx, |dialog, cx| {
                                            dialog.remove_key_value_row(
                                                KeyValueField::Headers,
                                                row_id,
                                                window,
                                                cx,
                                            );
                                        });
                                    },
                                ),
                                self.field_error_messages(
                                    |field| {
                                        matches!(
                                            field,
                                            McpFormField::HeaderName { .. }
                                                | McpFormField::HeaderValue { .. }
                                        )
                                    },
                                    cx,
                                ),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                render_key_value_list_field(
                                    "mcp-dialog-env-headers",
                                    env_headers_label,
                                    self.draft.env_headers.clone(),
                                    add_env_header_label,
                                    remove_label.clone(),
                                )
                                .on_add(move |window, cx| {
                                    let _ = add_env_headers_dialog.update(cx, |dialog, cx| {
                                        dialog.add_key_value_row(
                                            KeyValueField::EnvHeaders,
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .on_remove(
                                    move |row_id, window, cx| {
                                        let _ =
                                            remove_env_headers_dialog.update(cx, |dialog, cx| {
                                                dialog.remove_key_value_row(
                                                    KeyValueField::EnvHeaders,
                                                    row_id,
                                                    window,
                                                    cx,
                                                );
                                            });
                                    },
                                ),
                                self.field_error_messages(
                                    |field| {
                                        matches!(
                                            field,
                                            McpFormField::EnvHeaderName { .. }
                                                | McpFormField::EnvHeaderVar { .. }
                                        )
                                    },
                                    cx,
                                ),
                                cx,
                            ))
                            .when(
                                self.original_config
                                    .as_ref()
                                    .is_some_and(|server| server.oauth.is_some()),
                                |this| this.child(render_oauth_preserved_notice(cx)),
                            )
                    }),
            )
            .vertical_scrollbar(&scroll_handle)
    }
}

pub(super) fn open_mcp_server_edit_dialog(
    mode: McpServerEditMode,
    server: Option<McpServerTomlConfig>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<McpServerEditDialogState> {
    let title = cx.global::<I18n>().t(mode.title_key());
    let cancel_label = cx.global::<I18n>().t("button-cancel");
    let save_label = cx.global::<I18n>().t("provider-action-save");
    let form = cx.new(|cx| McpServerEditDialogState::new(mode, server, window, cx));
    let form_to_focus = form.clone();
    let form_to_return = form.clone();

    window.open_dialog(cx, move |dialog, window, _cx| {
        let dialog_height = (window.viewport_size().height - px(96.))
            .max(px(360.))
            .min(px(760.));
        dialog
            .title(title.clone())
            .w(px(720.))
            .h(dialog_height)
            .on_ok({
                let form = form.clone();
                move |_, window, cx| confirm_mcp_server_edit_dialog(&form, window, cx)
            })
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("mcp-dialog-cancel").label(cancel_label.clone())),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("mcp-dialog-save")
                                .primary()
                                .icon(IconName::Plug)
                                .label(save_label.clone()),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        form_to_focus.update(cx, |form, cx| form.focus_primary_input(window, cx));
    });

    form_to_return
}

fn confirm_mcp_server_edit_dialog(
    form: &Entity<McpServerEditDialogState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| form.save(window, cx))
}

pub(super) fn open_mcp_server_delete_confirm(server_id: String, window: &mut Window, cx: &mut App) {
    let mut args = FluentArgs::new();
    args.set("server", server_id.clone());
    let title = cx.global::<I18n>().t("mcp-delete-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("mcp-delete-description", &args);
    let deleted_title = cx.global::<I18n>().t("mcp-notify-server-deleted");
    let delete_failed_title = cx.global::<I18n>().t("mcp-notify-delete-failed");

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| match state::config::delete_mcp_server(cx, &server_id) {
            Ok(_) => {
                disconnect_server(server_id.clone(), window, cx);
                window.push_notification(
                    Notification::new()
                        .title(deleted_title.clone())
                        .with_type(NotificationType::Success),
                    cx,
                );
            }
            Err(err) => push_settings_error(window, cx, delete_failed_title.clone(), err),
        },
        window,
        cx,
    );
}

fn disconnect_server(server_id: String, window: &mut Window, cx: &mut App) {
    state::mcp::runtime(cx).update(cx, |runtime, cx| {
        runtime.disconnect_server(server_id, window, cx);
    });
}

fn transport_from_toggle_states(current: McpTransportKind, states: &[bool]) -> McpTransportKind {
    match single_selected_index(transport_toggle_index(current), states) {
        0 => McpTransportKind::Stdio,
        1 => McpTransportKind::StreamableHttp,
        _ => current,
    }
}

fn transport_toggle_index(transport: McpTransportKind) -> usize {
    match transport {
        McpTransportKind::Stdio => 0,
        McpTransportKind::StreamableHttp => 1,
    }
}

fn single_selected_index(current_index: usize, states: &[bool]) -> usize {
    states
        .iter()
        .enumerate()
        .find_map(|(index, checked)| (*checked && index != current_index).then_some(index))
        .unwrap_or(current_index)
}

fn form_field(
    label: impl Into<SharedString>,
    input: impl IntoElement,
    errors: Vec<SharedString>,
    cx: &mut App,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(Label::new(label.into()).text_sm().font_medium())
        .child(input)
        .when(!errors.is_empty(), |this| {
            this.child(validation_error_list(errors, cx))
        })
        .into_any_element()
}

fn list_field_with_errors(
    field: impl IntoElement,
    errors: Vec<SharedString>,
    cx: &mut App,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(field)
        .when(!errors.is_empty(), |this| {
            this.child(validation_error_list(errors, cx))
        })
        .into_any_element()
}

fn section_label(label: impl Into<SharedString>, cx: &mut App) -> AnyElement {
    Label::new(label.into())
        .text_sm()
        .font_medium()
        .text_color(cx.theme().muted_foreground)
        .into_any_element()
}

fn render_validation_summary(errors: &[McpFormValidationError], cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().danger.opacity(0.55))
        .bg(cx.theme().danger.opacity(0.08))
        .p_3()
        .child(
            Label::new(cx.global::<I18n>().t("mcp-validation-summary"))
                .text_sm()
                .font_medium()
                .text_color(cx.theme().danger),
        )
        .children(errors.iter().map(|error| {
            Label::new(error.message(cx))
                .text_xs()
                .line_height(relative(1.35))
                .text_color(cx.theme().danger)
        }))
        .into_any_element()
}

fn render_oauth_preserved_notice(cx: &mut App) -> AnyElement {
    Label::new(cx.global::<I18n>().t("mcp-oauth-preserved-notice"))
        .text_sm()
        .line_height(relative(1.35))
        .text_color(cx.theme().muted_foreground)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{McpTransportKind, transport_from_toggle_states};

    #[test]
    fn transport_toggle_states_keep_single_selection() {
        assert_eq!(
            transport_from_toggle_states(McpTransportKind::Stdio, &[true, true]),
            McpTransportKind::StreamableHttp
        );
        assert_eq!(
            transport_from_toggle_states(McpTransportKind::StreamableHttp, &[true, true]),
            McpTransportKind::Stdio
        );
        assert_eq!(
            transport_from_toggle_states(McpTransportKind::StreamableHttp, &[false, false]),
            McpTransportKind::StreamableHttp
        );
    }
}
