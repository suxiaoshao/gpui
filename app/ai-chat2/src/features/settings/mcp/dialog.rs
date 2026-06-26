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
    Task, Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::Input,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};
use tracing::{Level, event};

use super::{
    super::push_settings_error,
    form_rows::{render_key_value_list_field, render_string_list_field, validation_error_list},
    form_state::{KeyValueField, McpServerFormDraft, StringListField, trim_input},
    validation::{McpFormField, McpFormValidationError, validate_mcp_form},
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
    validation_errors: Vec<McpFormValidationError>,
    next_row_id: u64,
    content_scroll_handle: ScrollHandle,
    draft_oauth_status_key: String,
    draft_oauth_status_url: Option<String>,
    draft_oauth_credential_urls: BTreeSet<String>,
    save_task: Option<Task<()>>,
    sign_out_task: Option<Task<()>>,
}

struct McpOAuthDialogTarget {
    status_key: String,
    server_id: String,
    server: McpServerTomlConfig,
    server_url: String,
    is_draft: bool,
    cleanup_credentials: bool,
}

struct McpServerSaveRequest {
    original_server_id: Option<String>,
    server_id: String,
    server: McpServerTomlConfig,
    saved_auth: McpOAuthStatusSnapshot,
    credential_urls_to_delete: Vec<String>,
    success_title_key: &'static str,
}

struct McpOAuthSignOutRequest {
    server_id: String,
    server_url: String,
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
            draft_oauth_status_key: format!(
                "__mcp_oauth_draft_{}",
                NEXT_DRAFT_OAUTH_KEY.fetch_add(1, Ordering::Relaxed)
            ),
            draft_oauth_status_url: None,
            draft_oauth_credential_urls: BTreeSet::new(),
            save_task: None,
            sign_out_task: None,
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

    fn is_saving(&self) -> bool {
        self.save_task.is_some()
    }

    fn is_signing_out(&self) -> bool {
        self.sign_out_task.is_some()
    }

    fn is_busy(&self) -> bool {
        self.is_saving() || self.is_signing_out()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.is_busy() {
            return false;
        }
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
        let saved_server = server.clone();
        let saved_auth = self.oauth_status_after_save(&server_id, &saved_server, cx);
        let credential_urls_to_delete = oauth_credential_urls_to_delete(
            self.original_config.as_ref(),
            &saved_server,
            &self.draft_oauth_credential_urls,
            self.promoted_draft_oauth_url(&saved_server),
        );
        let request = McpServerSaveRequest {
            original_server_id,
            server_id,
            server,
            saved_auth,
            credential_urls_to_delete,
            success_title_key: match &self.mode {
                McpServerEditMode::Create => "mcp-notify-server-created",
                McpServerEditMode::Edit { .. } => "mcp-notify-server-saved",
            },
        };

        let form = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = delete_oauth_credentials_for_save(request, cx).await;
            if let Err(err) = form.update_in(cx, |form, window, cx| {
                form.finish_save(result, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish mcp server save failed");
            }
        });
        self.save_task = Some(task);
        cx.notify();
        false
    }

    fn finish_save(
        &mut self,
        result: Result<McpServerSaveRequest, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_task = None;
        let request = match result {
            Ok(request) => request,
            Err(err) => {
                let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                push_settings_error(window, cx, title, err);
                cx.notify();
                return;
            }
        };

        let saved_server = request.server.clone();
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
                self.finish_oauth_after_save(&saved_server, request.saved_auth, cx);
                self.validation_errors.clear();
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
                push_settings_error(window, cx, title, err);
            }
        }
        cx.notify();
    }

    fn oauth_status_after_save(
        &self,
        server_id: &str,
        saved_server: &McpServerTomlConfig,
        cx: &App,
    ) -> McpOAuthStatusSnapshot {
        let saved_url = saved_server.url.as_deref();
        if saved_server.oauth.is_some()
            && self.draft_oauth_status_url.as_deref() == saved_url
            && let Some(auth) = state::mcp::runtime(cx)
                .read(cx)
                .auth_status(&self.draft_oauth_status_key)
        {
            return auth;
        }

        if saved_server.oauth.is_some()
            && self
                .original_config
                .as_ref()
                .is_some_and(|server| server.oauth.is_some() && server.url.as_deref() == saved_url)
        {
            let status_key = self.mode.original_server_id().unwrap_or(server_id);
            if let Some(auth) = state::mcp::runtime(cx).read(cx).auth_status(status_key) {
                return auth;
            }
        }

        configured_oauth_status(saved_server)
    }

    fn promoted_draft_oauth_url<'a>(
        &self,
        saved_server: &'a McpServerTomlConfig,
    ) -> Option<&'a str> {
        let saved_url = saved_server.url.as_deref()?;
        (saved_server.oauth.is_some() && self.draft_oauth_status_url.as_deref() == Some(saved_url))
            .then_some(saved_url)
    }

    fn finish_oauth_after_save(
        &mut self,
        saved_server: &McpServerTomlConfig,
        saved_auth: McpOAuthStatusSnapshot,
        cx: &mut Context<Self>,
    ) {
        let server_id = self.draft.server_id(self.mode.original_server_id(), cx);
        let saved_url = saved_server.url.as_deref();
        let promote_draft =
            saved_server.oauth.is_some() && self.draft_oauth_status_url.as_deref() == saved_url;

        if promote_draft {
            state::mcp::runtime(cx).update(cx, |runtime, cx| {
                runtime.promote_draft_oauth_authorization(
                    &self.draft_oauth_status_key,
                    server_id.clone(),
                    saved_server.clone(),
                    cx,
                );
            });
            if let Some(url) = saved_url {
                self.draft_oauth_credential_urls.remove(url);
            }
        } else {
            self.clear_draft_oauth_authorization(cx);
        }

        state::mcp::runtime(cx).update(cx, |runtime, cx| {
            runtime.replace_saved_server_status(server_id, saved_server, saved_auth, cx);
        });

        self.draft_oauth_credential_urls.clear();
        self.draft_oauth_status_url = None;
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

    fn set_oauth_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.draft.set_oauth_enabled(enabled);
        self.validation_errors.clear();
        cx.notify();
    }

    fn authorize_oauth(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.draft_oauth_target(cx) else {
            return;
        };
        if target.is_draft {
            if self.draft_oauth_status_url.as_deref() != Some(target.server_url.as_str()) {
                state::mcp::runtime(cx).update(cx, |runtime, cx| {
                    runtime.discard_draft_oauth_authorization(&self.draft_oauth_status_key, cx);
                });
            }
            self.draft_oauth_status_url = Some(target.server_url.clone());
            if target.cleanup_credentials {
                self.draft_oauth_credential_urls
                    .insert(target.server_url.clone());
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
        if self.is_busy() {
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
            server_url: target.server_url,
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
            self.draft_oauth_credential_urls.remove(&request.server_url);
            if self.draft_oauth_status_url.as_deref() == Some(request.server_url.as_str()) {
                self.draft_oauth_status_url = None;
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
        for server_url in std::mem::take(&mut self.draft_oauth_credential_urls) {
            let _ = state::mcp_oauth::delete_credentials_detached(&server_url, cx);
        }
    }

    fn clear_draft_oauth_authorization(&mut self, cx: &mut Context<Self>) {
        self.draft_oauth_status_url = None;
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
        let server_url = server.url.clone()?;
        let original_server_id = self.mode.original_server_id();
        let uses_original_oauth_url = self.original_config.as_ref().is_some_and(|server| {
            server.oauth.is_some() && server.url.as_deref() == Some(server_url.as_str())
        });
        let is_saved_target =
            original_server_id == Some(server_id.as_str()) && uses_original_oauth_url;
        Some(McpOAuthDialogTarget {
            status_key: if is_saved_target {
                server_id.clone()
            } else {
                self.draft_oauth_status_key.clone()
            },
            server_id,
            server,
            server_url,
            is_draft: !is_saved_target,
            cleanup_credentials: !uses_original_oauth_url,
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
        let enabled = self.draft.oauth_enabled;
        let status = self.oauth_status(cx);
        let authorized = matches!(status, McpOAuthStatusSnapshot::Authorized { .. });
        let signing_in = matches!(status, McpOAuthStatusSnapshot::SigningIn);
        let signing_out = self.is_signing_out();
        let busy = self.is_busy();
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
                            .on_click(move |checked, _window, cx| {
                                let _ = dialog.update(cx, |dialog, cx| {
                                    dialog.set_oauth_enabled(*checked, cx);
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
        if !self.draft.oauth_enabled {
            return McpOAuthStatusSnapshot::SignedOut;
        }
        let Some(target) = self.draft_oauth_target(cx) else {
            return McpOAuthStatusSnapshot::SignedOut;
        };
        if target.is_draft {
            if self.draft_oauth_status_url.as_deref() == Some(target.server_url.as_str())
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
        let saving = form_state.is_saving();
        let busy = form_state.is_busy();
        dialog
            .title(title.clone())
            .w(px(720.))
            .h(dialog_height)
            .on_cancel({
                let form = form.clone();
                move |_, _window, cx| {
                    form.update(cx, |form, cx| {
                        if form.is_busy() {
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
                                .disabled(busy),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("mcp-dialog-save")
                                .primary()
                                .icon(IconName::Plug)
                                .label(save_label.clone())
                                .loading(saving)
                                .disabled(busy),
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
    server_urls: &[String],
    cx: &mut gpui::AsyncWindowContext,
) -> Result<(), String> {
    for server_url in server_urls {
        state::mcp_oauth::delete_credentials(server_url, cx).await?;
    }
    Ok(())
}

async fn delete_oauth_credentials_for_save(
    request: McpServerSaveRequest,
    cx: &mut gpui::AsyncWindowContext,
) -> Result<McpServerSaveRequest, String> {
    delete_oauth_credentials(&request.credential_urls_to_delete, cx).await?;
    Ok(request)
}

async fn delete_oauth_credentials_for_sign_out(
    request: McpOAuthSignOutRequest,
    cx: &mut gpui::AsyncWindowContext,
) -> Result<McpOAuthSignOutRequest, String> {
    state::mcp_oauth::delete_credentials(&request.server_url, cx).await?;
    Ok(request)
}

fn oauth_credential_urls_to_delete(
    original_config: Option<&McpServerTomlConfig>,
    saved_server: &McpServerTomlConfig,
    draft_credential_urls: &BTreeSet<String>,
    promoted_draft_url: Option<&str>,
) -> Vec<String> {
    let mut urls = BTreeSet::new();
    let original_url = original_config.and_then(|server| server.url.as_deref());
    let saved_url = saved_server.url.as_deref();
    let original_oauth_enabled = original_config.is_some_and(|server| server.oauth.is_some());
    let oauth_disabled = original_oauth_enabled && saved_server.oauth.is_none();
    let oauth_url_changed =
        original_oauth_enabled && saved_server.oauth.is_some() && original_url != saved_url;
    if (oauth_disabled || oauth_url_changed)
        && let Some(url) = original_url
    {
        urls.insert(url.to_string());
    }
    if oauth_disabled
        && let Some(url) = saved_url
        && Some(url) != original_url
    {
        urls.insert(url.to_string());
    }
    for url in draft_credential_urls {
        if promoted_draft_url != Some(url.as_str()) {
            urls.insert(url.clone());
        }
    }
    urls.into_iter().collect()
}

fn configured_oauth_status(server: &McpServerTomlConfig) -> McpOAuthStatusSnapshot {
    if server.transport == McpTransportKind::StreamableHttp && server.oauth.is_some() {
        McpOAuthStatusSnapshot::SignedOut
    } else {
        McpOAuthStatusSnapshot::NotConfigured
    }
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
        move |window, cx| {
            let server_before_delete =
                state::config::read(cx, |config| config.mcp_servers.get(&server_id).cloned());
            let credential_urls_to_delete = server_before_delete
                .as_ref()
                .filter(|server| server.oauth.is_some())
                .and_then(|server| server.url.clone())
                .into_iter()
                .collect::<Vec<_>>();
            let server_id = server_id.clone();
            let deleted_title = deleted_title.clone();
            let delete_failed_title = delete_failed_title.clone();
            let task = window.spawn(cx, async move |cx| {
                let credentials_result =
                    delete_oauth_credentials(&credential_urls_to_delete, cx).await;
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
            });
            task.detach();
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
    let url = trim_input(&draft.url_input, cx);
    can_authorize_oauth_values(draft.transport, draft.oauth_enabled, &server_id, &url)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::state::config::{McpOAuthTomlConfig, McpServerTomlConfig};

    use super::{
        McpTransportKind, can_authorize_oauth_values, oauth_credential_urls_to_delete,
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
        let original = oauth_server("https://old.example.com/mcp");
        let saved = oauth_server("https://new.example.com/mcp");
        let draft_urls = BTreeSet::from([
            "https://new.example.com/mcp".to_string(),
            "https://unused.example.com/mcp".to_string(),
        ]);

        let urls = oauth_credential_urls_to_delete(
            Some(&original),
            &saved,
            &draft_urls,
            saved.url.as_deref(),
        );

        assert_eq!(
            urls,
            vec![
                "https://old.example.com/mcp".to_string(),
                "https://unused.example.com/mcp".to_string()
            ]
        );
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
}
