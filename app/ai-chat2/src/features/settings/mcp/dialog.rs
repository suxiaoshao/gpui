use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state,
    state::config::{McpServerTomlConfig, McpTransportKind, is_valid_mcp_server_id},
};
use ai_chat_agent::McpOAuthStatusSnapshot;
use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled,
    Task, WeakEntity, Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    input::Input,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use gpui_form::{
    ErrorParamValue, FieldError, FormField, FormItemId, FormMeta, FormStore as _, SubmitError,
    ValidationSource, ValidationTrigger,
};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};
use tracing::{Level, event};

use super::{
    super::push_settings_error,
    form_rows::{
        AddMcpRow, McpRowList, RemoveMcpRow, one_input_rows, two_input_rows, validation_error_list,
    },
    form_state::{
        McpArgRowFormField, McpEnvHeaderRowFormField, McpEnvRowFormField, McpEnvVarRowFormField,
        McpHeaderRowFormField, McpServerFormDraft, McpServerFormField,
    },
    validation::{
        McpFormField, McpFormValidationError, McpSubmitValidationContext,
        validate_mcp_submit_output,
    },
};

static NEXT_DRAFT_OAUTH_KEY: AtomicU64 = AtomicU64::new(1);

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
    content_scroll_handle: ScrollHandle,
    draft_oauth_status_key: String,
    draft_oauth_credential_key: Option<state::mcp_oauth::CredentialsKey>,
    draft_oauth_credential_keys: BTreeSet<state::mcp_oauth::CredentialsKey>,
    sign_out_task: Option<Task<()>>,
}

struct McpOAuthDialogTarget {
    status_key: String,
    server_id: String,
    server: McpServerTomlConfig,
    credential_key: state::mcp_oauth::CredentialsKey,
    is_draft: bool,
    cleanup_credentials: bool,
}

struct McpServerSaveRequest {
    original_server_id: Option<String>,
    server_id: String,
    server: McpServerTomlConfig,
    saved_auth: McpOAuthStatusSnapshot,
    credential_keys_to_delete: Vec<state::mcp_oauth::CredentialsKey>,
    success_title_key: &'static str,
}

struct McpOAuthSignOutRequest {
    server_id: String,
    credential_key: state::mcp_oauth::CredentialsKey,
    draft_only: bool,
}

struct OAuthSectionLabels {
    title: SharedString,
    description: SharedString,
    authorized: SharedString,
    not_authorized: SharedString,
    signing_in: SharedString,
    authorization_required: SharedString,
    scope_upgrade_required: SharedString,
    failed: SharedString,
    authorize: SharedString,
    reauthorize: SharedString,
    sign_out: SharedString,
}

impl McpServerEditDialogState {
    fn new(
        mode: McpServerEditMode,
        server: Option<McpServerTomlConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let server_id = mode.original_server_id().unwrap_or_default().to_string();
        let server_for_draft = server.clone().unwrap_or_default();
        let draft = McpServerFormDraft::from_config(server_id, &server_for_draft, window, cx);

        Self {
            mode,
            original_config: server,
            draft,
            content_scroll_handle: ScrollHandle::default(),
            draft_oauth_status_key: format!(
                "__mcp_oauth_draft_{}",
                NEXT_DRAFT_OAUTH_KEY.fetch_add(1, Ordering::Relaxed)
            ),
            draft_oauth_credential_key: None,
            draft_oauth_credential_keys: BTreeSet::new(),
            sign_out_task: None,
        }
    }

    fn focus_primary_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if !self.mode.is_edit() {
            self.draft.form.read(cx).server_id_state()
        } else {
            match self.draft.form.read(cx).transport_value() {
                McpTransportKind::Stdio => self.draft.form.read(cx).command_state(),
                McpTransportKind::StreamableHttp => self.draft.form.read(cx).url_state(),
            }
        };
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn is_saving(&self, cx: &App) -> bool {
        self.draft.form.read(cx).is_submitting()
    }

    fn is_signing_out(&self) -> bool {
        self.sign_out_task.is_some()
    }

    fn is_busy(&self, cx: &App) -> bool {
        self.is_saving(cx) || self.is_signing_out()
    }

    fn is_oauth_signing_in(&self, cx: &App) -> bool {
        matches!(self.oauth_status(cx), McpOAuthStatusSnapshot::SigningIn)
    }

    fn is_dialog_blocked(&self, cx: &App) -> bool {
        self.is_busy(cx) || self.is_oauth_signing_in(cx)
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.is_dialog_blocked(cx) {
            return false;
        }
        let original_server_id = self.mode.original_server_id().map(ToOwned::to_owned);
        let existing_server_ids = state::config::read(cx, |config| {
            config.mcp_servers.keys().cloned().collect::<Vec<_>>()
        });
        self.clear_form_errors(cx);

        let original_config = self.original_config.clone();
        let submit_row_ids = self.draft.submit_row_ids(cx);
        let draft_oauth_credential_key = self.draft_oauth_credential_key.clone();
        let draft_oauth_credential_keys = self.draft_oauth_credential_keys.clone();
        let draft_oauth_status_key = self.draft_oauth_status_key.clone();
        let success_title_key = match &self.mode {
            McpServerEditMode::Create => "mcp-notify-server-created",
            McpServerEditMode::Edit { .. } => "mcp-notify-server-saved",
        };
        let form = cx.entity().downgrade();
        let start = self.draft.form.update(cx, |form_store, cx| {
            form_store.submit_async(
                move |output, window, cx| {
                    let validation_errors = validate_mcp_submit_output(
                        &output,
                        McpSubmitValidationContext {
                            original_server_id: original_server_id.as_deref(),
                            existing_server_ids: &existing_server_ids,
                            row_ids: &submit_row_ids,
                        },
                    );
                    if !validation_errors.is_empty() {
                        return Err(validation_errors);
                    }
                    let server_id = output.server_id(original_server_id.as_deref());
                    let server = output.merge_into_config(original_config.as_ref());
                    let saved_server = server.clone();
                    let saved_auth = oauth_status_after_save(
                        draft_oauth_credential_key.as_ref(),
                        &draft_oauth_status_key,
                        original_server_id.as_deref(),
                        original_config.as_ref(),
                        &server_id,
                        &saved_server,
                        cx,
                    );
                    let credential_keys_to_delete = oauth_credential_keys_to_delete(
                        original_server_id.as_deref(),
                        original_config.as_ref(),
                        &server_id,
                        &saved_server,
                        &draft_oauth_credential_keys,
                        promoted_draft_oauth_key(
                            draft_oauth_credential_key.as_ref(),
                            &server_id,
                            &saved_server,
                        ),
                    );
                    let request = McpServerSaveRequest {
                        original_server_id,
                        server_id,
                        server,
                        saved_auth,
                        credential_keys_to_delete,
                        success_title_key,
                    };
                    Ok(window.spawn(cx, async move |cx| {
                        let result = delete_oauth_credentials_for_save(request, cx).await;
                        match form
                            .update_in(cx, |form, window, cx| form.finish_save(result, window, cx))
                        {
                            Ok(result) => result,
                            Err(err) => {
                                event!(Level::ERROR, error = ?err, "finish mcp server save failed");
                                Err(err.to_string())
                            }
                        }
                    }))
                },
                window,
                cx,
            )
        });
        match start {
            Ok(gpui_form::SubmitStart::Started) => {}
            Err(SubmitError::Handler(validation_errors)) => {
                self.apply_validation_errors(validation_errors, cx);
                cx.notify();
                return false;
            }
            Err(SubmitError::Invalid(_)) | Err(SubmitError::Busy) => {
                return false;
            }
        }
        cx.notify();
        false
    }

    fn finish_save(
        &mut self,
        result: Result<McpServerSaveRequest, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let request = match result {
            Ok(request) => request,
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                push_settings_error(window, cx, title, err.clone());
                cx.notify();
                return Err(err);
            }
        };

        let saved_server = request.server.clone();
        let saved_server_id = request.server_id.clone();
        match state::config::upsert_mcp_server(
            cx,
            request.original_server_id.as_deref(),
            request.server_id,
            request.server,
        ) {
            Ok(()) => {
                if let Some(original_server_id) = request.original_server_id {
                    disconnect_server(original_server_id, window, cx);
                }
                self.finish_oauth_after_save(
                    &saved_server_id,
                    &saved_server,
                    request.saved_auth,
                    cx,
                );
                self.clear_form_errors(cx);
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t(request.success_title_key))
                        .with_type(NotificationType::Success),
                    cx,
                );
                window.close_dialog(cx);
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                let message = err.to_string();
                push_settings_error(window, cx, title, err);
                cx.notify();
                return Err(message);
            }
        }
        cx.notify();
        Ok(())
    }

    fn finish_oauth_after_save(
        &mut self,
        server_id: &str,
        saved_server: &McpServerTomlConfig,
        saved_auth: McpOAuthStatusSnapshot,
        cx: &mut Context<Self>,
    ) {
        let saved_key = oauth_credential_key_for_server(server_id, saved_server);
        let promote_draft =
            saved_key.is_some() && self.draft_oauth_credential_key.as_ref() == saved_key.as_ref();

        if promote_draft {
            state::mcp::runtime(cx).update(cx, |runtime, cx| {
                runtime.promote_draft_oauth_authorization(
                    &self.draft_oauth_status_key,
                    server_id.to_string(),
                    saved_server.clone(),
                    cx,
                );
            });
            if let Some(key) = saved_key.as_ref() {
                self.draft_oauth_credential_keys.remove(key);
            }
        } else {
            self.clear_draft_oauth_authorization(cx);
        }

        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.replace_saved_server_status(
                server_id.to_string(),
                saved_server,
                saved_auth,
                cx,
            );
        });

        self.draft_oauth_credential_keys.clear();
        self.draft_oauth_credential_key = None;
    }

    fn render_transport_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let transport = self.draft.form.read(cx).transport_value();
        ToggleGroup::new("mcp-dialog-transport")
            .segmented()
            .outline()
            .w_full()
            .children([
                Toggle::new("mcp-dialog-transport-stdio")
                    .label(cx.global::<I18n>().t("mcp-transport-stdio"))
                    .checked(transport == McpTransportKind::Stdio)
                    .flex_1()
                    .h(px(36.)),
                Toggle::new("mcp-dialog-transport-http")
                    .label(cx.global::<I18n>().t("mcp-transport-streamable-http"))
                    .checked(transport == McpTransportKind::StreamableHttp)
                    .flex_1()
                    .h(px(36.)),
            ])
            .on_click(cx.listener(|this, states: &Vec<bool>, window, cx| {
                let transport = transport_from_toggle_states(
                    this.draft.form.read(cx).transport_value(),
                    states,
                );
                this.draft.set_transport(transport, window, cx);
                this.clear_form_errors(cx);
                cx.notify();
            }))
            .into_any_element()
    }

    fn on_add_mcp_row(&mut self, action: &AddMcpRow, window: &mut Window, cx: &mut Context<Self>) {
        match action.list {
            McpRowList::Args => self.draft.add_arg_row(window, cx),
            McpRowList::Env => self.draft.add_env_row(window, cx),
            McpRowList::EnvVars => self.draft.add_env_var_row(window, cx),
            McpRowList::Headers => self.draft.add_header_row(window, cx),
            McpRowList::EnvHeaders => self.draft.add_env_header_row(window, cx),
        }
        self.clear_form_errors(cx);
        cx.notify();
    }

    fn on_remove_mcp_row(
        &mut self,
        action: &RemoveMcpRow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let row_id = FormItemId::new(action.row_id);
        match action.list {
            McpRowList::Args => self.draft.remove_arg_row(row_id, window, cx),
            McpRowList::Env => self.draft.remove_env_row(row_id, window, cx),
            McpRowList::EnvVars => self.draft.remove_env_var_row(row_id, window, cx),
            McpRowList::Headers => self.draft.remove_header_row(row_id, window, cx),
            McpRowList::EnvHeaders => self.draft.remove_env_header_row(row_id, window, cx),
        }
        self.clear_form_errors(cx);
        cx.notify();
    }

    fn set_oauth_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.set_oauth_enabled(enabled, window, cx);
        self.clear_form_errors(cx);
        cx.notify();
    }

    fn authorize_oauth(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.draft_oauth_target(cx) else {
            return;
        };
        if target.is_draft {
            if self.draft_oauth_credential_key.as_ref() != Some(&target.credential_key) {
                state::mcp::runtime(cx).update(cx, |runtime, cx| {
                    runtime.discard_draft_oauth_authorization(&self.draft_oauth_status_key, cx);
                });
            }
            self.draft_oauth_credential_key = Some(target.credential_key.clone());
            if target.cleanup_credentials {
                self.draft_oauth_credential_keys
                    .insert(target.credential_key.clone());
            }
        } else {
            self.clear_draft_oauth_authorization(cx);
        }
        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.authenticate_server_config(
                target.status_key,
                target.server_id,
                target.server,
                window,
                cx,
            );
        });
    }

    fn sign_out_oauth(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy(cx) {
            return;
        }
        let Some(target) = self.draft_oauth_target(cx) else {
            return;
        };
        let request = McpOAuthSignOutRequest {
            server_id: if target.is_draft && !target.cleanup_credentials {
                self.mode
                    .original_server_id()
                    .unwrap_or(target.server_id.as_str())
                    .to_string()
            } else {
                target.server_id
            },
            credential_key: target.credential_key,
            draft_only: target.is_draft && target.cleanup_credentials,
        };
        let form = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = delete_oauth_credentials_for_sign_out(request, cx).await;
            if let Err(err) = form.update_in(cx, |form, window, cx| {
                form.finish_oauth_sign_out(result, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish mcp oauth sign out failed");
            }
        });
        self.sign_out_task = Some(task);
        cx.notify();
    }

    fn finish_oauth_sign_out(
        &mut self,
        result: Result<McpOAuthSignOutRequest, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sign_out_task = None;
        let request = match result {
            Ok(request) => request,
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                push_settings_error(window, cx, title, err);
                cx.notify();
                return;
            }
        };

        if request.draft_only {
            self.draft_oauth_credential_keys
                .remove(&request.credential_key);
            if self.draft_oauth_credential_key.as_ref() == Some(&request.credential_key) {
                self.draft_oauth_credential_key = None;
            }
            state::mcp::runtime(cx).update(cx, |runtime, cx| {
                runtime.discard_draft_oauth_authorization(&self.draft_oauth_status_key, cx);
            });
            cx.notify();
            return;
        }

        self.clear_draft_oauth_authorization(cx);
        let server = state::config::read(cx, |config| {
            config.mcp_servers.get(&request.server_id).cloned()
        });
        let Some(server) = server else {
            let title = cx.global::<I18n>().t("mcp-notify-save-failed");
            push_settings_error(
                window,
                cx,
                title,
                format!("MCP server `{}` not found", request.server_id),
            );
            cx.notify();
            return;
        };
        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.finish_server_sign_out(request.server_id, server, window, cx);
        });
        cx.notify();
    }

    fn cleanup_draft_oauth_credentials(&mut self, cx: &mut Context<Self>) {
        self.clear_draft_oauth_authorization(cx);
        for credential_key in std::mem::take(&mut self.draft_oauth_credential_keys) {
            state::mcp_oauth::delete_credentials_detached(&credential_key, cx);
        }
    }

    fn clear_draft_oauth_authorization(&mut self, cx: &mut Context<Self>) {
        self.draft_oauth_credential_key = None;
        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.discard_draft_oauth_authorization(&self.draft_oauth_status_key, cx);
        });
    }

    fn draft_oauth_target(&self, cx: &App) -> Option<McpOAuthDialogTarget> {
        if !can_authorize_draft_oauth(&self.draft, self.mode.original_server_id(), cx) {
            return None;
        }
        let server_id = self.draft.server_id(self.mode.original_server_id(), cx);
        let server = self
            .draft
            .merge_into_config(self.original_config.as_ref(), cx);
        let credential_key = oauth_credential_key_for_server(&server_id, &server)?;
        let original_server_id = self.mode.original_server_id();
        let uses_original_oauth_credentials = original_server_id
            .and_then(|original_server_id| {
                self.original_config
                    .as_ref()
                    .and_then(|server| oauth_credential_key_for_server(original_server_id, server))
            })
            .as_ref()
            == Some(&credential_key);
        let is_saved_target =
            original_server_id == Some(server_id.as_str()) && uses_original_oauth_credentials;
        Some(McpOAuthDialogTarget {
            status_key: if is_saved_target {
                server_id.clone()
            } else {
                self.draft_oauth_status_key.clone()
            },
            server_id,
            server,
            credential_key,
            is_draft: !is_saved_target,
            cleanup_credentials: !uses_original_oauth_credentials,
        })
    }

    fn render_oauth_section(
        &self,
        labels: OAuthSectionLabels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = cx.entity().downgrade();
        let authorize_dialog = dialog.clone();
        let sign_out_dialog = dialog.clone();
        let enabled = self.draft.form.read(cx).oauth_enabled_value();
        let status = self.oauth_status(cx);
        let authorized = matches!(status, McpOAuthStatusSnapshot::Authorized { .. });
        let signing_in = matches!(status, McpOAuthStatusSnapshot::SigningIn);
        let signing_out = self.is_signing_out();
        let busy = self.is_busy(cx);
        let can_authorize =
            can_authorize_draft_oauth(&self.draft, self.mode.original_server_id(), cx);
        let status_text = oauth_status_text(
            &status,
            labels.authorized,
            labels.not_authorized,
            labels.signing_in,
            labels.authorization_required,
            labels.scope_upgrade_required,
            labels.failed,
        );
        let status_icon = oauth_status_icon(&status);
        let status_color = oauth_status_color(&status, cx);

        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted.opacity(0.25))
                    .p_3()
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::new(IconName::Shield)
                                    .with_size(px(18.))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .child(Label::new(labels.title).text_sm().font_medium())
                                    .child(
                                        Label::new(labels.description)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        Switch::new("mcp-dialog-oauth-enabled")
                            .checked(enabled)
                            .disabled(busy)
                            .on_click(move |checked, window, cx| {
                                let _ = dialog.update(cx, |dialog, cx| {
                                    dialog.set_oauth_enabled(*checked, window, cx);
                                });
                            }),
                    ),
            )
            .when(enabled, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .p_3()
                        .child(
                            h_flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(status_icon)
                                        .with_size(px(18.))
                                        .text_color(status_color),
                                )
                                .child(
                                    Label::new(status_text)
                                        .text_sm()
                                        .font_medium()
                                        .line_height(relative(1.35))
                                        .text_color(status_color),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .when(authorized, |this| {
                                    this.child(
                                        Button::new("mcp-dialog-oauth-sign-out")
                                            .icon(IconName::LogOut)
                                            .label(labels.sign_out.clone())
                                            .outline()
                                            .loading(signing_out)
                                            .disabled(!can_authorize || signing_in || busy)
                                            .on_click(move |_, window, cx| {
                                                let _ = sign_out_dialog.update(cx, |dialog, cx| {
                                                    dialog.sign_out_oauth(window, cx);
                                                });
                                            }),
                                    )
                                })
                                .child(
                                    Button::new(if authorized {
                                        "mcp-dialog-oauth-reauthorize"
                                    } else {
                                        "mcp-dialog-oauth-authorize"
                                    })
                                    .icon(if authorized {
                                        IconName::RefreshCcw
                                    } else {
                                        IconName::Shield
                                    })
                                    .label(if authorized {
                                        labels.reauthorize.clone()
                                    } else {
                                        labels.authorize.clone()
                                    })
                                    .loading(signing_in)
                                    .disabled(!can_authorize || signing_in || busy)
                                    .when(!authorized, |button| button.primary())
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = authorize_dialog.update(cx, |dialog, cx| {
                                                dialog.authorize_oauth(window, cx);
                                            });
                                        },
                                    ),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn oauth_status(&self, cx: &App) -> McpOAuthStatusSnapshot {
        if !self.draft.form.read(cx).oauth_enabled_value() {
            return McpOAuthStatusSnapshot::SignedOut;
        }
        let Some(target) = self.draft_oauth_target(cx) else {
            return McpOAuthStatusSnapshot::SignedOut;
        };
        if target.is_draft {
            if self.draft_oauth_credential_key.as_ref() == Some(&target.credential_key)
                && let Some(auth) = state::mcp::runtime(cx)
                    .read(cx)
                    .auth_status(&target.status_key)
            {
                return auth;
            }
            if !target.cleanup_credentials
                && let Some(original_server_id) = self.mode.original_server_id()
                && let Some(auth) = state::mcp::runtime(cx)
                    .read(cx)
                    .auth_status(original_server_id)
            {
                return auth;
            }
            return McpOAuthStatusSnapshot::SignedOut;
        }
        state::mcp::runtime(cx)
            .read(cx)
            .auth_status(&target.status_key)
            .unwrap_or(McpOAuthStatusSnapshot::SignedOut)
    }

    fn clear_form_errors(&self, cx: &mut Context<Self>) {
        self.draft.form.update(cx, |form, cx| {
            form.clear_all_errors(cx);
        });
    }

    fn apply_validation_errors(&self, errors: Vec<McpFormValidationError>, cx: &mut Context<Self>) {
        self.draft.form.update(cx, |form, cx| {
            form.clear_all_errors(cx);
            for error in errors {
                match error.field {
                    McpFormField::Form => {}
                    McpFormField::ServerId => form.apply_field_error(
                        McpServerFormField::ServerId,
                        mcp_field_error(McpServerFormField::ServerId.key(), &error),
                        cx,
                    ),
                    McpFormField::Command => form.apply_field_error(
                        McpServerFormField::Command,
                        mcp_field_error(McpServerFormField::Command.key(), &error),
                        cx,
                    ),
                    McpFormField::Argument { row_id } => form.apply_arg_value_error(
                        row_id,
                        mcp_field_error(McpArgRowFormField::Value.key(), &error),
                        cx,
                    ),
                    McpFormField::EnvKey { row_id } => form.apply_env_field_error(
                        row_id,
                        McpEnvRowFormField::Key,
                        mcp_field_error(McpEnvRowFormField::Key.key(), &error),
                        cx,
                    ),
                    McpFormField::EnvValue { row_id } => form.apply_env_field_error(
                        row_id,
                        McpEnvRowFormField::Value,
                        mcp_field_error(McpEnvRowFormField::Value.key(), &error),
                        cx,
                    ),
                    McpFormField::EnvVar { row_id } => form.apply_env_var_value_error(
                        row_id,
                        mcp_field_error(McpEnvVarRowFormField::Value.key(), &error),
                        cx,
                    ),
                    McpFormField::Cwd => form.apply_field_error(
                        McpServerFormField::Cwd,
                        mcp_field_error(McpServerFormField::Cwd.key(), &error),
                        cx,
                    ),
                    McpFormField::Url => form.apply_field_error(
                        McpServerFormField::Url,
                        mcp_field_error(McpServerFormField::Url.key(), &error),
                        cx,
                    ),
                    McpFormField::BearerTokenEnvVar => form.apply_field_error(
                        McpServerFormField::BearerTokenEnvVar,
                        mcp_field_error(McpServerFormField::BearerTokenEnvVar.key(), &error),
                        cx,
                    ),
                    McpFormField::HeaderName { row_id } => form.apply_header_field_error(
                        row_id,
                        McpHeaderRowFormField::Name,
                        mcp_field_error(McpHeaderRowFormField::Name.key(), &error),
                        cx,
                    ),
                    McpFormField::HeaderValue { row_id } => form.apply_header_field_error(
                        row_id,
                        McpHeaderRowFormField::Value,
                        mcp_field_error(McpHeaderRowFormField::Value.key(), &error),
                        cx,
                    ),
                    McpFormField::EnvHeaderName { row_id } => form.apply_env_header_field_error(
                        row_id,
                        McpEnvHeaderRowFormField::Name,
                        mcp_field_error(McpEnvHeaderRowFormField::Name.key(), &error),
                        cx,
                    ),
                    McpFormField::EnvHeaderVar { row_id } => form.apply_env_header_field_error(
                        row_id,
                        McpEnvHeaderRowFormField::EnvVar,
                        mcp_field_error(McpEnvHeaderRowFormField::EnvVar.key(), &error),
                        cx,
                    ),
                }
            }
        });
    }

    fn validation_summary_messages(&self, cx: &App) -> Vec<SharedString> {
        let form = self.draft.form.read(cx);
        let mut messages = Vec::new();
        messages.extend(field_error_messages(&form.server_id, cx));
        messages.extend(field_error_messages(&form.command, cx));
        messages.extend(field_error_messages(&form.cwd, cx));
        messages.extend(field_error_messages(&form.url, cx));
        messages.extend(field_error_messages(&form.bearer_token_env_var, cx));
        for item in form.args_items() {
            let store = item.item.store();
            let store = store.read(cx);
            messages.extend(field_error_messages(&store.value, cx));
        }
        for item in form.env_items() {
            let store = item.item.store();
            let store = store.read(cx);
            messages.extend(field_error_messages(&store.key, cx));
            messages.extend(field_error_messages(&store.value, cx));
        }
        for item in form.env_vars_items() {
            let store = item.item.store();
            let store = store.read(cx);
            messages.extend(field_error_messages(&store.value, cx));
        }
        for item in form.headers_items() {
            let store = item.item.store();
            let store = store.read(cx);
            messages.extend(field_error_messages(&store.name, cx));
            messages.extend(field_error_messages(&store.value, cx));
        }
        for item in form.env_headers_items() {
            let store = item.item.store();
            let store = store.read(cx);
            messages.extend(field_error_messages(&store.name, cx));
            messages.extend(field_error_messages(&store.env_var, cx));
        }
        messages
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
        let oauth_required_title = i18n.t("mcp-oauth-required-title");
        let oauth_required_description = i18n.t("mcp-oauth-required-description");
        let oauth_authorized = i18n.t("mcp-oauth-authorized");
        let oauth_not_authorized = i18n.t("mcp-oauth-not-authorized");
        let oauth_signing_in = i18n.t("mcp-oauth-signing-in");
        let oauth_authorization_required = i18n.t("mcp-oauth-authorization-required");
        let oauth_scope_upgrade_required = i18n.t("mcp-oauth-scope-upgrade-required");
        let oauth_failed = i18n.t("mcp-oauth-failed");
        let oauth_authorize = i18n.t("mcp-oauth-authorize");
        let oauth_reauthorize = i18n.t("mcp-oauth-reauthorize");
        let oauth_sign_out = i18n.t("mcp-oauth-sign-out");
        let (
            transport,
            server_id_input,
            server_id_required,
            command_input,
            command_required,
            cwd_input,
            cwd_required,
            url_input,
            url_required,
            bearer_token_env_var_input,
            bearer_token_env_var_required,
            server_id_errors,
            command_errors,
            cwd_errors,
            url_errors,
            bearer_token_env_var_errors,
        ) = {
            let form = self.draft.form.read(cx);
            (
                form.transport_value(),
                form.server_id_state(),
                form.server_id_required(),
                form.command_state(),
                form.command_required(),
                form.cwd_state(),
                form.cwd_required(),
                form.url_state(),
                form.url_required(),
                form.bearer_token_env_var_state(),
                form.bearer_token_env_var_required(),
                field_error_messages(&form.server_id, cx),
                field_error_messages(&form.command, cx),
                field_error_messages(&form.cwd, cx),
                field_error_messages(&form.url, cx),
                field_error_messages(&form.bearer_token_env_var, cx),
            )
        };
        let validation_summary_messages = self.validation_summary_messages(cx);
        let scroll_handle = self.content_scroll_handle.clone();

        div()
            .on_action(cx.listener(Self::on_add_mcp_row))
            .on_action(cx.listener(Self::on_remove_mcp_row))
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
                    .when(!validation_summary_messages.is_empty(), |this| {
                        this.child(render_validation_summary(validation_summary_messages, cx))
                    })
                    .child(form_field(
                        name_label,
                        Input::new(&server_id_input).w_full().into_any_element(),
                        server_id_errors,
                        server_id_required,
                        cx,
                    ))
                    .child(form_field(
                        transport_label,
                        self.render_transport_toggle(cx),
                        Vec::new(),
                        false,
                        cx,
                    ))
                    .when(transport == McpTransportKind::Stdio, |this| {
                        this.child(section_label(stdio_section_label, cx))
                            .child(form_field(
                                command_label,
                                Input::new(&command_input).w_full().into_any_element(),
                                command_errors,
                                command_required,
                                cx,
                            ))
                            .child(list_field_with_errors(
                                {
                                    let rows = {
                                        let form = self.draft.form.read(cx);
                                        form.args_items()
                                            .iter()
                                            .map(|item| {
                                                let store = item.item.store();
                                                let store = store.read(cx);
                                                (
                                                    item.id,
                                                    store.value_state(),
                                                    field_error_messages(&store.value, cx),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    one_input_rows(
                                        "mcp-dialog-args",
                                        args_label,
                                        rows,
                                        McpRowList::Args,
                                        add_arg_label,
                                        remove_label.clone(),
                                        cx,
                                    )
                                },
                                Vec::new(),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                {
                                    let rows = {
                                        let form = self.draft.form.read(cx);
                                        form.env_items()
                                            .iter()
                                            .map(|item| {
                                                let store = item.item.store();
                                                let store = store.read(cx);
                                                (
                                                    item.id,
                                                    store.key_state(),
                                                    store.value_state(),
                                                    [
                                                        field_error_messages(&store.key, cx),
                                                        field_error_messages(&store.value, cx),
                                                    ]
                                                    .concat(),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    two_input_rows(
                                        "mcp-dialog-env",
                                        env_label,
                                        rows,
                                        McpRowList::Env,
                                        add_env_label,
                                        remove_label.clone(),
                                        cx,
                                    )
                                },
                                Vec::new(),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                {
                                    let rows = {
                                        let form = self.draft.form.read(cx);
                                        form.env_vars_items()
                                            .iter()
                                            .map(|item| {
                                                let store = item.item.store();
                                                let store = store.read(cx);
                                                (
                                                    item.id,
                                                    store.value_state(),
                                                    field_error_messages(&store.value, cx),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    one_input_rows(
                                        "mcp-dialog-env-vars",
                                        env_vars_label,
                                        rows,
                                        McpRowList::EnvVars,
                                        add_env_var_label,
                                        remove_label.clone(),
                                        cx,
                                    )
                                },
                                Vec::new(),
                                cx,
                            ))
                            .child(form_field(
                                cwd_label,
                                Input::new(&cwd_input).w_full().into_any_element(),
                                cwd_errors,
                                cwd_required,
                                cx,
                            ))
                    })
                    .when(transport == McpTransportKind::StreamableHttp, |this| {
                        this.child(section_label(http_section_label, cx))
                            .child(form_field(
                                url_label,
                                Input::new(&url_input).w_full().into_any_element(),
                                url_errors,
                                url_required,
                                cx,
                            ))
                            .child(form_field(
                                bearer_token_env_var_label,
                                Input::new(&bearer_token_env_var_input)
                                    .w_full()
                                    .into_any_element(),
                                bearer_token_env_var_errors,
                                bearer_token_env_var_required,
                                cx,
                            ))
                            .child(list_field_with_errors(
                                {
                                    let rows = {
                                        let form = self.draft.form.read(cx);
                                        form.headers_items()
                                            .iter()
                                            .map(|item| {
                                                let store = item.item.store();
                                                let store = store.read(cx);
                                                (
                                                    item.id,
                                                    store.name_state(),
                                                    store.value_state(),
                                                    [
                                                        field_error_messages(&store.name, cx),
                                                        field_error_messages(&store.value, cx),
                                                    ]
                                                    .concat(),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    two_input_rows(
                                        "mcp-dialog-headers",
                                        headers_label,
                                        rows,
                                        McpRowList::Headers,
                                        add_header_label,
                                        remove_label.clone(),
                                        cx,
                                    )
                                },
                                Vec::new(),
                                cx,
                            ))
                            .child(list_field_with_errors(
                                {
                                    let rows = {
                                        let form = self.draft.form.read(cx);
                                        form.env_headers_items()
                                            .iter()
                                            .map(|item| {
                                                let store = item.item.store();
                                                let store = store.read(cx);
                                                (
                                                    item.id,
                                                    store.name_state(),
                                                    store.env_var_state(),
                                                    [
                                                        field_error_messages(&store.name, cx),
                                                        field_error_messages(&store.env_var, cx),
                                                    ]
                                                    .concat(),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    two_input_rows(
                                        "mcp-dialog-env-headers",
                                        env_headers_label,
                                        rows,
                                        McpRowList::EnvHeaders,
                                        add_env_header_label,
                                        remove_label.clone(),
                                        cx,
                                    )
                                },
                                Vec::new(),
                                cx,
                            ))
                            .child(self.render_oauth_section(
                                OAuthSectionLabels {
                                    title: oauth_required_title.into(),
                                    description: oauth_required_description.into(),
                                    authorized: oauth_authorized.into(),
                                    not_authorized: oauth_not_authorized.into(),
                                    signing_in: oauth_signing_in.into(),
                                    authorization_required: oauth_authorization_required.into(),
                                    scope_upgrade_required: oauth_scope_upgrade_required.into(),
                                    failed: oauth_failed.into(),
                                    authorize: oauth_authorize.into(),
                                    reauthorize: oauth_reauthorize.into(),
                                    sign_out: oauth_sign_out.into(),
                                },
                                cx,
                            ))
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

    window.open_dialog(cx, move |dialog, window, cx| {
        let dialog_height = (window.viewport_size().height - px(96.))
            .max(px(360.))
            .min(px(760.));
        let form_state = form.read(cx);
        let saving = form_state.is_saving(cx);
        let dialog_blocked = form_state.is_dialog_blocked(cx);
        dialog
            .title(title.clone())
            .w(px(720.))
            .h(dialog_height)
            .on_cancel({
                let form = form.clone();
                move |_, _window, cx| {
                    form.update(cx, |form, cx| {
                        if form.is_dialog_blocked(cx) {
                            false
                        } else {
                            form.cleanup_draft_oauth_credentials(cx);
                            true
                        }
                    })
                }
            })
            .on_ok({
                let form = form.clone();
                move |_, window, cx| confirm_mcp_server_edit_dialog(&form, window, cx)
            })
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("mcp-dialog-cancel")
                                .label(cancel_label.clone())
                                .disabled(dialog_blocked),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("mcp-dialog-save")
                                .primary()
                                .icon(IconName::Plug)
                                .label(save_label.clone())
                                .loading(saving)
                                .disabled(dialog_blocked),
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

async fn delete_oauth_credentials(
    credential_keys: &[state::mcp_oauth::CredentialsKey],
    cx: &mut gpui::AsyncWindowContext,
) -> Result<(), String> {
    for credential_key in credential_keys {
        state::mcp_oauth::delete_credentials(credential_key, cx).await?;
    }
    Ok(())
}

async fn delete_oauth_credentials_for_save(
    request: McpServerSaveRequest,
    cx: &mut gpui::AsyncWindowContext,
) -> Result<McpServerSaveRequest, String> {
    delete_oauth_credentials(&request.credential_keys_to_delete, cx).await?;
    Ok(request)
}

async fn delete_oauth_credentials_for_sign_out(
    request: McpOAuthSignOutRequest,
    cx: &mut gpui::AsyncWindowContext,
) -> Result<McpOAuthSignOutRequest, String> {
    state::mcp_oauth::delete_credentials(&request.credential_key, cx).await?;
    Ok(request)
}

fn oauth_credential_keys_to_delete(
    original_server_id: Option<&str>,
    original_config: Option<&McpServerTomlConfig>,
    saved_server_id: &str,
    saved_server: &McpServerTomlConfig,
    draft_credential_keys: &BTreeSet<state::mcp_oauth::CredentialsKey>,
    promoted_draft_key: Option<state::mcp_oauth::CredentialsKey>,
) -> Vec<state::mcp_oauth::CredentialsKey> {
    let mut keys = BTreeSet::new();
    let original_key = original_server_id
        .zip(original_config)
        .and_then(|(server_id, server)| oauth_credential_key_for_server(server_id, server));
    let saved_key = oauth_credential_key_for_server(saved_server_id, saved_server);

    if original_key.is_some() && original_key != saved_key {
        keys.extend(original_key);
    }
    for key in draft_credential_keys {
        if promoted_draft_key.as_ref() != Some(key) {
            keys.insert(key.clone());
        }
    }
    keys.into_iter().collect()
}

fn oauth_credential_key_for_server(
    server_id: &str,
    server: &McpServerTomlConfig,
) -> Option<state::mcp_oauth::CredentialsKey> {
    state::mcp_oauth::credentials_key_for_server(server_id, server)
        .ok()
        .flatten()
}

fn configured_oauth_status(server: &McpServerTomlConfig) -> McpOAuthStatusSnapshot {
    if server.transport == McpTransportKind::StreamableHttp && server.oauth.is_some() {
        McpOAuthStatusSnapshot::SignedOut
    } else {
        McpOAuthStatusSnapshot::NotConfigured
    }
}

pub(super) fn open_mcp_server_delete_confirm(
    server_id: String,
    page: WeakEntity<super::McpSettingsPage>,
    window: &mut Window,
    cx: &mut App,
) {
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
        move |window, cx| {
            let server_before_delete =
                state::config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
            let credential_keys_to_delete = server_before_delete
                .as_ref()
                .and_then(|server| oauth_credential_key_for_server(&server_id, server))
                .into_iter()
                .collect::<Vec<_>>();
            let server_id = server_id.clone();
            let deleted_title = deleted_title.clone();
            let delete_failed_title = delete_failed_title.clone();
            let page_for_task = page.clone();
            let task = window.spawn(cx, async move |cx| {
                let credentials_result =
                    delete_oauth_credentials(&credential_keys_to_delete, cx).await;
                if let Err(err) = cx.update(|window, cx| match credentials_result {
                    Ok(()) => match state::config::delete_mcp_server(cx, &server_id) {
                        Ok(_) => {
                            disconnect_server(server_id.clone(), window, cx);
                            window.push_notification(
                                Notification::new()
                                    .title(deleted_title.clone())
                                    .with_type(NotificationType::Success),
                                cx,
                            );
                        }
                        Err(err) => {
                            push_settings_error(window, cx, delete_failed_title.clone(), err)
                        }
                    },
                    Err(err) => push_settings_error(window, cx, delete_failed_title.clone(), err),
                }) {
                    event!(Level::ERROR, error = ?err, "finish mcp server delete failed");
                }
                if let Err(err) = page_for_task.update(cx, |page, cx| {
                    page.delete_task = None;
                    cx.notify();
                }) {
                    event!(Level::ERROR, error = ?err, "clear mcp server delete task failed");
                }
            });
            if let Err(err) = page.update(cx, |page, cx| {
                page.delete_task = Some(task);
                cx.notify();
            }) {
                event!(Level::ERROR, error = ?err, "store mcp server delete task failed");
            }
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

fn oauth_status_text(
    status: &McpOAuthStatusSnapshot,
    authorized_label: SharedString,
    not_authorized_label: SharedString,
    signing_in_label: SharedString,
    authorization_required_label: SharedString,
    scope_upgrade_required_label: SharedString,
    failed_label: SharedString,
) -> SharedString {
    match status {
        McpOAuthStatusSnapshot::Authorized { .. } => authorized_label,
        McpOAuthStatusSnapshot::SigningIn => signing_in_label,
        McpOAuthStatusSnapshot::AuthorizationRequired => authorization_required_label,
        McpOAuthStatusSnapshot::ScopeUpgradeRequired { required_scope, .. }
            if !required_scope.trim().is_empty() && required_scope != "unknown" =>
        {
            format!(
                "{}: {required_scope}",
                scope_upgrade_required_label.as_ref()
            )
            .into()
        }
        McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. } => scope_upgrade_required_label,
        McpOAuthStatusSnapshot::Failed { message } if !message.trim().is_empty() => {
            format!("{}: {message}", failed_label.as_ref()).into()
        }
        McpOAuthStatusSnapshot::Failed { .. } => failed_label,
        McpOAuthStatusSnapshot::NotConfigured | McpOAuthStatusSnapshot::SignedOut => {
            not_authorized_label
        }
    }
}

fn oauth_status_icon(status: &McpOAuthStatusSnapshot) -> IconName {
    match status {
        McpOAuthStatusSnapshot::Authorized { .. } => IconName::ShieldCheck,
        McpOAuthStatusSnapshot::SigningIn => IconName::RefreshCcw,
        McpOAuthStatusSnapshot::AuthorizationRequired
        | McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. }
        | McpOAuthStatusSnapshot::Failed { .. } => IconName::ShieldAlert,
        McpOAuthStatusSnapshot::NotConfigured | McpOAuthStatusSnapshot::SignedOut => {
            IconName::ShieldAlert
        }
    }
}

fn oauth_status_color(status: &McpOAuthStatusSnapshot, cx: &App) -> gpui::Hsla {
    match status {
        McpOAuthStatusSnapshot::Authorized { .. } => cx.theme().success,
        McpOAuthStatusSnapshot::Failed { .. } => cx.theme().danger,
        McpOAuthStatusSnapshot::SigningIn
        | McpOAuthStatusSnapshot::AuthorizationRequired
        | McpOAuthStatusSnapshot::ScopeUpgradeRequired { .. } => cx.theme().warning,
        McpOAuthStatusSnapshot::NotConfigured | McpOAuthStatusSnapshot::SignedOut => {
            cx.theme().muted_foreground
        }
    }
}

fn can_authorize_draft_oauth(
    draft: &McpServerFormDraft,
    original_server_id: Option<&str>,
    cx: &App,
) -> bool {
    let server_id = draft.server_id(original_server_id, cx);
    let input = draft.input(cx);
    can_authorize_oauth_values(
        input.transport,
        input.oauth_enabled,
        &server_id,
        input.url.trim(),
    )
}

fn oauth_status_after_save(
    draft_oauth_credential_key: Option<&state::mcp_oauth::CredentialsKey>,
    draft_oauth_status_key: &str,
    original_server_id: Option<&str>,
    original_config: Option<&McpServerTomlConfig>,
    server_id: &str,
    saved_server: &McpServerTomlConfig,
    cx: &App,
) -> McpOAuthStatusSnapshot {
    let saved_key = oauth_credential_key_for_server(server_id, saved_server);
    if saved_key.is_some()
        && draft_oauth_credential_key == saved_key.as_ref()
        && let Some(auth) = state::mcp::runtime(cx)
            .read(cx)
            .auth_status(draft_oauth_status_key)
    {
        return auth;
    }

    if saved_key.is_some()
        && let Some(original_server_id) = original_server_id
        && original_config
            .and_then(|server| oauth_credential_key_for_server(original_server_id, server))
            .as_ref()
            == saved_key.as_ref()
        && let Some(auth) = state::mcp::runtime(cx)
            .read(cx)
            .auth_status(original_server_id)
    {
        return auth;
    }

    configured_oauth_status(saved_server)
}

fn promoted_draft_oauth_key(
    draft_oauth_credential_key: Option<&state::mcp_oauth::CredentialsKey>,
    server_id: &str,
    saved_server: &McpServerTomlConfig,
) -> Option<state::mcp_oauth::CredentialsKey> {
    let saved_key = oauth_credential_key_for_server(server_id, saved_server)?;
    (draft_oauth_credential_key == Some(&saved_key)).then_some(saved_key)
}

fn can_authorize_oauth_values(
    transport: McpTransportKind,
    oauth_enabled: bool,
    server_id: &str,
    url: &str,
) -> bool {
    if transport != McpTransportKind::StreamableHttp || !oauth_enabled {
        return false;
    }
    if !is_valid_mcp_server_id(server_id.trim()) {
        return false;
    }
    url::Url::parse(url)
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
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
    required: bool,
    cx: &mut App,
) -> AnyElement {
    component_form_field()
        .label(label.into())
        .required(required)
        .child(
            v_flex()
                .w_full()
                .gap_2()
                .child(input)
                .when(!errors.is_empty(), |this| {
                    this.child(validation_error_list(errors, cx))
                }),
        )
        .into_any_element()
}

fn mcp_field_error(field: &'static str, error: &McpFormValidationError) -> FieldError {
    let mut field_error = FieldError::new_for_field(
        field,
        ValidationTrigger::Submit,
        ValidationSource::App("ai-chat2-mcp".into()),
        error.message_key,
        error.message_key,
    );
    for (key, value) in &error.args {
        field_error = field_error.with_param(*key, value.clone());
    }
    field_error
}

fn field_error_messages<Field>(field: &Field, cx: &App) -> Vec<SharedString>
where
    Field: FormField,
{
    field
        .visible_errors(&FormMeta::default())
        .into_iter()
        .map(|error| field_error_message(error, cx))
        .collect()
}

fn field_error_message(error: &FieldError, cx: &App) -> SharedString {
    let i18n = cx.global::<I18n>();
    if error.params.is_empty() {
        return i18n.t(error.message_key.as_ref()).into();
    }

    let mut args = FluentArgs::new();
    for (key, value) in &error.params {
        args.set(key.to_string(), error_param_value(value));
    }
    i18n.t_with_args(error.message_key.as_ref(), &args).into()
}

fn error_param_value(value: &ErrorParamValue) -> String {
    match value {
        ErrorParamValue::String(value) => value.to_string(),
        ErrorParamValue::Integer(value) => value.to_string(),
        ErrorParamValue::Unsigned(value) => value.to_string(),
        ErrorParamValue::Float(value) => value.to_string(),
        ErrorParamValue::Bool(value) => value.to_string(),
    }
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

fn render_validation_summary(messages: Vec<SharedString>, cx: &mut App) -> AnyElement {
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
        .children(messages.into_iter().map(|message| {
            Label::new(message)
                .text_xs()
                .line_height(relative(1.35))
                .text_color(cx.theme().danger)
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::{
        foundation, state,
        state::config::{McpOAuthTomlConfig, McpServerTomlConfig},
    };
    use ai_chat_agent::McpOAuthStatusSnapshot;
    use gpui::{AppContext as _, Entity, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::{InputEvent, InputState};
    use gpui_form::FormField as _;

    use super::{
        McpServerEditDialogState, McpServerEditMode, McpTransportKind, can_authorize_oauth_values,
        oauth_credential_key_for_server, oauth_credential_keys_to_delete,
        transport_from_toggle_states,
    };

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

    #[test]
    fn oauth_authorization_can_start_from_unsaved_http_draft() {
        assert!(can_authorize_oauth_values(
            McpTransportKind::StreamableHttp,
            true,
            "github",
            "https://example.com/mcp",
        ));
        assert!(!can_authorize_oauth_values(
            McpTransportKind::Stdio,
            true,
            "github",
            "https://example.com/mcp",
        ));
        assert!(!can_authorize_oauth_values(
            McpTransportKind::StreamableHttp,
            false,
            "github",
            "https://example.com/mcp",
        ));
        assert!(!can_authorize_oauth_values(
            McpTransportKind::StreamableHttp,
            true,
            "",
            "https://example.com/mcp",
        ));
        assert!(!can_authorize_oauth_values(
            McpTransportKind::StreamableHttp,
            true,
            "bad id",
            "https://example.com/mcp",
        ));
        assert!(!can_authorize_oauth_values(
            McpTransportKind::StreamableHttp,
            true,
            "github",
            "file:///tmp/mcp",
        ));
    }

    #[test]
    fn oauth_save_deletes_stale_credentials_but_keeps_promoted_draft() {
        let server_id = "server";
        let original = oauth_server("https://old.example.com/mcp");
        let saved = oauth_server("https://new.example.com/mcp");
        let original_key = oauth_credential_key_for_server(server_id, &original).unwrap();
        let saved_key = oauth_credential_key_for_server(server_id, &saved).unwrap();
        let unused_draft_key = oauth_credential_key_for_server(
            server_id,
            &oauth_server("https://unused.example.com/mcp"),
        )
        .unwrap();
        let draft_keys = BTreeSet::from([saved_key.clone(), unused_draft_key.clone()]);

        let keys = oauth_credential_keys_to_delete(
            Some(server_id),
            Some(&original),
            server_id,
            &saved,
            &draft_keys,
            Some(saved_key),
        );

        assert_eq!(
            keys.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([original_key, unused_draft_key])
        );
    }

    #[gpui::test]
    fn oauth_signing_in_blocks_save(cx: &mut TestAppContext) {
        init_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            let server_id = "server".to_string();
            let server = oauth_server("https://example.com/mcp");
            state::mcp::runtime(cx).update(cx, |runtime, cx| {
                runtime.replace_saved_server_status(
                    server_id.clone(),
                    &server,
                    McpOAuthStatusSnapshot::SigningIn,
                    cx,
                );
            });

            let form = cx.new(|cx| {
                McpServerEditDialogState::new(
                    McpServerEditMode::Edit {
                        original_server_id: server_id,
                    },
                    Some(server),
                    window,
                    cx,
                )
            });

            assert!(form.read(cx).is_oauth_signing_in(cx));
            assert!(form.read(cx).is_dialog_blocked(cx));
            assert!(!form.update(cx, |form, cx| form.save(window, cx)));
            assert!(!form.read(cx).draft.form.read(cx).is_submitting());
        });
    }

    #[gpui::test]
    fn save_validation_errors_are_applied_to_form_fields(cx: &mut TestAppContext) {
        init_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx.update(|window, cx| {
            cx.new(|cx| McpServerEditDialogState::new(McpServerEditMode::Create, None, window, cx))
        });
        let arg_input = cx.update(|_, cx| {
            form.read(cx).draft.form.read(cx).args_items()[0]
                .item
                .store()
                .read(cx)
                .value_state()
        });
        set_input_value(arg_input, "   ", &mut cx);

        cx.update(|window, cx| {
            assert!(!form.update(cx, |form, cx| form.save(window, cx)));

            let dialog = form.read(cx);
            let draft_form = dialog.draft.form.read(cx);
            assert_eq!(
                draft_form
                    .server_id
                    .errors()
                    .first()
                    .map(|error| error.message_key.as_ref()),
                Some("mcp-validation-name-required")
            );
            assert_eq!(
                draft_form
                    .command
                    .errors()
                    .first()
                    .map(|error| error.message_key.as_ref()),
                Some("mcp-validation-command-required")
            );

            let arg_store = draft_form.args_items()[0].item.store();
            assert_eq!(
                arg_store
                    .read(cx)
                    .value
                    .errors()
                    .first()
                    .map(|error| error.message_key.as_ref()),
                Some("mcp-validation-arg-empty")
            );
            assert!(
                draft_form
                    .command
                    .errors()
                    .iter()
                    .any(gpui_form::FieldError::is_error)
            );
            assert!(dialog.validation_summary_messages(cx).len() >= 3);
        });
    }

    fn oauth_server(url: &str) -> McpServerTomlConfig {
        McpServerTomlConfig {
            transport: McpTransportKind::StreamableHttp,
            url: Some(url.to_string()),
            oauth: Some(McpOAuthTomlConfig::AuthorizationCodePkce {
                scopes: Vec::new(),
                client_id: None,
                client_metadata_url: None,
                resource: None,
                callback_port: None,
                callback_url: None,
            }),
            ..Default::default()
        }
    }

    fn init_dialog_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            foundation::init_i18n(cx);
            state::config::install_for_test(cx, state::AiChat2Config::default())
                .expect("install config store");
            state::mcp::init(cx).expect("init MCP runtime");
        });
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<gpui_component::Root> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| TestView);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("open mcp dialog test window")
        })
    }

    fn set_input_value(input: Entity<InputState>, value: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
                cx.emit(InputEvent::Change);
            });
        });
    }

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }
}
