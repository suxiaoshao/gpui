use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    errors::{JacoError, JacoResult},
    foundation::{I18n, assets::IconName},
    state,
    state::config::{McpServerTomlConfig, McpTransportKind, is_valid_mcp_server_id},
};
use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled,
    Subscription, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    input::{Input, InputContentType},
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use gpui_form::{
    DynamicPath, FormEvent, FormSchema, FormVersion, GardeValidator, ItemPath, ModelChange,
    MutationError, ResolveError, TotalItemsPath, ValidationTrigger,
};
use jaco_agent::McpOAuthStatusSnapshot;
use std::{
    collections::BTreeSet,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
use tracing::{Level, event};

use super::{
    super::push_settings_error,
    form_rows::{
        AddMcpRow, McpRowList, MoveRowHandler, RemoveRowHandler, RowMoveHandlers, one_input_rows,
        two_input_rows, validation_error_list,
    },
    form_state::{
        McpArgRowInput, McpCollectionImpact, McpEnvHeaderRowInput, McpEnvRowInput,
        McpEnvVarRowInput, McpHeaderRowInput, McpServerFormComponents, McpServerFormDraft,
        McpServerFormInput,
    },
    validation::{mcp_validation_context, normalize_mcp_input},
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
    components: McpServerFormComponents,
    content_scroll_handle: ScrollHandle,
    draft_oauth_status_key: String,
    draft_oauth_credential_key: Option<state::mcp::oauth::CredentialsKey>,
    draft_oauth_credential_keys: BTreeSet<state::mcp::oauth::CredentialsKey>,
    sign_out_task: Option<Task<()>>,
    save_task: Option<Task<()>>,
    _form_subscription: Subscription,
}

struct McpOAuthDialogTarget {
    status_key: String,
    server_id: String,
    server: McpServerTomlConfig,
    credential_key: state::mcp::oauth::CredentialsKey,
    is_draft: bool,
    cleanup_credentials: bool,
}

#[derive(Clone)]
struct McpServerSaveRequest {
    version: FormVersion,
    output: McpServerFormInput,
    original_server_id: Option<String>,
    expected_original: Option<McpServerTomlConfig>,
    server_id: String,
    server: McpServerTomlConfig,
    saved_auth: McpOAuthStatusSnapshot,
    credential_keys_to_delete: Vec<state::mcp::oauth::CredentialsKey>,
    success_title_key: &'static str,
}

struct McpOAuthSignOutRequest {
    server_id: String,
    credential_key: state::mcp::oauth::CredentialsKey,
    draft_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialCleanupOutcome {
    Complete,
    WarnOnly,
}

fn credential_cleanup_plan<T, E>(config_result: &Result<(), E>, keys: Vec<T>) -> Option<Vec<T>> {
    (config_result.is_ok() && !keys.is_empty()).then_some(keys)
}

fn credential_cleanup_outcome(failure_count: usize) -> CredentialCleanupOutcome {
    if failure_count == 0 {
        CredentialCleanupOutcome::Complete
    } else {
        CredentialCleanupOutcome::WarnOnly
    }
}

type RemoveDraftRow<Row> = fn(
    &mut McpServerFormDraft,
    ItemPath<McpServerFormInput, Row>,
    &mut Window,
    &mut App,
) -> Result<bool, MutationError>;

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

fn mcp_collection_impact(change: &ModelChange<McpServerFormInput>) -> McpCollectionImpact {
    fn changed(impact: gpui_form::PathImpact) -> bool {
        impact.structure_changed() || impact.retired()
    }

    McpCollectionImpact {
        args: changed(change.impact(&McpServerFormInput::ROOT.then(McpServerFormInput::ARGS))),
        env: changed(change.impact(&McpServerFormInput::ROOT.then(McpServerFormInput::ENV))),
        env_vars: changed(
            change.impact(&McpServerFormInput::ROOT.then(McpServerFormInput::ENV_VARS)),
        ),
        headers: changed(
            change.impact(&McpServerFormInput::ROOT.then(McpServerFormInput::HEADERS)),
        ),
        env_headers: changed(
            change.impact(&McpServerFormInput::ROOT.then(McpServerFormInput::ENV_HEADERS)),
        ),
    }
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
        let components = McpServerFormComponents::try_bind(&draft.form, window, cx)
            .expect("fresh MCP form rows have unique stable ids");
        let dialog = cx.entity().downgrade();
        let dialog_window = window.window_handle();
        let form_subscription = App::subscribe(
            cx,
            &draft.form,
            move |form, event: &FormEvent<McpServerFormInput>, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                let impact = mcp_collection_impact(change);
                if impact.is_empty() {
                    return;
                }
                let dialog = dialog.clone();
                cx.defer(move |cx| {
                    let Some(dialog) = dialog.upgrade() else {
                        return;
                    };
                    let _ = dialog_window.update(cx, |_, window, cx| {
                        dialog.update(cx, |dialog, cx| {
                            if let Err(error) = dialog.components.reconcile(
                                &form,
                                impact,
                                window,
                                cx,
                            ) {
                                event!(Level::ERROR, error = ?error, "reconcile MCP form rows failed");
                                return;
                            }
                            cx.notify();
                        });
                    });
                });
            },
        );

        Self {
            mode,
            original_config: server,
            draft,
            components,
            content_scroll_handle: ScrollHandle::default(),
            draft_oauth_status_key: format!(
                "__mcp_oauth_draft_{}",
                NEXT_DRAFT_OAUTH_KEY.fetch_add(1, Ordering::Relaxed)
            ),
            draft_oauth_credential_key: None,
            draft_oauth_credential_keys: BTreeSet::new(),
            sign_out_task: None,
            save_task: None,
            _form_subscription: form_subscription,
        }
    }

    fn focus_primary_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if !self.mode.is_edit() {
            self.components.server_id.clone()
        } else {
            match McpServerFormInput::TRANSPORT.get(&self.draft.form, cx) {
                McpTransportKind::Stdio => self.components.command.clone(),
                McpTransportKind::StreamableHttp => self.components.url.clone(),
            }
        };
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn is_saving(&self, cx: &App) -> bool {
        let _ = cx;
        self.save_task.is_some()
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

        let original_config = self.original_config.clone();
        let draft_oauth_credential_key = self.draft_oauth_credential_key.clone();
        let draft_oauth_credential_keys = self.draft_oauth_credential_keys.clone();
        let draft_oauth_status_key = self.draft_oauth_status_key.clone();
        let success_title_key = match &self.mode {
            McpServerEditMode::Create => "mcp-notify-server-created",
            McpServerEditMode::Edit { .. } => "mcp-notify-server-saved",
        };
        let prepared = self.draft.form.update(cx, |form_store, cx| {
            form_store.replace_validator(
                GardeValidator::<
                    McpServerFormInput,
                    crate::features::settings::form_validation::JacoGardeMessageProvider,
                >::new(mcp_validation_context(
                    original_server_id.clone(),
                    existing_server_ids.clone(),
                )),
                cx,
            );
            form_store
                .prepare(cx)
                .map(|prepared| prepared.map(|value| normalize_mcp_input(&value)))
        });
        let Ok(prepared) = prepared else {
            return false;
        };
        let (version, output) = prepared.into_parts();
        let server_id = output.server_id(original_server_id.as_deref());
        let server = output.clone().merge_into_config(original_config.as_ref());
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
            version,
            output,
            original_server_id,
            expected_original: original_config,
            server_id,
            server,
            saved_auth,
            credential_keys_to_delete,
            success_title_key,
        };
        let result = state::config::upsert_mcp_server_if_unchanged(
            cx,
            request.original_server_id.as_deref(),
            request.expected_original.as_ref(),
            request.server_id.clone(),
            request.server.clone(),
        );
        let _ = self.finish_config_save(request, result, window, cx);
        false
    }

    fn finish_config_save(
        &mut self,
        request: McpServerSaveRequest,
        result: JacoResult<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let saved_server = request.server.clone();
        let saved_server_id = request.server_id.clone();
        let credential_cleanup =
            credential_cleanup_plan(&result, request.credential_keys_to_delete.clone());
        match result {
            Ok(()) => {
                let rebased = self.draft.form.update(cx, |form, cx| {
                    form.rebase_if_current(request.version, request.output, cx)
                });
                if let Some(original_server_id) = request.original_server_id {
                    disconnect_server(original_server_id, window, cx);
                }
                self.finish_oauth_after_save(
                    &saved_server_id,
                    &saved_server,
                    request.saved_auth,
                    cx,
                );
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t(request.success_title_key))
                        .with_type(NotificationType::Success),
                    cx,
                );
                if rebased {
                    window.close_dialog(cx);
                } else {
                    self.mode = McpServerEditMode::Edit {
                        original_server_id: saved_server_id,
                    };
                    self.original_config = Some(saved_server);
                    cx.notify();
                }
                if let Some(credential_keys) = credential_cleanup {
                    self.schedule_oauth_cleanup(credential_keys, window, cx);
                }
            }
            Err(err) => {
                let message = err.to_string();
                if matches!(err, JacoError::ConfigEditConflict(_)) {
                    window.push_notification(
                        Notification::new()
                            .title(cx.global::<I18n>().t("mcp-notify-save-conflict-title"))
                            .message(cx.global::<I18n>().t("mcp-notify-save-conflict-message"))
                            .with_type(NotificationType::Error),
                        cx,
                    );
                } else {
                    let title = cx.global::<I18n>().t("mcp-notify-save-failed");
                    push_settings_error(window, cx, title, err);
                }
                cx.notify();
                return Err(message);
            }
        }
        cx.notify();
        Ok(())
    }

    fn schedule_oauth_cleanup(
        &self,
        credential_keys: Vec<state::mcp::oauth::CredentialsKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if credential_keys.is_empty() {
            return;
        }
        let credential_count = credential_keys.len();
        let window_handle = window.window_handle();
        state::mcp::oauth::schedule_credential_cleanup(
            credential_keys,
            move |result, cx| {
                if credential_cleanup_outcome(result.failure_count)
                    != CredentialCleanupOutcome::WarnOnly
                {
                    return;
                }
                event!(
                    Level::ERROR,
                    credential_count,
                    failure_count = result.failure_count,
                    "MCP OAuth credential cleanup failed after config commit"
                );
                let _ = window_handle.update(cx, |_, window, cx| {
                    window.push_notification(
                        Notification::new()
                            .title(
                                cx.global::<I18n>()
                                    .t("mcp-notify-credential-cleanup-failed"),
                            )
                            .with_type(NotificationType::Warning),
                        cx,
                    );
                });
            },
            cx,
        );
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
        let transport = McpServerFormInput::TRANSPORT.get(&self.draft.form, cx);
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
                    McpServerFormInput::TRANSPORT.get(&this.draft.form, cx),
                    states,
                );
                this.draft.set_transport(transport, window, cx);
                this.revalidate_form(ValidationTrigger::Change, window, cx);
                cx.notify();
            }))
            .into_any_element()
    }

    fn on_add_mcp_row(&mut self, action: &AddMcpRow, window: &mut Window, cx: &mut Context<Self>) {
        let result = match action.list {
            McpRowList::Args => self.draft.add_arg_row(window, cx),
            McpRowList::Env => self.draft.add_env_row(window, cx),
            McpRowList::EnvVars => self.draft.add_env_var_row(window, cx),
            McpRowList::Headers => self.draft.add_header_row(window, cx),
            McpRowList::EnvHeaders => self.draft.add_env_header_row(window, cx),
        };
        if let Err(error) = result {
            event!(Level::ERROR, error = ?error, "add MCP form row failed");
            return;
        }
        self.revalidate_form(ValidationTrigger::Change, window, cx);
        cx.notify();
    }

    fn remove_row_handler<Row>(
        row: ItemPath<McpServerFormInput, Row>,
        remove: RemoveDraftRow<Row>,
        cx: &mut Context<Self>,
    ) -> RemoveRowHandler
    where
        Row: FormSchema,
    {
        let dialog = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let row = row.clone();
            let _ = dialog.update(cx, |dialog, cx| {
                match remove(&mut dialog.draft, row, window, cx) {
                    Ok(false) => {
                        event!(Level::DEBUG, "ignored stale MCP row removal callback");
                    }
                    Ok(true) => {
                        dialog.revalidate_form(ValidationTrigger::Change, window, cx);
                        cx.notify();
                    }
                    Err(error) => {
                        event!(Level::ERROR, error = ?error, "remove MCP form row failed");
                    }
                }
            });
        })
    }

    fn row_move_handlers<Row>(
        collection: TotalItemsPath<McpServerFormInput, Row>,
        rows: &[ItemPath<McpServerFormInput, Row>],
        index: usize,
        cx: &mut Context<Self>,
    ) -> RowMoveHandlers
    where
        Row: FormSchema,
    {
        let current = rows[index].clone();
        let up = index.checked_sub(1).map(|previous| {
            Self::move_row_handler(
                collection.clone(),
                current.clone(),
                rows[previous].clone(),
                cx,
            )
        });
        let down = rows
            .get(index + 1)
            .map(|next| Self::move_row_handler(collection, next.clone(), current.clone(), cx));
        RowMoveHandlers { up, down }
    }

    fn move_row_handler<Row>(
        collection: TotalItemsPath<McpServerFormInput, Row>,
        row: ItemPath<McpServerFormInput, Row>,
        anchor: ItemPath<McpServerFormInput, Row>,
        cx: &mut Context<Self>,
    ) -> MoveRowHandler
    where
        Row: FormSchema,
    {
        let dialog = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let collection = collection.clone();
            let row = row.clone();
            let anchor = anchor.clone();
            let _ = dialog.update(cx, |dialog, cx| {
                match dialog
                    .draft
                    .move_row_before(collection, &row, &anchor, window, cx)
                {
                    Ok(false) => {
                        event!(Level::DEBUG, "ignored stale MCP row move callback");
                    }
                    Ok(true) => {
                        dialog.revalidate_form(ValidationTrigger::Change, window, cx);
                        cx.notify();
                    }
                    Err(error) => {
                        event!(Level::ERROR, error = ?error, "move MCP form row failed");
                    }
                }
            });
        })
    }

    fn set_oauth_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.set_oauth_enabled(enabled, window, cx);
        self.revalidate_form(ValidationTrigger::Change, window, cx);
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
        let credential_keys = std::mem::take(&mut self.draft_oauth_credential_keys)
            .into_iter()
            .collect::<Vec<_>>();
        if credential_keys.is_empty() {
            return;
        }
        let credential_count = credential_keys.len();
        state::mcp::oauth::schedule_credential_cleanup(
            credential_keys,
            move |result, _cx| {
                if result.failure_count == 0 {
                    return;
                }
                event!(
                    Level::ERROR,
                    credential_count,
                    failure_count = result.failure_count,
                    "MCP draft OAuth credential cleanup failed"
                );
            },
            cx,
        );
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
        let enabled = McpServerFormInput::OAUTH_ENABLED.get(&self.draft.form, cx);
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
                    .bg(cx.theme().tokens.muted.background.opacity(0.25))
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
        if !McpServerFormInput::OAUTH_ENABLED.get(&self.draft.form, cx) {
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

    fn revalidate_form(
        &self,
        trigger: ValidationTrigger,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let original_server_id = self.mode.original_server_id().map(ToOwned::to_owned);
        let existing_server_ids = state::config::read(cx, |config| {
            config.mcp_servers.keys().cloned().collect::<Vec<_>>()
        });
        self.draft.form.update(cx, |form, cx| {
            form.replace_validator(
                GardeValidator::<
                    McpServerFormInput,
                    crate::features::settings::form_validation::JacoGardeMessageProvider,
                >::new(mcp_validation_context(
                    original_server_id,
                    existing_server_ids,
                )),
                cx,
            );
            form.validate(trigger, cx);
        });
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
            let value = McpServerFormInput::ROOT.get(&self.draft.form, cx);
            (
                value.transport,
                self.components.server_id.clone(),
                true,
                self.components.command.clone(),
                value.transport == McpTransportKind::Stdio,
                self.components.cwd.clone(),
                false,
                self.components.url.clone(),
                value.transport == McpTransportKind::StreamableHttp,
                self.components.bearer_token_env_var.clone(),
                false,
                validation_messages(
                    McpServerFormInput::SERVER_ID.errors(&self.draft.form, cx),
                    cx,
                ),
                validation_messages(McpServerFormInput::COMMAND.errors(&self.draft.form, cx), cx),
                validation_messages(McpServerFormInput::CWD.errors(&self.draft.form, cx), cx),
                validation_messages(McpServerFormInput::URL.errors(&self.draft.form, cx), cx),
                validation_messages(
                    McpServerFormInput::BEARER_TOKEN_ENV_VAR.errors(&self.draft.form, cx),
                    cx,
                ),
            )
        };
        let scroll_handle = self.content_scroll_handle.clone();

        div()
            .on_action(cx.listener(Self::on_add_mcp_row))
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
                                        let items = live_mcp_items(
                                            McpServerFormInput::ARGS.items(&self.draft.form, cx),
                                            "args",
                                        );
                                        items
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(index, item)| {
                                                let key = item.key();
                                                let moves = Self::row_move_handlers(
                                                    McpServerFormInput::ROOT
                                                        .then(McpServerFormInput::ARGS),
                                                    &items,
                                                    index,
                                                    cx,
                                                );
                                                let remove = Self::remove_row_handler(
                                                    item.clone(),
                                                    McpServerFormDraft::remove_arg_row,
                                                    cx,
                                                );
                                                let Some(row) = self.components.args.get(&key)
                                                else {
                                                    event!(Level::ERROR, row = ?key, "MCP arg row control is missing");
                                                    return None;
                                                };
                                                let errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpArgRowInput::VALUE),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                Some((
                                                    key,
                                                    row.input.clone(),
                                                    errors,
                                                    moves,
                                                    remove,
                                                ))
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
                                        let items = live_mcp_items(
                                            McpServerFormInput::ENV.items(&self.draft.form, cx),
                                            "env",
                                        );
                                        items
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(index, item)| {
                                                let key = item.key();
                                                let moves = Self::row_move_handlers(
                                                    McpServerFormInput::ROOT
                                                        .then(McpServerFormInput::ENV),
                                                    &items,
                                                    index,
                                                    cx,
                                                );
                                                let remove = Self::remove_row_handler(
                                                    item.clone(),
                                                    McpServerFormDraft::remove_env_row,
                                                    cx,
                                                );
                                                let Some(row) = self.components.env.get(&key)
                                                else {
                                                    event!(Level::ERROR, row = ?key, "MCP env row controls are missing");
                                                    return None;
                                                };
                                                let key_errors = dynamic_validation_messages(
                                                    row.item.clone().then(McpEnvRowInput::KEY),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                let value_errors = dynamic_validation_messages(
                                                    row.item.clone().then(McpEnvRowInput::VALUE),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                Some((
                                                    key,
                                                    row.key.clone(),
                                                    key_errors,
                                                    row.value.clone(),
                                                    value_errors,
                                                    moves,
                                                    remove,
                                                ))
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
                                        let items = live_mcp_items(
                                            McpServerFormInput::ENV_VARS
                                                .items(&self.draft.form, cx),
                                            "env_vars",
                                        );
                                        items
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(index, item)| {
                                                let key = item.key();
                                                let moves = Self::row_move_handlers(
                                                    McpServerFormInput::ROOT
                                                        .then(McpServerFormInput::ENV_VARS),
                                                    &items,
                                                    index,
                                                    cx,
                                                );
                                                let remove = Self::remove_row_handler(
                                                    item.clone(),
                                                    McpServerFormDraft::remove_env_var_row,
                                                    cx,
                                                );
                                                let Some(row) = self.components.env_vars.get(&key)
                                                else {
                                                    event!(Level::ERROR, row = ?key, "MCP env-var row control is missing");
                                                    return None;
                                                };
                                                let errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpEnvVarRowInput::VALUE),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                Some((
                                                    key,
                                                    row.input.clone(),
                                                    errors,
                                                    moves,
                                                    remove,
                                                ))
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
                                Input::new(&url_input)
                                    .w_full()
                                    .content_type(InputContentType::Url)
                                    .into_any_element(),
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
                                        let items = live_mcp_items(
                                            McpServerFormInput::HEADERS
                                                .items(&self.draft.form, cx),
                                            "headers",
                                        );
                                        items
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(index, item)| {
                                                let key = item.key();
                                                let moves = Self::row_move_handlers(
                                                    McpServerFormInput::ROOT
                                                        .then(McpServerFormInput::HEADERS),
                                                    &items,
                                                    index,
                                                    cx,
                                                );
                                                let remove = Self::remove_row_handler(
                                                    item.clone(),
                                                    McpServerFormDraft::remove_header_row,
                                                    cx,
                                                );
                                                let Some(row) = self.components.headers.get(&key)
                                                else {
                                                    event!(Level::ERROR, row = ?key, "MCP header row controls are missing");
                                                    return None;
                                                };
                                                let name_errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpHeaderRowInput::NAME),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                let value_errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpHeaderRowInput::VALUE),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                Some((
                                                    key,
                                                    row.key.clone(),
                                                    name_errors,
                                                    row.value.clone(),
                                                    value_errors,
                                                    moves,
                                                    remove,
                                                ))
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
                                        let items = live_mcp_items(
                                            McpServerFormInput::ENV_HEADERS
                                                .items(&self.draft.form, cx),
                                            "env_headers",
                                        );
                                        items
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(index, item)| {
                                                let key = item.key();
                                                let moves = Self::row_move_handlers(
                                                    McpServerFormInput::ROOT
                                                        .then(McpServerFormInput::ENV_HEADERS),
                                                    &items,
                                                    index,
                                                    cx,
                                                );
                                                let remove = Self::remove_row_handler(
                                                    item.clone(),
                                                    McpServerFormDraft::remove_env_header_row,
                                                    cx,
                                                );
                                                let Some(row) =
                                                    self.components.env_headers.get(&key)
                                                else {
                                                    event!(Level::ERROR, row = ?key, "MCP env-header row controls are missing");
                                                    return None;
                                                };
                                                let name_errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpEnvHeaderRowInput::NAME),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                let env_var_errors = dynamic_validation_messages(
                                                    row.item
                                                        .clone()
                                                        .then(McpEnvHeaderRowInput::ENV_VAR),
                                                    &self.draft.form,
                                                    cx,
                                                )?;
                                                Some((
                                                    key,
                                                    row.key.clone(),
                                                    name_errors,
                                                    row.value.clone(),
                                                    env_var_errors,
                                                    moves,
                                                    remove,
                                                ))
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

async fn delete_oauth_credentials_for_sign_out(
    request: McpOAuthSignOutRequest,
    cx: &mut gpui::AsyncWindowContext,
) -> Result<McpOAuthSignOutRequest, String> {
    state::mcp::oauth::delete_credentials(&request.credential_key, cx).await?;
    Ok(request)
}

fn oauth_credential_keys_to_delete(
    original_server_id: Option<&str>,
    original_config: Option<&McpServerTomlConfig>,
    saved_server_id: &str,
    saved_server: &McpServerTomlConfig,
    draft_credential_keys: &BTreeSet<state::mcp::oauth::CredentialsKey>,
    promoted_draft_key: Option<state::mcp::oauth::CredentialsKey>,
) -> Vec<state::mcp::oauth::CredentialsKey> {
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
) -> Option<state::mcp::oauth::CredentialsKey> {
    state::mcp::oauth::credentials_key_for_server(server_id, server)
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
            let credential_keys_to_delete = server_before_delete
                .as_ref()
                .and_then(|server| oauth_credential_key_for_server(&server_id, server))
                .into_iter()
                .collect::<Vec<_>>();
            let server_id = server_id.clone();
            let deleted_title = deleted_title.clone();
            let delete_failed_title = delete_failed_title.clone();
            let config_result = state::config::delete_mcp_server(cx, &server_id);
            let Err(error) = config_result else {
                disconnect_server(server_id, window, cx);
                let window_handle = window.window_handle();
                state::mcp::oauth::schedule_credential_cleanup(
                    credential_keys_to_delete,
                    move |result, cx| {
                        let (message, notification_type) = if result.failure_count == 0 {
                            (None, NotificationType::Success)
                        } else {
                            (result.first_error, NotificationType::Warning)
                        };
                        let mut notification = Notification::new()
                            .title(deleted_title.clone())
                            .with_type(notification_type);
                        if let Some(message) = message {
                            notification = notification.message(message);
                        }
                        if let Err(err) = window_handle.update(cx, |_, window, cx| {
                            window.push_notification(notification, cx);
                        }) {
                            event!(Level::ERROR, error = ?err, "finish mcp server delete failed");
                        }
                    },
                    cx,
                );
                return;
            };
            push_settings_error(window, cx, delete_failed_title.clone(), error);
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
    draft_oauth_credential_key: Option<&state::mcp::oauth::CredentialsKey>,
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
    draft_oauth_credential_key: Option<&state::mcp::oauth::CredentialsKey>,
    server_id: &str,
    saved_server: &McpServerTomlConfig,
) -> Option<state::mcp::oauth::CredentialsKey> {
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

fn validation_messages(issues: Vec<gpui_form::ValidationIssue>, cx: &App) -> Vec<SharedString> {
    issues
        .into_iter()
        .map(|issue| {
            crate::features::settings::form_validation::validation_message(issue.message(), cx)
        })
        .collect()
}

fn live_mcp_items<Row: FormSchema>(
    items: Vec<ItemPath<McpServerFormInput, Row>>,
    _list: &'static str,
) -> Vec<ItemPath<McpServerFormInput, Row>> {
    items
}

fn dynamic_validation_messages<T: Clone + PartialEq + 'static>(
    path: DynamicPath<McpServerFormInput, T>,
    form: &Entity<gpui_form::Form<McpServerFormInput>>,
    cx: &App,
) -> Option<Vec<SharedString>> {
    match path.try_errors(form, cx) {
        Ok(issues) => Some(validation_messages(issues, cx)),
        Err(ResolveError::Retired { .. } | ResolveError::MissingItem { .. }) => {
            event!(
                Level::DEBUG,
                "ignored retired MCP validation path during row projection"
            );
            None
        }
        Err(error) => {
            event!(Level::ERROR, error = ?error, "resolve MCP validation path failed");
            None
        }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::{
        features::settings::mcp::form_state::{McpArgRowInput, McpServerFormInput},
        foundation, state,
        state::config::{McpOAuthTomlConfig, McpServerTomlConfig},
    };
    use gpui::{AppContext as _, Entity, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::{InputEvent, InputState};
    use jaco_agent::McpOAuthStatusSnapshot;
    use tempfile::{TempDir, tempdir};

    use super::{
        CredentialCleanupOutcome, McpServerEditDialogState, McpServerEditMode, McpTransportKind,
        can_authorize_oauth_values, credential_cleanup_outcome, credential_cleanup_plan,
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
    fn credential_cleanup_is_planned_only_after_config_commit() {
        assert_eq!(
            credential_cleanup_plan(&Err("config conflict"), vec!["old", "draft"]),
            None
        );
        assert_eq!(
            credential_cleanup_plan(&Ok::<(), &str>(()), vec!["old", "draft"]),
            Some(vec!["old", "draft"])
        );
        assert_eq!(
            credential_cleanup_plan(&Ok::<(), &str>(()), Vec::<&str>::new()),
            None
        );
    }

    #[test]
    fn credential_cleanup_failure_is_warning_only() {
        assert_eq!(
            credential_cleanup_outcome(0),
            CredentialCleanupOutcome::Complete
        );
        assert_eq!(
            credential_cleanup_outcome(1),
            CredentialCleanupOutcome::WarnOnly
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
        let _dir = init_dialog_test(cx);
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
            assert!(!form.read(cx).is_saving(cx));
        });
    }

    #[gpui::test]
    fn save_validation_errors_are_applied_to_form_fields(cx: &mut TestAppContext) {
        let _dir = init_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx.update(|window, cx| {
            cx.new(|cx| McpServerEditDialogState::new(McpServerEditMode::Create, None, window, cx))
        });
        let arg_input = cx.update(|_, cx| {
            let (draft_form, arg_input) = {
                let dialog = form.read(cx);
                let draft_form = dialog.draft.form.clone();
                let row_key = McpServerFormInput::ARGS.items(&draft_form, cx)[0].key();
                let input = dialog
                    .components
                    .args
                    .get(&row_key)
                    .expect("MCP arg row control exists")
                    .input
                    .clone();
                (draft_form, input)
            };
            let _ = draft_form;
            arg_input
        });
        set_input_value(arg_input, "   ", &mut cx);
        cx.run_until_parked();

        cx.update(|window, cx| {
            assert!(!form.update(cx, |form, cx| form.save(window, cx)));

            let draft_form = form.read(cx).draft.form.clone();
            let arg = McpServerFormInput::ARGS.items(&draft_form, cx).remove(0);
            assert_eq!(
                McpServerFormInput::SERVER_ID
                    .errors(&draft_form, cx)
                    .first()
                    .map(|error| error.code()),
                Some("required")
            );
            assert_eq!(
                McpServerFormInput::COMMAND
                    .errors(&draft_form, cx)
                    .first()
                    .map(|error| error.code()),
                Some("garde")
            );

            assert_eq!(
                arg.then(McpArgRowInput::VALUE)
                    .try_errors(&draft_form, cx)
                    .unwrap()
                    .first()
                    .map(|error| error.code()),
                Some("garde")
            );
            assert!(
                McpServerFormInput::COMMAND
                    .errors(&draft_form, cx)
                    .iter()
                    .any(|error| error.code() == "garde")
            );
        });
    }

    #[gpui::test]
    fn same_parent_reorder_reuses_mcp_row_control_owner(cx: &mut TestAppContext) {
        let _dir = init_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let dialog = cx.update(|window, cx| {
            cx.new(|cx| {
                McpServerEditDialogState::new(
                    McpServerEditMode::Create,
                    Some(McpServerTomlConfig {
                        transport: McpTransportKind::Stdio,
                        command: Some("mcp".to_string()),
                        args: vec!["first".to_string()],
                        ..Default::default()
                    }),
                    window,
                    cx,
                )
            })
        });

        cx.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.draft.add_arg_row(window, cx).unwrap();
            });
        });
        cx.run_until_parked();

        let (first, second, control_ids) = cx.update(|_, cx| {
            let dialog = dialog.read(cx);
            let items = McpServerFormInput::ARGS.items(&dialog.draft.form, cx);
            let control_ids = items
                .iter()
                .map(|item| {
                    let key = item.key();
                    (key.clone(), dialog.components.args[&key].input.entity_id())
                })
                .collect::<std::collections::HashMap<_, _>>();
            (items[0].clone(), items[1].clone(), control_ids)
        });

        cx.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                assert!(
                    dialog
                        .draft
                        .move_row_before(
                            McpServerFormInput::ROOT.then(McpServerFormInput::ARGS),
                            &second,
                            &first,
                            window,
                            cx,
                        )
                        .unwrap()
                );
            });
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            let dialog = dialog.read(cx);
            let items = McpServerFormInput::ARGS.items(&dialog.draft.form, cx);
            assert_eq!(items[0].key(), second.key());
            assert_eq!(items[1].key(), first.key());
            for item in items {
                let key = item.key();
                assert_eq!(
                    dialog.components.args[&key].input.entity_id(),
                    control_ids[&key]
                );
            }
        });
    }

    #[gpui::test]
    fn retired_remove_callback_cannot_remove_reinserted_row(cx: &mut TestAppContext) {
        let _dir = init_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let dialog = cx.update(|window, cx| {
            cx.new(|cx| McpServerEditDialogState::new(McpServerEditMode::Create, None, window, cx))
        });

        let (remove, old_key) = cx.update(|_, cx| {
            dialog.update(cx, |dialog, cx| {
                let row = McpServerFormInput::ARGS
                    .items(&dialog.draft.form, cx)
                    .remove(0);
                let key = row.key();
                (
                    McpServerEditDialogState::remove_row_handler(
                        row,
                        super::super::form_state::McpServerFormDraft::remove_arg_row,
                        cx,
                    ),
                    key,
                )
            })
        });

        cx.update(|window, cx| remove(window, cx));
        cx.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.draft.add_arg_row(window, cx).unwrap();
            });
        });
        cx.run_until_parked();
        let new_key = cx.update(|_, cx| {
            let form = dialog.read(cx).draft.form.clone();
            McpServerFormInput::ARGS.items(&form, cx).remove(0).key()
        });
        assert_ne!(old_key, new_key);

        cx.update(|window, cx| remove(window, cx));
        cx.update(|_, cx| {
            let form = dialog.read(cx).draft.form.clone();
            assert_eq!(McpServerFormInput::ROOT.get(&form, cx).args.len(), 1);
            assert_eq!(McpServerFormInput::ARGS.items(&form, cx)[0].key(), new_key);
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

    fn init_dialog_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().expect("create test config dir");
        let config_path = dir.path().join("config.toml");
        cx.update(|cx| {
            gpui_component::init(cx);
            foundation::init_i18n(cx);
            let config = state::JacoConfig::load_from_path_for_test(&config_path)
                .expect("create test config");
            state::config::install_for_test(cx, config_path.clone(), config)
                .expect("install config store");
            state::mcp::init(cx).expect("init MCP runtime");
        });
        dir
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let _ = window;
                cx.new(|_| TestView)
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
