use std::collections::{BTreeMap, BTreeSet};

use crate::{
    database,
    foundation::{I18n, assets::IconName},
};
use ai_chat_agent::{ProviderModelFetchError, ProviderModelFetchRequest, fetch_provider_models};
use ai_chat_core::{
    ProviderId, ProviderSecretRefs, ProviderSettingValue, ProviderSettingsPayload, new_id,
};
use ai_chat_db::{
    FreshRepository, NewProvider, ProviderModelRecord, ProviderRecord, UpdateProvider,
};
use fluent_bundle::FluentArgs;
use gpui::{StatefulInteractiveElement as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::{List, ListEvent, ListState},
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    switch::Switch,
    tag::Tag,
    v_flex,
};
use tracing::{Level, event};

mod capabilities;
mod catalog;
mod components;
mod draft;
mod list_delegates;
mod model_fetch;
mod secret_store;

use self::{
    catalog::{ProviderFieldKind, ProviderKindKey, ProviderSpec, builtin_provider_specs},
    draft::{
        AsyncActionState, ManualModelEditor, ProviderDraft, ProviderDraftSnapshot,
        ProviderDraftValue, ProviderModelDraft, ProviderSecretInput, ProviderSelection,
        ProviderValidationState,
    },
    list_delegates::{
        ProviderListDelegate, ProviderModelListDelegate, model_list_rows, provider_list_rows,
    },
    model_fetch::{ModelFetchSupport, fetch_support},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderListItem {
    spec: ProviderSpec,
    provider: Option<ProviderRecord>,
}

pub(super) struct ProviderSettingsPage {
    provider_list: Entity<ListState<ProviderListDelegate>>,
    model_list: Entity<ListState<ProviderModelListDelegate>>,
    detail_scroll_handle: ScrollHandle,
    selected: ProviderSelection,
    providers: Vec<ProviderListItem>,
    models: Vec<ProviderModelDraft>,
    draft: ProviderDraft,
    saved_snapshot: Option<ProviderDraftSnapshot>,
    text_inputs: BTreeMap<String, Entity<InputState>>,
    secret_inputs: BTreeMap<String, Entity<ProviderSecretInput>>,
    validation: ProviderValidationState,
    save_state: AsyncActionState,
    fetch_state: AsyncActionState,
    #[allow(dead_code)]
    manual_model_editor: Option<Entity<ManualModelEditor>>,
    _list_subscriptions: Vec<Subscription>,
    _field_subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
    _save_task: Option<Task<()>>,
    _fetch_task: Option<Task<()>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProviderSecretValidationState {
    has_saved_secret: bool,
    has_pending_value: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderFetchPrecondition {
    Ready,
    SaveProviderFirst,
    SaveChangesFirst,
    ManualModelsRequired,
}

#[derive(Debug, Clone)]
struct ProviderSaveRequest {
    provider_id: Option<ProviderId>,
    new_provider_id: Option<ProviderId>,
    kind: String,
    display_name: String,
    enabled: bool,
    settings: ProviderSettingsPayload,
    secret_refs: ProviderSecretRefs,
    writes: Vec<secret_store::ProviderSecretWrite>,
}

impl ProviderSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let providers = Self::load_provider_list(cx).unwrap_or_else(|err| {
            event!(Level::ERROR, error = ?err, "load provider settings failed");
            Vec::new()
        });
        let selected = providers
            .first()
            .map(|item| ProviderSelection::Builtin {
                kind: item.spec.kind.clone(),
                provider_id: item.provider.as_ref().map(|provider| provider.id.clone()),
            })
            .unwrap_or(ProviderSelection::NewCustom);
        let draft = Self::draft_for_selection(&selected, &providers);
        let models = Self::load_models_for_draft(&draft, cx).unwrap_or_default();
        let provider_rows = provider_list_rows(&providers, cx.global::<I18n>());
        let provider_selected_index =
            ProviderListDelegate::selected_index_for(&provider_rows, &draft.kind);
        let provider_empty_label = cx.global::<I18n>().t("provider-empty-selection");
        let provider_list = cx.new(|cx| {
            let mut state = ListState::new(
                ProviderListDelegate::new(provider_rows.clone(), provider_empty_label.clone()),
                window,
                cx,
            );
            state.set_selected_index(provider_selected_index, window, cx);
            state.searchable(true)
        });
        let model_rows = model_list_rows(&models);
        let model_empty_label = cx.global::<I18n>().t("provider-empty-models");
        let model_list = cx.new(|cx| {
            ListState::new(
                ProviderModelListDelegate::new(model_rows.clone(), model_empty_label.clone()),
                window,
                cx,
            )
            .searchable(true)
            .selectable(false)
        });
        let provider_list_subscription =
            cx.subscribe_in(&provider_list, window, Self::on_provider_list_event);
        let model_list_subscription =
            cx.subscribe_in(&model_list, window, Self::on_model_list_event);
        let mut this = Self {
            provider_list,
            model_list,
            detail_scroll_handle: ScrollHandle::default(),
            selected,
            providers,
            models,
            draft,
            saved_snapshot: None,
            text_inputs: BTreeMap::new(),
            secret_inputs: BTreeMap::new(),
            validation: ProviderValidationState::Idle,
            save_state: AsyncActionState::Idle,
            fetch_state: AsyncActionState::Idle,
            manual_model_editor: None,
            _list_subscriptions: vec![provider_list_subscription, model_list_subscription],
            _field_subscriptions: Vec::new(),
            _load_task: None,
            _save_task: None,
            _fetch_task: None,
        };
        this.rebuild_inputs(window, cx);
        this.saved_snapshot = Some(this.current_snapshot(cx));
        this
    }

    fn load_provider_list(cx: &App) -> ai_chat_db::Result<Vec<ProviderListItem>> {
        let records = database::repository(cx).list_providers()?;
        Ok(builtin_provider_specs()
            .into_iter()
            .map(|spec| {
                let provider = records
                    .iter()
                    .find(|provider| provider.kind == spec.kind.as_str())
                    .cloned();
                ProviderListItem { spec, provider }
            })
            .collect())
    }

    fn draft_for_selection(
        selection: &ProviderSelection,
        providers: &[ProviderListItem],
    ) -> ProviderDraft {
        match selection {
            ProviderSelection::Builtin { kind, provider_id } => {
                let item = providers
                    .iter()
                    .find(|item| &item.spec.kind == kind)
                    .expect("builtin provider selection must have catalog item");
                if let Some(provider) = item.provider.as_ref() {
                    draft_from_record(provider)
                } else {
                    draft_from_spec(&item.spec, provider_id.clone())
                }
            }
            ProviderSelection::Custom { provider_id } => providers
                .iter()
                .filter_map(|item| item.provider.as_ref())
                .find(|provider| &provider.id == provider_id)
                .map(draft_from_record)
                .unwrap_or_else(|| {
                    draft_from_spec(&builtin_provider_specs().last().unwrap().clone(), None)
                }),
            ProviderSelection::NewCustom => {
                draft_from_spec(&builtin_provider_specs().last().unwrap().clone(), None)
            }
        }
    }

    fn load_models_for_draft(
        draft: &ProviderDraft,
        cx: &App,
    ) -> ai_chat_db::Result<Vec<ProviderModelDraft>> {
        let Some(provider_id) = draft.provider_id.as_ref() else {
            return Ok(Vec::new());
        };
        database::repository(cx)
            .list_provider_models(provider_id)?
            .into_iter()
            .map(|model| Ok(model.into()))
            .collect()
    }

    fn rebuild_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.text_inputs.clear();
        self.secret_inputs.clear();
        self._field_subscriptions.clear();
        let Some(spec) = self.selected_spec() else {
            return;
        };
        let fields = spec.fields.clone();
        for field in fields {
            match field.kind {
                ProviderFieldKind::Secret => {
                    let saved_ref_id = self
                        .draft
                        .existing_secret_refs
                        .refs
                        .iter()
                        .find(|secret| secret.key == field.key)
                        .map(|secret| secret.ref_id.clone());
                    let input = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(cx.global::<I18n>().t(field.placeholder_key))
                            .masked(true)
                    });
                    self._field_subscriptions.push(cx.subscribe_in(
                        &input,
                        window,
                        Self::on_form_input,
                    ));
                    let key = field.key.to_string();
                    let secret_input = input.clone();
                    let secret = cx.new(|cx| {
                        ProviderSecretInput::new(
                            key.clone(),
                            saved_ref_id,
                            secret_input,
                            window,
                            cx,
                        )
                    });
                    self.secret_inputs.insert(field.key.to_string(), secret);
                }
                ProviderFieldKind::Text | ProviderFieldKind::Url | ProviderFieldKind::Select => {
                    let value = self.draft.field_string(field.key);
                    let input = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(cx.global::<I18n>().t(field.placeholder_key))
                            .default_value(value)
                    });
                    self._field_subscriptions.push(cx.subscribe_in(
                        &input,
                        window,
                        Self::on_form_input,
                    ));
                    self.text_inputs.insert(field.key.to_string(), input);
                }
            }
        }
    }

    fn on_form_input(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.validation = ProviderValidationState::Idle;
            cx.notify();
        }
    }

    fn selected_spec(&self) -> Option<&ProviderSpec> {
        let kind = &self.draft.kind;
        self.providers
            .iter()
            .find(|item| &item.spec.kind == kind)
            .map(|item| &item.spec)
    }

    fn select_provider_from_list(
        &mut self,
        kind: ProviderKindKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_provider_inner(kind, window, cx);
        self.sync_model_list(window, cx);
        cx.notify();
    }

    fn on_provider_list_event(
        &mut self,
        _: &Entity<ListState<ProviderListDelegate>>,
        event: &ListEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ix = match event {
            ListEvent::Select(ix) | ListEvent::Confirm(ix) => *ix,
            ListEvent::Cancel => return,
        };
        let Some(kind) = self.provider_list.read(cx).delegate().kind_for_index(ix) else {
            return;
        };
        if self.draft.kind == kind {
            return;
        }
        self.select_provider_from_list(kind, window, cx);
    }

    fn on_model_list_event(
        &mut self,
        _: &Entity<ListState<ProviderModelListDelegate>>,
        event: &ListEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ListEvent::Confirm(ix) = event else {
            return;
        };
        let Some(row) = self
            .model_list
            .read(cx)
            .delegate()
            .row_for_index(*ix)
            .cloned()
        else {
            return;
        };
        self.toggle_model(row.model_id, !row.enabled, window, cx);
    }

    fn select_provider_inner(
        &mut self,
        kind: ProviderKindKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let provider_id = self
            .providers
            .iter()
            .find(|item| item.spec.kind == kind)
            .and_then(|item| item.provider.as_ref().map(|provider| provider.id.clone()));
        self.selected = ProviderSelection::Builtin { kind, provider_id };
        self.draft = Self::draft_for_selection(&self.selected, &self.providers);
        self.models = Self::load_models_for_draft(&self.draft, cx).unwrap_or_default();
        self.validation = ProviderValidationState::Idle;
        self.rebuild_inputs(window, cx);
        self.saved_snapshot = Some(self.current_snapshot(cx));
    }

    fn sync_list_delegates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_provider_list(window, cx);
        self.sync_model_list(window, cx);
    }

    fn sync_provider_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = provider_list_rows(&self.providers, cx.global::<I18n>());
        self.provider_list.update(cx, |list, cx| {
            list.delegate_mut().set_rows(rows);
            let selected_index = list.delegate().selected_index_for_kind(&self.draft.kind);
            list.set_selected_index(selected_index, window, cx);
        });
    }

    fn sync_model_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = model_list_rows(&self.models);
        self.model_list.update(cx, |list, cx| {
            list.delegate_mut().set_rows(rows);
            list.set_selected_index(None, window, cx);
        });
    }

    fn sync_inputs_to_draft(&mut self, cx: &mut Context<Self>) {
        for (key, input) in &self.text_inputs {
            let value = input.read(cx).value().to_string();
            self.draft
                .fields
                .insert(key.clone(), ProviderDraftValue::String(value));
        }
    }

    fn validate_current_draft(&mut self, cx: &mut Context<Self>) -> bool {
        self.sync_inputs_to_draft(cx);
        let Some(spec) = self.selected_spec().cloned() else {
            self.validation = ProviderValidationState::Invalid(
                cx.global::<I18n>()
                    .t("provider-validation-not-registered")
                    .into(),
            );
            return false;
        };
        let secrets = self.secret_validation_states(cx);
        match validate_provider_draft(&self.draft, &spec, &secrets, cx.global::<I18n>()) {
            Ok(()) => {
                self.validation = ProviderValidationState::Valid;
                true
            }
            Err(message) => {
                self.validation = ProviderValidationState::Invalid(message);
                false
            }
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_state == AsyncActionState::Running {
            return;
        }
        if !self.validate_current_draft(cx) {
            if let ProviderValidationState::Invalid(message) = &self.validation {
                window.push_notification(
                    Notification::new()
                        .title(
                            cx.global::<I18n>()
                                .t("provider-notification-validation-failed"),
                        )
                        .message(message.clone())
                        .with_type(NotificationType::Error),
                    cx,
                );
            }
            cx.notify();
            return;
        }
        self.save_state = AsyncActionState::Running;
        self.sync_inputs_to_draft(cx);
        let repository = database::repository(cx);
        let writes = self.secret_writes(cx);
        let provider_id = self.draft.provider_id.clone();
        let new_provider_id = provider_id.is_none().then(new_id);
        let secret_ref_owner = provider_id
            .as_deref()
            .or(new_provider_id.as_deref())
            .expect("new provider id is preallocated before saving secrets");
        let secret_refs = self.secret_refs_for_provider(secret_ref_owner, &writes);
        let save = ProviderSaveRequest {
            provider_id,
            new_provider_id,
            kind: self.draft.kind.as_str().to_string(),
            display_name: self.draft.display_name.clone(),
            enabled: self.draft.enabled,
            settings: self.draft.settings_payload(),
            secret_refs,
            writes,
        };
        let page = cx.entity().downgrade();
        self._save_task = Some(window.spawn(cx, async move |cx| {
            let result = save_provider(repository, save, cx).await;
            if let Err(err) = page.update_in(cx, |page, window, cx| {
                page.finish_save(result, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish provider save failed");
            }
        }));
        cx.notify();
    }

    fn finish_save(
        &mut self,
        result: Result<ProviderRecord, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._save_task = None;
        self.save_state = AsyncActionState::Idle;
        match result {
            Ok(provider) => {
                self.draft = draft_from_record(&provider);
                self.providers = Self::load_provider_list(cx).unwrap_or_default();
                self.models = Self::load_models_for_draft(&self.draft, cx).unwrap_or_default();
                self.rebuild_inputs(window, cx);
                self.saved_snapshot = Some(self.current_snapshot(cx));
                self.validation = ProviderValidationState::Valid;
                self.sync_list_delegates(window, cx);
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-saved"))
                        .message(provider.display_name)
                        .with_type(NotificationType::Success),
                    cx,
                );
            }
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("notify-save-settings-failed"))
                        .message(err)
                        .with_type(NotificationType::Error),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn current_snapshot(&self, cx: &App) -> ProviderDraftSnapshot {
        let mut snapshot = ProviderDraftSnapshot::from_draft(&self.draft);
        for (key, input) in &self.text_inputs {
            snapshot.fields.insert(
                key.clone(),
                ProviderDraftValue::String(input.read(cx).value().to_string()),
            );
        }
        snapshot.dirty_secret_keys = self
            .secret_inputs
            .iter()
            .filter_map(|(key, secret)| {
                let secret = secret.read(cx);
                let value = secret.input.read(cx).value();
                (secret.dirty || !value.is_empty()).then(|| key.clone())
            })
            .collect::<BTreeSet<_>>();
        snapshot
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.current_snapshot(cx)
            .is_dirty_against(self.saved_snapshot.as_ref())
    }

    fn secret_validation_states(
        &self,
        cx: &App,
    ) -> BTreeMap<String, ProviderSecretValidationState> {
        self.secret_inputs
            .iter()
            .map(|(key, secret)| {
                let secret = secret.read(cx);
                (
                    key.clone(),
                    ProviderSecretValidationState {
                        has_saved_secret: secret.has_saved_secret,
                        has_pending_value: !secret.input.read(cx).value().is_empty(),
                    },
                )
            })
            .collect()
    }

    fn secret_writes(&self, cx: &App) -> Vec<secret_store::ProviderSecretWrite> {
        self.secret_inputs
            .iter()
            .filter_map(|(key, secret)| {
                let secret = secret.read(cx);
                let value = secret.input.read(cx).value().to_string();
                (!value.is_empty()).then(|| secret_store::ProviderSecretWrite {
                    key: key.clone(),
                    value,
                })
            })
            .collect()
    }

    fn secret_refs_for_provider(
        &self,
        provider_id: &str,
        writes: &[secret_store::ProviderSecretWrite],
    ) -> ProviderSecretRefs {
        let mut refs = secret_store::ProviderSecretStore::refs_for(provider_id, &writes);
        for saved in &self.draft.existing_secret_refs.refs {
            if !refs.refs.iter().any(|secret| secret.key == saved.key) {
                refs.refs.push(saved.clone());
            }
        }
        refs
    }

    fn toggle_model(
        &mut self,
        model_id: String,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(provider_id) = self.draft.provider_id.clone() else {
            return;
        };
        match database::repository(cx).set_provider_model_enabled(&provider_id, &model_id, enabled)
        {
            Ok(_) => {
                self.models = Self::load_models_for_draft(&self.draft, cx).unwrap_or_default();
                self.sync_model_list(window, cx);
                cx.notify();
            }
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(
                            cx.global::<I18n>()
                                .t("provider-notification-update-model-failed"),
                        )
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
            }
        }
    }

    fn fetch_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_state == AsyncActionState::Running {
            return;
        }
        let Some(spec) = self.selected_spec() else {
            return;
        };
        let support = fetch_support(spec.model_listing);
        let precondition = provider_fetch_precondition(
            self.draft.provider_id.as_ref(),
            self.is_dirty(cx),
            support,
        );
        match precondition {
            ProviderFetchPrecondition::Ready => {}
            ProviderFetchPrecondition::SaveProviderFirst => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-fetch-failed"))
                        .message(cx.global::<I18n>().t("provider-fetch-save-first"))
                        .with_type(NotificationType::Warning),
                    cx,
                );
                return;
            }
            ProviderFetchPrecondition::SaveChangesFirst => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-fetch-failed"))
                        .message(cx.global::<I18n>().t("provider-fetch-save-changes-first"))
                        .with_type(NotificationType::Warning),
                    cx,
                );
                return;
            }
            ProviderFetchPrecondition::ManualModelsRequired => {
                window.push_notification(
                    Notification::new()
                        .title(
                            cx.global::<I18n>()
                                .t("provider-notification-fetch-unavailable-title"),
                        )
                        .message(cx.global::<I18n>().t("provider-fetch-manual-only"))
                        .with_type(NotificationType::Warning),
                    cx,
                );
                return;
            }
        }
        let Some(provider_id) = self.draft.provider_id.clone() else {
            return;
        };
        self.fetch_state = AsyncActionState::Running;
        let repository = database::repository(cx);
        let page = cx.entity().downgrade();
        self._fetch_task = Some(window.spawn(cx, async move |cx| {
            let result = fetch_and_store_models(repository, provider_id, cx).await;
            if let Err(err) = page.update_in(cx, |page, window, cx| {
                page.finish_fetch(result, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish provider model fetch failed");
            }
        }));
        cx.notify();
    }

    fn finish_fetch(
        &mut self,
        result: Result<usize, ProviderModelFetchError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._fetch_task = None;
        self.fetch_state = AsyncActionState::Idle;
        match result {
            Ok(count) => {
                self.models = Self::load_models_for_draft(&self.draft, cx).unwrap_or_default();
                self.sync_model_list(window, cx);
                let mut args = FluentArgs::new();
                args.set("count", count);
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-fetch-success"))
                        .message(
                            cx.global::<I18n>()
                                .t_with_args("provider-fetch-success-message", &args),
                        )
                        .with_type(NotificationType::Success),
                    cx,
                );
            }
            Err(ProviderModelFetchError::ManualModelsRequired { .. }) => {
                window.push_notification(
                    Notification::new()
                        .title(
                            cx.global::<I18n>()
                                .t("provider-notification-fetch-unavailable-title"),
                        )
                        .message(cx.global::<I18n>().t("provider-fetch-manual-only"))
                        .with_type(NotificationType::Warning),
                    cx,
                );
            }
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-fetch-failed"))
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn render_provider_list(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w(px(260.))
            .min_w(px(220.))
            .h_full()
            .min_h_0()
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .child(
                        List::new(&self.provider_list)
                            .search_placeholder(
                                cx.global::<I18n>().t("provider-search-placeholder"),
                            )
                            .w_full()
                            .h_full(),
                    ),
            )
            .into_any_element()
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(spec) = self.selected_spec() else {
            return v_flex()
                .child(cx.global::<I18n>().t("provider-empty-selection"))
                .into_any_element();
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .gap_4()
            .child(self.render_header(spec, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .relative()
                    .child(
                        div()
                            .id("provider-settings-detail-scroll")
                            .size_full()
                            .track_scroll(&self.detail_scroll_handle)
                            .overflow_y_scroll()
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .gap_4()
                                    .child(self.render_config(spec, cx))
                                    .child(self.render_models(cx)),
                            ),
                    )
                    .vertical_scrollbar(&self.detail_scroll_handle),
            )
            .into_any_element()
    }

    fn render_header(&self, spec: &ProviderSpec, cx: &mut Context<Self>) -> AnyElement {
        let dirty = self.is_dirty(cx);
        h_flex()
            .flex_none()
            .w_full()
            .items_start()
            .justify_between()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Label::new(spec.display_name).text_lg().font_medium())
                            .when(self.draft.provider_id.is_some() && !dirty, |this| {
                                this.child(
                                    Tag::success()
                                        .small()
                                        .child(cx.global::<I18n>().t("provider-status-saved")),
                                )
                            })
                            .when(dirty, |this| {
                                this.child(
                                    Tag::warning()
                                        .small()
                                        .child(cx.global::<I18n>().t("provider-status-unsaved")),
                                )
                            }),
                    )
                    .child(
                        Label::new(cx.global::<I18n>().t(spec.description_key))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                Switch::new("provider-settings-enabled")
                    .checked(self.draft.enabled)
                    .on_click(cx.listener(|page, checked, _, cx| {
                        page.draft.enabled = *checked;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_config(&self, spec: &ProviderSpec, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .flex_none()
            .w_full()
            .gap_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_4()
            .child(
                Label::new(cx.global::<I18n>().t("provider-section-configuration"))
                    .text_sm()
                    .font_medium(),
            )
            .children(spec.fields.iter().map(|field| {
                v_flex()
                    .gap_1()
                    .child(Label::new(cx.global::<I18n>().t(field.label_key)).text_sm())
                    .child(match field.kind {
                        ProviderFieldKind::Secret => self
                            .secret_inputs
                            .get(field.key)
                            .map(|secret| {
                                Input::new(&secret.read(cx).input)
                                    .w_full()
                                    .mask_toggle()
                                    .into_any_element()
                            })
                            .unwrap_or_else(|| div().into_any_element()),
                        _ => self
                            .text_inputs
                            .get(field.key)
                            .map(|input| Input::new(input).w_full().into_any_element())
                            .unwrap_or_else(|| div().into_any_element()),
                    })
                    .into_any_element()
            }))
            .child(self.render_validation_state(cx))
            .child(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("provider-settings-validate")
                            .icon(IconName::CircleCheck)
                            .label(cx.global::<I18n>().t("provider-action-validate"))
                            .small()
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.validate_current_draft(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("provider-settings-save")
                            .icon(IconName::Check)
                            .label(cx.global::<I18n>().t("provider-action-save"))
                            .small()
                            .primary()
                            .loading(self.save_state == AsyncActionState::Running)
                            .disabled(self.save_state == AsyncActionState::Running)
                            .on_click(cx.listener(|page, _, window, cx| page.save(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_validation_state(&self, cx: &mut Context<Self>) -> AnyElement {
        match &self.validation {
            ProviderValidationState::Idle => div().into_any_element(),
            ProviderValidationState::Valid => h_flex()
                .gap_2()
                .text_color(cx.theme().success)
                .child(Icon::new(IconName::CircleCheck))
                .child(Label::new(cx.global::<I18n>().t("provider-validation-valid")).text_sm())
                .into_any_element(),
            ProviderValidationState::Invalid(message) => h_flex()
                .gap_2()
                .text_color(cx.theme().danger)
                .child(Icon::new(IconName::CircleAlert))
                .child(Label::new(message.clone()).text_sm())
                .into_any_element(),
        }
    }

    fn render_models(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .flex_none()
            .w_full()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(cx.global::<I18n>().t("provider-section-models"))
                            .text_sm()
                            .font_medium(),
                    )
                    .child(
                        Button::new("provider-settings-fetch-models")
                            .icon(IconName::RefreshCcw)
                            .label(cx.global::<I18n>().t("provider-action-fetch-models"))
                            .small()
                            .loading(self.fetch_state == AsyncActionState::Running)
                            .disabled(self.fetch_state == AsyncActionState::Running)
                            .on_click(cx.listener(|page, _, window, cx| {
                                page.fetch_models(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(300.))
                    .min_h(px(120.))
                    .overflow_hidden()
                    .child(
                        List::new(&self.model_list)
                            .search_placeholder(
                                cx.global::<I18n>().t("provider-model-search-placeholder"),
                            )
                            .w_full()
                            .h_full(),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ProviderSettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .gap_5()
            .child(self.render_provider_list(cx))
            .child(self.render_detail(cx))
    }
}

async fn save_provider(
    repository: FreshRepository,
    save: ProviderSaveRequest,
    cx: &mut AsyncWindowContext,
) -> Result<ProviderRecord, String> {
    secret_store::ProviderSecretStore::write_values(cx, &save.secret_refs, &save.writes).await?;

    match save.provider_id {
        Some(provider_id) => repository
            .update_provider(
                &provider_id,
                UpdateProvider {
                    display_name: save.display_name,
                    enabled: save.enabled,
                    settings: save.settings,
                    secret_refs: save.secret_refs,
                },
            )
            .map_err(|err| err.to_string()),
        None => repository
            .insert_provider_with_id(
                save.new_provider_id
                    .expect("new provider saves must include a preallocated id"),
                NewProvider {
                    kind: save.kind,
                    display_name: save.display_name,
                    enabled: save.enabled,
                    settings: save.settings,
                    secret_refs: save.secret_refs,
                },
            )
            .map_err(|err| err.to_string()),
    }
}

async fn fetch_and_store_models(
    repository: FreshRepository,
    provider_id: ProviderId,
    cx: &mut AsyncWindowContext,
) -> Result<usize, ProviderModelFetchError> {
    let provider = repository
        .get_provider(&provider_id)
        .map_err(|err| ProviderModelFetchError::InvalidConfig {
            message: err.to_string(),
        })?
        .ok_or_else(|| ProviderModelFetchError::InvalidConfig {
            message: format!("provider `{provider_id}` was not found"),
        })?;
    let secrets = secret_store::ProviderSecretStore::read_values(cx, &provider.secret_refs)
        .await
        .map_err(|err| ProviderModelFetchError::InvalidConfig { message: err })?;
    let provider_kind = provider.kind.clone();
    let models = gpui_tokio::Tokio::spawn(
        cx,
        fetch_provider_models(ProviderModelFetchRequest {
            provider: provider.clone(),
            secrets,
        }),
    )
    .await
    .map_err(|err| ProviderModelFetchError::ListingFailed {
        provider_kind,
        message: err.to_string(),
    })??;
    let count = models.len();
    repository
        .replace_fetched_provider_models(&provider.id, models)
        .map_err(|err| ProviderModelFetchError::InvalidConfig {
            message: err.to_string(),
        })?;
    Ok(count)
}

fn provider_fetch_precondition(
    provider_id: Option<&ProviderId>,
    dirty: bool,
    support: ModelFetchSupport,
) -> ProviderFetchPrecondition {
    if provider_id.is_none() {
        ProviderFetchPrecondition::SaveProviderFirst
    } else if dirty {
        ProviderFetchPrecondition::SaveChangesFirst
    } else if support == ModelFetchSupport::ManualOnly {
        ProviderFetchPrecondition::ManualModelsRequired
    } else {
        ProviderFetchPrecondition::Ready
    }
}

fn draft_from_spec(spec: &ProviderSpec, provider_id: Option<ProviderId>) -> ProviderDraft {
    ProviderDraft {
        provider_id,
        kind: spec.kind.clone(),
        display_name: spec.display_name.to_string(),
        enabled: false,
        fields: spec
            .fields
            .iter()
            .filter_map(|field| {
                field.default_value.map(|value| {
                    (
                        field.key.to_string(),
                        ProviderDraftValue::String(value.to_string()),
                    )
                })
            })
            .collect(),
        existing_secret_refs: ProviderSecretRefs { refs: Vec::new() },
        dirty: false,
    }
}

fn validate_provider_draft(
    draft: &ProviderDraft,
    spec: &ProviderSpec,
    secrets: &BTreeMap<String, ProviderSecretValidationState>,
    i18n: &I18n,
) -> Result<(), SharedString> {
    for field in &spec.fields {
        if !field.required {
            continue;
        }
        match field.kind {
            ProviderFieldKind::Secret => {
                let valid = secrets
                    .get(field.key)
                    .is_some_and(|secret| secret.has_saved_secret || secret.has_pending_value);
                if !valid {
                    return Err(required_field_message(field.label_key, i18n));
                }
            }
            _ if draft.field_string(field.key).trim().is_empty() => {
                return Err(required_field_message(field.label_key, i18n));
            }
            _ => {}
        }
    }
    Ok(())
}

fn required_field_message(field_label_key: &str, i18n: &I18n) -> SharedString {
    let mut args = FluentArgs::new();
    args.set("field", i18n.t(field_label_key));
    i18n.t_with_args("provider-validation-required", &args)
        .into()
}

fn draft_from_record(provider: &ProviderRecord) -> ProviderDraft {
    ProviderDraft {
        provider_id: Some(provider.id.clone()),
        kind: ProviderKindKey::new(provider.kind.clone()),
        display_name: provider.display_name.clone(),
        enabled: provider.enabled,
        fields: provider
            .settings
            .fields
            .iter()
            .map(|field| {
                let value = match &field.value {
                    ProviderSettingValue::String { value } => {
                        ProviderDraftValue::String(value.clone())
                    }
                    ProviderSettingValue::Bool { value } => ProviderDraftValue::Bool(*value),
                    ProviderSettingValue::Number { value } => ProviderDraftValue::Number(*value),
                    ProviderSettingValue::Object { .. } => {
                        ProviderDraftValue::String(String::new())
                    }
                };
                (field.key.clone(), value)
            })
            .collect(),
        existing_secret_refs: provider.secret_refs.clone(),
        dirty: false,
    }
}

impl From<ProviderModelRecord> for ProviderModelDraft {
    fn from(model: ProviderModelRecord) -> Self {
        Self {
            row_id: Some(model.id),
            provider_id: model.provider_id,
            model_id: model.model_id,
            display_name: model.display_name,
            enabled: model.enabled,
            capabilities: model.capabilities,
            metadata: model.metadata,
            fetched_at: Some(model.fetched_at.to_string()),
            dirty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModelFetchSupport, ProviderFetchPrecondition, ProviderListItem,
        ProviderSecretValidationState, draft_from_record, draft_from_spec, fetch_support,
        provider_fetch_precondition, validate_provider_draft,
    };
    use crate::features::settings::provider::catalog::{
        ModelListingStrategy, ProviderKindKey, builtin_provider_specs,
    };
    use crate::features::settings::provider::draft::{
        ProviderDraftSnapshot, ProviderDraftValue, ProviderModelDraft,
        secret_input_event_marks_dirty,
    };
    use crate::features::settings::provider::list_delegates::{
        ProviderListDelegate, ProviderModelListDelegate, model_list_rows, provider_list_rows,
    };
    use crate::features::settings::provider::secret_store::{
        ProviderSecretStore, ProviderSecretWrite,
    };
    use crate::foundation::I18n;
    use ai_chat_core::{
        ProviderModelMetadata, ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload, conservative_model_capabilities,
    };
    use ai_chat_db::{FreshStore, NewProvider};
    use fluent_bundle::FluentArgs;
    use gpui_component::IndexPath;
    use gpui_component::input::InputEvent;
    use std::collections::{BTreeMap, BTreeSet};
    use tempfile::tempdir;

    #[test]
    fn unsaved_builtin_provider_draft_is_disabled_by_default() {
        let spec = builtin_provider_specs()
            .into_iter()
            .find(|spec| spec.kind.as_str() == "openai")
            .expect("openai provider spec exists");
        let draft = draft_from_spec(&spec, None);

        assert!(!draft.enabled);
    }

    #[test]
    fn saved_provider_draft_uses_record_enabled_state() {
        let dir = tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        let provider = store
            .repository()
            .insert_provider(NewProvider {
                kind: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                enabled: true,
                settings: ProviderSettingsPayload {
                    provider_kind: "openai".to_string(),
                    fields: Vec::new(),
                },
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            })
            .unwrap();
        let draft = draft_from_record(&provider);

        assert!(draft.enabled);
    }

    #[test]
    fn provider_settings_i18n_keys_are_present() {
        let keys = [
            "settings-page-provider",
            "provider-search-placeholder",
            "provider-model-search-placeholder",
            "provider-section-configuration",
            "provider-section-models",
            "provider-action-validate",
            "provider-action-save",
            "provider-action-fetch-models",
            "provider-status-saved",
            "provider-status-unsaved",
            "provider-validation-valid",
            "provider-validation-not-registered",
            "provider-empty-selection",
            "provider-empty-models",
            "provider-field-api-key",
            "provider-field-base-url",
            "provider-field-bearer-token",
            "provider-field-name",
            "provider-field-api-mode",
            "provider-placeholder-api-key",
            "provider-placeholder-base-url-default",
            "provider-placeholder-ollama-base-url",
            "provider-placeholder-bearer-token",
            "provider-placeholder-provider-name",
            "provider-placeholder-custom-base-url",
            "provider-placeholder-api-mode",
            "provider-api-mode-responses",
            "provider-api-mode-chat-completions",
            "provider-description-openai",
            "provider-description-anthropic",
            "provider-description-gemini",
            "provider-description-ollama",
            "provider-description-openrouter",
            "provider-description-deepseek",
            "provider-description-moonshot",
            "provider-description-zai",
            "provider-description-azure-openai",
            "provider-description-mistral",
            "provider-description-xai",
            "provider-description-groq",
            "provider-description-perplexity",
            "provider-description-together",
            "provider-description-custom-openai-compatible",
            "provider-notification-saved",
            "provider-notification-validation-failed",
            "provider-notification-update-model-failed",
            "provider-notification-fetch-success",
            "provider-notification-fetch-failed",
            "provider-notification-fetch-unavailable-title",
            "provider-fetch-save-first",
            "provider-fetch-save-changes-first",
            "provider-fetch-supported-placeholder",
            "provider-fetch-manual-only",
        ];
        let locales = [I18n::english_for_test(), I18n::for_locale_tag("zh-CN")];

        for i18n in locales {
            for key in keys {
                assert_ne!(i18n.t(key), key, "missing provider i18n key {key}");
            }

            let mut args = FluentArgs::new();
            args.set("field", "API Key");
            assert_ne!(
                i18n.t_with_args("provider-validation-required", &args),
                "provider-validation-required"
            );
            let mut args = FluentArgs::new();
            args.set("count", 3);
            assert_ne!(
                i18n.t_with_args("provider-fetch-success-message", &args),
                "provider-fetch-success-message"
            );
        }
    }

    #[test]
    fn validation_rejects_missing_secret_before_repository_write() {
        let spec = builtin_provider_specs()
            .into_iter()
            .find(|spec| spec.kind.as_str() == "openai")
            .expect("openai provider spec exists");
        let draft = draft_from_spec(&spec, None);
        let secrets = BTreeMap::new();
        let i18n = I18n::english_for_test();
        let dir = tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        let repository = store.repository();

        if validate_provider_draft(&draft, &spec, &secrets, &i18n).is_ok() {
            repository
                .insert_provider(NewProvider {
                    kind: draft.kind.as_str().to_string(),
                    display_name: draft.display_name.clone(),
                    enabled: draft.enabled,
                    settings: draft.settings_payload(),
                    secret_refs: ProviderSecretRefs { refs: Vec::new() },
                })
                .unwrap();
        }

        assert!(repository.list_providers().unwrap().is_empty());
    }

    #[test]
    fn validation_accepts_saved_or_pending_secret_consistently() {
        let spec = builtin_provider_specs()
            .into_iter()
            .find(|spec| spec.kind.as_str() == "openai")
            .expect("openai provider spec exists");
        let draft = draft_from_spec(&spec, None);
        let i18n = I18n::english_for_test();
        let saved_secret = BTreeMap::from([(
            "api_key".to_string(),
            ProviderSecretValidationState {
                has_saved_secret: true,
                has_pending_value: false,
            },
        )]);
        let pending_secret = BTreeMap::from([(
            "api_key".to_string(),
            ProviderSecretValidationState {
                has_saved_secret: false,
                has_pending_value: true,
            },
        )]);

        assert!(validate_provider_draft(&draft, &spec, &saved_secret, &i18n).is_ok());
        assert!(validate_provider_draft(&draft, &spec, &pending_secret, &i18n).is_ok());
    }

    #[test]
    fn provider_snapshot_tracks_field_and_enabled_dirty_state() {
        let provider = provider_record_with_base_url(false, "https://old.example/v1");
        let draft = draft_from_record(&provider);
        let saved = ProviderDraftSnapshot::from_draft(&draft);
        let mut changed_field = saved.clone();
        changed_field.fields.insert(
            "base_url".to_string(),
            ProviderDraftValue::String("https://new.example/v1".to_string()),
        );
        let mut changed_enabled = saved.clone();
        changed_enabled.enabled = true;

        assert!(!saved.is_dirty_against(Some(&saved)));
        assert!(changed_field.is_dirty_against(Some(&saved)));
        assert!(changed_enabled.is_dirty_against(Some(&saved)));
    }

    #[test]
    fn provider_snapshot_tracks_secret_dirty_without_persisting_secret_value() {
        let provider = provider_record_with_base_url(false, "https://old.example/v1");
        let draft = draft_from_record(&provider);
        let saved = ProviderDraftSnapshot::from_draft(&draft);
        let mut changed_secret = saved.clone();
        changed_secret.dirty_secret_keys = BTreeSet::from(["api_key".to_string()]);
        let secret_refs = ProviderSecretStore::refs_for(
            "provider-id",
            &[ProviderSecretWrite {
                key: "api_key".to_string(),
                value: "sk-secret-value".to_string(),
            }],
        );

        assert!(changed_secret.is_dirty_against(Some(&saved)));
        assert!(
            secret_refs
                .refs
                .iter()
                .all(|secret| !secret.ref_id.contains("sk-secret-value"))
        );
    }

    #[test]
    fn secret_input_dirty_tracking_only_uses_text_changes() {
        assert!(secret_input_event_marks_dirty(&InputEvent::Change));
        assert!(!secret_input_event_marks_dirty(&InputEvent::Focus));
        assert!(!secret_input_event_marks_dirty(&InputEvent::Blur));
        assert!(!secret_input_event_marks_dirty(&InputEvent::PressEnter {
            secondary: false,
        }));
    }

    #[test]
    fn provider_fetch_precondition_blocks_unsaved_dirty_and_manual_providers() {
        let provider_id = "provider-id".to_string();

        assert_eq!(
            provider_fetch_precondition(None, false, ModelFetchSupport::Supported),
            ProviderFetchPrecondition::SaveProviderFirst
        );
        assert_eq!(
            provider_fetch_precondition(Some(&provider_id), true, ModelFetchSupport::Supported),
            ProviderFetchPrecondition::SaveChangesFirst
        );
        assert_eq!(
            provider_fetch_precondition(Some(&provider_id), false, ModelFetchSupport::ManualOnly),
            ProviderFetchPrecondition::ManualModelsRequired
        );
        assert_eq!(
            provider_fetch_precondition(Some(&provider_id), false, ModelFetchSupport::Supported),
            ProviderFetchPrecondition::Ready
        );
    }

    #[test]
    fn moonshot_provider_uses_manual_model_listing() {
        let moonshot = builtin_provider_specs()
            .into_iter()
            .find(|spec| spec.kind.as_str() == "moonshot")
            .expect("moonshot provider spec exists");

        assert_eq!(moonshot.model_listing, ModelListingStrategy::Manual);
        assert_eq!(
            fetch_support(moonshot.model_listing),
            ModelFetchSupport::ManualOnly
        );
    }

    #[test]
    fn provider_list_delegate_searches_by_brand_kind_and_localized_terms() {
        let providers = builtin_provider_specs()
            .into_iter()
            .map(|spec| ProviderListItem {
                spec,
                provider: None,
            })
            .collect::<Vec<_>>();
        let i18n = I18n::for_locale_tag("zh-CN");
        let mut delegate = provider_list_delegate(&providers, &i18n);

        delegate.set_query_for_test("ollama");
        assert_eq!(delegate.row_count_for_test(), 1);

        delegate.set_query_for_test("提供商");
        assert_eq!(delegate.row_count_for_test(), providers.len());

        delegate.set_query_for_test("模型");
        assert_eq!(delegate.row_count_for_test(), providers.len());
    }

    #[test]
    fn provider_list_selected_index_tracks_selected_kind() {
        let providers = builtin_provider_specs()
            .into_iter()
            .map(|spec| ProviderListItem {
                spec,
                provider: None,
            })
            .collect::<Vec<_>>();
        let i18n = I18n::english_for_test();
        let rows = provider_list_rows(&providers, &i18n);

        assert_eq!(
            ProviderListDelegate::selected_index_for(&rows, &ProviderKindKey::from("openai")),
            Some(IndexPath::new(0))
        );
        assert_eq!(
            ProviderListDelegate::selected_index_for(&rows, &ProviderKindKey::from("missing")),
            None
        );
    }

    #[test]
    fn provider_list_delegate_preserves_query_when_rows_change() {
        let providers = builtin_provider_specs()
            .into_iter()
            .map(|spec| ProviderListItem {
                spec,
                provider: None,
            })
            .collect::<Vec<_>>();
        let i18n = I18n::english_for_test();
        let mut delegate = provider_list_delegate(&providers, &i18n);

        delegate.set_query_for_test("ollama");
        assert_eq!(delegate.row_count_for_test(), 1);
        assert_eq!(
            delegate.kind_for_index(IndexPath::new(0)),
            Some(ProviderKindKey::from("ollama"))
        );
        delegate.set_rows(provider_list_rows(&providers, &i18n));

        assert_eq!(delegate.row_count_for_test(), 1);
        assert_eq!(
            delegate.selected_index_for_kind(&ProviderKindKey::from("ollama")),
            Some(IndexPath::new(0))
        );
        assert_eq!(
            delegate.selected_index_for_kind(&ProviderKindKey::from("openai")),
            None
        );
    }

    #[test]
    fn provider_list_delegate_separates_filtered_rows_except_last() {
        let providers = builtin_provider_specs()
            .into_iter()
            .map(|spec| ProviderListItem {
                spec,
                provider: None,
            })
            .collect::<Vec<_>>();
        let i18n = I18n::english_for_test();
        let mut delegate = provider_list_delegate(&providers, &i18n);

        assert!(delegate.row_separator_for_test(0));
        assert!(!delegate.row_separator_for_test(providers.len() - 1));

        delegate.set_query_for_test("ollama");

        assert_eq!(delegate.row_count_for_test(), 1);
        assert!(!delegate.row_separator_for_test(0));
    }

    #[test]
    fn model_list_delegate_searches_by_id_display_name_capability_and_status() {
        let models = vec![
            provider_model_draft("gpt-5", Some("GPT Five"), true, "openai"),
            provider_model_draft("llama3.2", None, false, "ollama"),
        ];
        let mut delegate = model_list_delegate(&models);

        delegate.set_query_for_test("five");
        assert_eq!(delegate.row_count_for_test(), 1);

        delegate.set_query_for_test("tools");
        assert_eq!(delegate.row_count_for_test(), 2);

        delegate.set_query_for_test("禁用");
        assert_eq!(delegate.row_count_for_test(), 1);
    }

    #[test]
    fn model_list_delegate_index_uses_filtered_rows() {
        let models = vec![
            provider_model_draft("gpt-5", Some("GPT Five"), true, "openai"),
            provider_model_draft("llama3.2", None, false, "ollama"),
        ];
        let mut delegate = model_list_delegate(&models);

        delegate.set_query_for_test("llama");

        let row = delegate
            .row_for_index(IndexPath::new(0))
            .expect("filtered row exists");
        assert_eq!(row.model_id, "llama3.2");
    }

    #[test]
    fn model_list_delegate_preserves_query_when_rows_change() {
        let models = vec![
            provider_model_draft("gpt-5", Some("GPT Five"), true, "openai"),
            provider_model_draft("llama3.2", None, false, "ollama"),
        ];
        let mut delegate = model_list_delegate(&models);

        delegate.set_query_for_test("five");
        assert_eq!(delegate.row_count_for_test(), 1);
        delegate.set_rows(model_list_rows(&[provider_model_draft(
            "llama3.2", None, false, "ollama",
        )]));

        assert_eq!(delegate.row_count_for_test(), 0);
    }

    #[test]
    fn model_list_delegate_separates_rows_except_last() {
        let models = vec![
            provider_model_draft("gpt-5", Some("GPT Five"), true, "openai"),
            provider_model_draft("gpt-5-mini", Some("GPT Five Mini"), true, "openai"),
        ];
        let delegate = model_list_delegate(&models);

        assert!(delegate.row_separator_for_test(0));
        assert!(!delegate.row_separator_for_test(1));
    }

    fn provider_record_with_base_url(enabled: bool, base_url: &str) -> ai_chat_db::ProviderRecord {
        let dir = tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        store
            .repository()
            .insert_provider(NewProvider {
                kind: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                enabled,
                settings: ProviderSettingsPayload {
                    provider_kind: "openai".to_string(),
                    fields: vec![ProviderSettingFieldValue {
                        key: "base_url".to_string(),
                        value: ProviderSettingValue::String {
                            value: base_url.to_string(),
                        },
                    }],
                },
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            })
            .unwrap()
    }

    fn provider_model_draft(
        model_id: &str,
        display_name: Option<&str>,
        enabled: bool,
        provider_kind: &str,
    ) -> ProviderModelDraft {
        ProviderModelDraft {
            row_id: None,
            provider_id: "provider-id".to_string(),
            model_id: model_id.to_string(),
            display_name: display_name.map(ToString::to_string),
            enabled,
            capabilities: conservative_model_capabilities(provider_kind),
            metadata: ProviderModelMetadata {
                display_name: None,
                family: None,
                raw: None,
            },
            fetched_at: Some("2026-06-02T00:00:00Z".to_string()),
            dirty: false,
        }
    }

    fn provider_list_delegate(providers: &[ProviderListItem], i18n: &I18n) -> ProviderListDelegate {
        ProviderListDelegate::new(provider_list_rows(providers, i18n), "empty")
    }

    fn model_list_delegate(models: &[ProviderModelDraft]) -> ProviderModelListDelegate {
        ProviderModelListDelegate::new(model_list_rows(models), "empty")
    }
}
