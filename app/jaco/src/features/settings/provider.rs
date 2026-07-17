use std::collections::{BTreeMap, BTreeSet};

use crate::{
    database,
    foundation::{
        I18n,
        assets::{IconName, provider_visual_icon},
    },
    state,
};
use fluent_bundle::FluentArgs;
use gpui::{StatefulInteractiveElement as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    form::field as component_form_field,
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::{List, ListEvent, ListState},
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    searchable_list::SearchableListDelegate,
    select::{Select, SelectState},
    switch::Switch,
    tag::Tag,
    v_flex,
};
use gpui_form::{
    ErrorParamValue, FieldError, FormStoreEvent, FormValidationReport, SubmitError, SubscriptionSet,
};
use jaco_agent::{ProviderModelFetchError, ProviderModelFetchRequest, fetch_provider_models};
use jaco_core::{
    ProviderId, ProviderSecretRefs, ProviderSettingValue, ProviderSettingsPayload, new_id,
};
use jaco_db::{
    FreshRepository, NewProvider, NewProviderModel, ProviderModelRecord, ProviderRecord,
    UpdateProvider,
};
use tracing::{Level, event};

mod capabilities;
mod catalog;
mod components;
mod draft;
mod forms;
mod list_delegates;
mod model_fetch;

use self::{
    catalog::{ProviderFormKind, ProviderKindKey, ProviderSpec, builtin_provider_specs},
    draft::{
        ManualModelEditor, ProviderDraft, ProviderDraftSnapshot, ProviderDraftValue,
        ProviderModelDraft, ProviderValidationState,
    },
    forms::{
        ApiKeyProviderFormField, ApiKeyProviderFormStore, ApiModeChoice,
        CustomOpenAiProviderFormField, CustomOpenAiProviderFormStore, OllamaProviderFormField,
        OllamaProviderFormStore, ProviderFormField, ProviderSettingsForm,
        ProviderSettingsFormOutput, bind_provider_secret, field_errors, localized_api_mode_choices,
    },
    list_delegates::{
        ProviderListDelegate, ProviderModelListDelegate, model_list_rows, provider_list_rows,
    },
    model_fetch::{ModelFetchSupport, fetch_support},
};
use state::provider_secrets::{ProviderSecretStore, ProviderSecretWrite};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderListItem {
    spec: ProviderSpec,
    provider: Option<ProviderRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderEditorKey {
    kind: ProviderKindKey,
}

enum ProviderFormComponents {
    ApiKey {
        api_key: Entity<gpui_component::input::InputState>,
        base_url: Entity<gpui_component::input::InputState>,
    },
    Ollama {
        base_url: Entity<gpui_component::input::InputState>,
        bearer_token: Entity<gpui_component::input::InputState>,
    },
    CustomOpenAi {
        name: Entity<gpui_component::input::InputState>,
        api_key: Entity<gpui_component::input::InputState>,
        base_url: Entity<gpui_component::input::InputState>,
        api_mode: Entity<SelectState<Vec<ApiModeChoice>>>,
    },
}

fn new_provider_input<T>(
    value: String,
    placeholder: String,
    masked: bool,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<InputState>
where
    T: 'static,
{
    cx.new(|cx| {
        let mut input = InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder);
        if masked {
            input = input.masked(true);
        }
        input
    })
}

impl ProviderFormComponents {
    fn bind<T>(
        form: &ProviderSettingsForm,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> (Self, SubscriptionSet)
    where
        T: 'static,
    {
        let mut subscriptions = SubscriptionSet::new();

        match form {
            ProviderSettingsForm::ApiKey(form) => {
                let base_url = {
                    let form = form.read(cx);
                    form.base_url_draft()
                };
                let api_key_input = new_provider_input(
                    String::new(),
                    cx.global::<I18n>().t("provider-placeholder-api-key"),
                    true,
                    window,
                    cx,
                );
                let base_url_input = new_provider_input(
                    base_url,
                    cx.global::<I18n>()
                        .t("provider-placeholder-base-url-default"),
                    false,
                    window,
                    cx,
                );
                subscriptions.extend(
                    bind_provider_secret(
                        ApiKeyProviderFormStore::api_key_handle(form),
                        &api_key_input,
                        window,
                        cx,
                    )
                    .expect("bind provider API key input"),
                );
                subscriptions.extend(
                    gpui_form_gpui_component::bind_input(
                        ApiKeyProviderFormStore::base_url_handle(form),
                        &base_url_input,
                        window,
                        cx,
                    )
                    .expect("bind provider base URL input"),
                );
                (
                    Self::ApiKey {
                        api_key: api_key_input,
                        base_url: base_url_input,
                    },
                    subscriptions,
                )
            }
            ProviderSettingsForm::Ollama(form) => {
                let base_url = {
                    let form = form.read(cx);
                    form.base_url_draft()
                };
                let base_url_input = new_provider_input(
                    base_url,
                    cx.global::<I18n>()
                        .t("provider-placeholder-ollama-base-url"),
                    false,
                    window,
                    cx,
                );
                let bearer_token_input = new_provider_input(
                    String::new(),
                    cx.global::<I18n>().t("provider-placeholder-bearer-token"),
                    true,
                    window,
                    cx,
                );
                subscriptions.extend(
                    gpui_form_gpui_component::bind_input(
                        OllamaProviderFormStore::base_url_handle(form),
                        &base_url_input,
                        window,
                        cx,
                    )
                    .expect("bind Ollama base URL input"),
                );
                subscriptions.extend(
                    bind_provider_secret(
                        OllamaProviderFormStore::bearer_token_handle(form),
                        &bearer_token_input,
                        window,
                        cx,
                    )
                    .expect("bind Ollama bearer token input"),
                );
                (
                    Self::Ollama {
                        base_url: base_url_input,
                        bearer_token: bearer_token_input,
                    },
                    subscriptions,
                )
            }
            ProviderSettingsForm::CustomOpenAi(form) => {
                let (name, base_url, api_mode) = {
                    let form = form.read(cx);
                    (
                        form.name_draft(),
                        form.base_url_draft(),
                        form.api_mode_draft(),
                    )
                };
                let name_input = new_provider_input(
                    name,
                    cx.global::<I18n>().t("provider-placeholder-provider-name"),
                    false,
                    window,
                    cx,
                );
                let api_key_input = new_provider_input(
                    String::new(),
                    cx.global::<I18n>().t("provider-placeholder-api-key"),
                    true,
                    window,
                    cx,
                );
                let base_url_input = new_provider_input(
                    base_url,
                    cx.global::<I18n>()
                        .t("provider-placeholder-custom-base-url"),
                    false,
                    window,
                    cx,
                );
                let choices = localized_api_mode_choices(cx.global::<I18n>());
                let api_mode_input = cx.new(|cx| {
                    SelectState::new(choices.clone(), choices.position(&api_mode), window, cx)
                });
                subscriptions.extend(
                    gpui_form_gpui_component::bind_input(
                        CustomOpenAiProviderFormStore::name_handle(form),
                        &name_input,
                        window,
                        cx,
                    )
                    .expect("bind custom provider name input"),
                );
                subscriptions.extend(
                    bind_provider_secret(
                        CustomOpenAiProviderFormStore::api_key_handle(form),
                        &api_key_input,
                        window,
                        cx,
                    )
                    .expect("bind custom provider API key input"),
                );
                subscriptions.extend(
                    gpui_form_gpui_component::bind_input(
                        CustomOpenAiProviderFormStore::base_url_handle(form),
                        &base_url_input,
                        window,
                        cx,
                    )
                    .expect("bind custom provider base URL input"),
                );
                subscriptions.extend(
                    gpui_form_gpui_component::bind_select(
                        CustomOpenAiProviderFormStore::api_mode_handle(form),
                        &api_mode_input,
                        window,
                        cx,
                    )
                    .expect("bind custom provider API mode select"),
                );
                (
                    Self::CustomOpenAi {
                        name: name_input,
                        api_key: api_key_input,
                        base_url: base_url_input,
                        api_mode: api_mode_input,
                    },
                    subscriptions,
                )
            }
        }
    }
}

impl ProviderEditorKey {
    fn new(kind: ProviderKindKey) -> Self {
        Self { kind }
    }

    fn kind(&self) -> &ProviderKindKey {
        &self.kind
    }
}

struct ProviderEditorState {
    draft: ProviderDraft,
    form: ProviderSettingsForm,
    models: Vec<ProviderModelDraft>,
    saved_snapshot: Option<ProviderDraftSnapshot>,
    validation: ProviderValidationState,
    #[allow(dead_code)]
    manual_model_editor: Option<Entity<ManualModelEditor>>,
    components: ProviderFormComponents,
    _field_subscriptions: SubscriptionSet,
    fetch_task: Option<Task<()>>,
}

impl ProviderEditorState {
    fn new(
        draft: ProviderDraft,
        form: ProviderSettingsForm,
        models: Vec<ProviderModelDraft>,
        window: &mut Window,
        cx: &mut Context<ProviderSettingsPage>,
    ) -> Self {
        let (components, _field_subscriptions) = ProviderFormComponents::bind(&form, window, cx);
        Self {
            draft,
            form,
            models,
            saved_snapshot: None,
            validation: ProviderValidationState::Idle,
            manual_model_editor: None,
            components,
            _field_subscriptions,
            fetch_task: None,
        }
    }
}

fn editor_is_saving(editor: &ProviderEditorState, cx: &App) -> bool {
    editor.form.is_submitting(cx)
}

fn editor_is_fetching(editor: &ProviderEditorState) -> bool {
    editor.fetch_task.is_some()
}

pub(super) struct ProviderSettingsPage {
    provider_list: Entity<ListState<ProviderListDelegate>>,
    model_list: Entity<ListState<ProviderModelListDelegate>>,
    detail_scroll_handle: ScrollHandle,
    selected_key: ProviderEditorKey,
    providers: Vec<ProviderListItem>,
    editors: BTreeMap<ProviderEditorKey, ProviderEditorState>,
    _list_subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
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
    writes: Vec<ProviderSecretWrite>,
}

struct ProviderModelFetchResult {
    provider_id: ProviderId,
    models: Vec<NewProviderModel>,
}

impl ProviderSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let providers = Self::load_provider_list(cx).unwrap_or_else(|err| {
            event!(Level::ERROR, error = ?err, "load provider settings failed");
            Vec::new()
        });
        let selected_key = providers
            .first()
            .map(|item| ProviderEditorKey::new(item.spec.kind.clone()))
            .unwrap_or_else(|| ProviderEditorKey::new("custom_openai_compatible".into()));
        let editors = providers
            .iter()
            .map(|item| {
                let key = ProviderEditorKey::new(item.spec.kind.clone());
                let editor = Self::editor_for_item(item, window, cx);
                (key, editor)
            })
            .collect::<BTreeMap<_, _>>();
        let provider_rows = provider_list_rows(&providers, cx.global::<I18n>());
        let provider_selected_index =
            ProviderListDelegate::selected_index_for(&provider_rows, selected_key.kind());
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
        let model_rows = editors
            .get(&selected_key)
            .map(|editor| model_list_rows(&editor.models))
            .unwrap_or_default();
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
        Self {
            provider_list,
            model_list,
            detail_scroll_handle: ScrollHandle::default(),
            selected_key,
            providers,
            editors,
            _list_subscriptions: vec![provider_list_subscription, model_list_subscription],
            _load_task: None,
        }
    }

    fn load_provider_list(cx: &App) -> jaco_db::Result<Vec<ProviderListItem>> {
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

    fn editor_for_item(
        item: &ProviderListItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ProviderEditorState {
        let draft = Self::draft_for_item(item);
        let models = Self::load_models_for_draft(&draft, cx).unwrap_or_default();
        let form = ProviderSettingsForm::new(item.spec.form_kind, &draft, window, cx);
        let mut editor = ProviderEditorState::new(draft, form, models, window, cx);
        Self::rebuild_editor_form(&mut editor, &item.spec, window, cx);
        editor.saved_snapshot = Some(Self::snapshot_for_editor(&editor, cx));
        editor
    }

    fn draft_for_item(item: &ProviderListItem) -> ProviderDraft {
        item.provider
            .as_ref()
            .map(draft_from_record)
            .unwrap_or_else(|| draft_from_spec(&item.spec, None))
    }

    fn load_models_for_draft(
        draft: &ProviderDraft,
        cx: &App,
    ) -> jaco_db::Result<Vec<ProviderModelDraft>> {
        let Some(provider_id) = draft.provider_id.as_ref() else {
            return Ok(Vec::new());
        };
        database::repository(cx)
            .list_provider_models(provider_id)?
            .into_iter()
            .map(|model| Ok(model.into()))
            .collect()
    }

    fn rebuild_editor_form(
        editor: &mut ProviderEditorState,
        spec: &ProviderSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        editor._field_subscriptions.clear();
        editor.form = ProviderSettingsForm::new(spec.form_kind, &editor.draft, window, cx);
        let (components, mut subscriptions) =
            ProviderFormComponents::bind(&editor.form, window, cx);
        editor.components = components;
        match &editor.form {
            ProviderSettingsForm::ApiKey(form) => {
                let form_id = form.entity_id();
                subscriptions.push(cx.subscribe_in(
                    form,
                    window,
                    move |page,
                          _form,
                          _event: &FormStoreEvent<ApiKeyProviderFormField>,
                          _window,
                          cx| {
                        page.on_provider_form_changed(form_id, cx);
                    },
                ));
            }
            ProviderSettingsForm::Ollama(form) => {
                let form_id = form.entity_id();
                subscriptions.push(cx.subscribe_in(
                    form,
                    window,
                    move |page,
                          _form,
                          _event: &FormStoreEvent<OllamaProviderFormField>,
                          _window,
                          cx| {
                        page.on_provider_form_changed(form_id, cx);
                    },
                ));
            }
            ProviderSettingsForm::CustomOpenAi(form) => {
                let form_id = form.entity_id();
                subscriptions.push(cx.subscribe_in(
                    form,
                    window,
                    move |page,
                          _form,
                          _event: &FormStoreEvent<CustomOpenAiProviderFormField>,
                          _window,
                          cx| {
                        page.on_provider_form_changed(form_id, cx);
                    },
                ));
            }
        }
        editor._field_subscriptions = subscriptions;
    }

    fn on_provider_form_changed(&mut self, form_id: EntityId, cx: &mut Context<Self>) {
        let Some(key) = self.editor_key_for_form_id(form_id) else {
            return;
        };
        let Some(editor) = self.editors.get_mut(&key) else {
            return;
        };
        editor.validation = ProviderValidationState::Idle;
        cx.notify();
    }

    fn editor_key_for_form_id(&self, form_id: EntityId) -> Option<ProviderEditorKey> {
        self.editors
            .iter()
            .find_map(|(key, editor)| (editor.form.entity_id() == form_id).then(|| key.clone()))
    }

    fn selected_editor(&self) -> Option<&ProviderEditorState> {
        self.editors.get(&self.selected_key)
    }

    fn selected_editor_mut(&mut self) -> Option<&mut ProviderEditorState> {
        self.editors.get_mut(&self.selected_key)
    }

    fn selected_editor_is_saving(&self, cx: &App) -> bool {
        self.selected_editor()
            .is_some_and(|editor| editor_is_saving(editor, cx))
    }

    fn selected_spec(&self) -> Option<&ProviderSpec> {
        self.spec_for_key(&self.selected_key)
    }

    fn spec_for_key(&self, key: &ProviderEditorKey) -> Option<&ProviderSpec> {
        self.providers
            .iter()
            .find(|item| &item.spec.kind == key.kind())
            .map(|item| &item.spec)
    }

    fn select_provider_from_list(
        &mut self,
        kind: ProviderKindKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_key = ProviderEditorKey::new(kind);
        if self.selected_key == next_key {
            return;
        }
        self.selected_key = next_key;
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
        if self.selected_key.kind == kind {
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
        if self.selected_editor_is_saving(cx) {
            return;
        }
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

    fn sync_list_delegates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_provider_list(window, cx);
        self.sync_model_list(window, cx);
    }

    fn sync_provider_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = provider_list_rows(&self.providers, cx.global::<I18n>());
        self.provider_list.update(cx, |list, cx| {
            list.delegate_mut().set_rows(rows);
            let selected_index = list
                .delegate()
                .selected_index_for_kind(self.selected_key.kind());
            list.set_selected_index(selected_index, window, cx);
        });
    }

    fn sync_model_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self
            .selected_editor()
            .map(|editor| model_list_rows(&editor.models))
            .unwrap_or_default();
        let locked = self.selected_editor_is_saving(cx);
        self.model_list.update(cx, |list, cx| {
            list.delegate_mut().set_rows(rows);
            list.delegate_mut().set_disabled(locked);
            list.set_selected_index(None, window, cx);
        });
    }

    fn validate_current_output(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let key = self.selected_key.clone();
        if self.spec_for_key(&key).is_none() {
            if let Some(editor) = self.editors.get_mut(&key) {
                editor.validation = ProviderValidationState::Invalid(
                    cx.global::<I18n>()
                        .t("provider-validation-not-registered")
                        .into(),
                );
            }
            return false;
        }
        let Some(editor) = self.editors.get(&key) else {
            return false;
        };
        let validation_report =
            editor
                .form
                .validate_current(editor.draft.existing_secret_refs.clone(), window, cx);
        let validation = provider_validation_state_from_report(&validation_report, cx);
        let valid = matches!(validation, ProviderValidationState::Valid);
        if let Some(editor) = self.editors.get_mut(&key) {
            editor.validation = validation;
        }
        valid
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.selected_key.clone();
        if self
            .editors
            .get(&key)
            .is_some_and(|editor| editor_is_saving(editor, cx))
        {
            return;
        }
        let Some(editor) = self.editors.get(&key) else {
            return;
        };
        let provider_id = editor.draft.provider_id.clone();
        let new_provider_id = provider_id.is_none().then(new_id);
        let secret_ref_owner = provider_id
            .as_deref()
            .or(new_provider_id.as_deref())
            .expect("new provider id is preallocated before saving secrets");
        let secret_ref_owner = secret_ref_owner.to_string();
        let kind = editor.draft.kind.as_str().to_string();
        let display_name_fallback = editor.draft.display_name.clone();
        let existing_secret_refs = editor.draft.existing_secret_refs.clone();
        let page = cx.entity().downgrade();
        let task_key = key.clone();
        let start = editor.form.submit_async_save(
            existing_secret_refs.clone(),
            move |output, window, cx| {
                let writes = Self::secret_writes_for_output(&output);
                let secret_refs = Self::secret_refs_for_output(
                    &existing_secret_refs,
                    &secret_ref_owner,
                    &writes,
                    &output,
                );
                let save = ProviderSaveRequest {
                    provider_id,
                    new_provider_id,
                    kind: kind.clone(),
                    display_name: output.display_name(&display_name_fallback),
                    enabled: output.enabled(),
                    settings: output.settings_payload(&kind),
                    secret_refs,
                    writes,
                };
                window.spawn(cx, async move |cx| {
                    let result = write_provider_secrets(save, cx).await;
                    match page.update_in(cx, |page, window, cx| {
                        page.finish_save(task_key, result, window, cx)
                    }) {
                        Ok(result) => result,
                        Err(err) => {
                            event!(Level::ERROR, error = ?err, "finish provider save failed");
                            Err(err.to_string())
                        }
                    }
                })
            },
            window,
            cx,
        );
        match start {
            Ok(()) => {}
            Err(SubmitError::Invalid(report)) => {
                let validation = provider_validation_state_from_report(&report, cx);
                if let Some(editor) = self.editors.get_mut(&key) {
                    editor.validation = validation;
                }
                cx.notify();
                return;
            }
            Err(SubmitError::Handler(())) | Err(SubmitError::Busy) => {
                return;
            }
        }
        if self.selected_key == key {
            self.sync_model_list(window, cx);
        }
        cx.notify();
    }

    fn finish_save(
        &mut self,
        key: ProviderEditorKey,
        result: Result<ProviderSaveRequest, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let selected = self.selected_key == key;
        if selected {
            self.sync_model_list(window, cx);
        }
        let result =
            result.and_then(|save| persist_provider_save(save, cx).map_err(|err| err.to_string()));
        match result {
            Ok(provider) => {
                let draft = draft_from_record(&provider);
                let models = Self::load_models_for_draft(&draft, cx).unwrap_or_default();
                self.providers = Self::load_provider_list(cx).unwrap_or_default();
                let spec = self.spec_for_key(&key).cloned();
                if let (Some(editor), Some(spec)) = (self.editors.get_mut(&key), spec) {
                    editor.draft = draft;
                    editor.models = models;
                    editor.validation = ProviderValidationState::Valid;
                    Self::rebuild_editor_form(editor, &spec, window, cx);
                    editor.saved_snapshot = Some(Self::snapshot_for_editor(editor, cx));
                }
                self.sync_list_delegates(window, cx);
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("provider-notification-saved"))
                        .message(provider.display_name)
                        .with_type(NotificationType::Success),
                    cx,
                );
                cx.notify();
                Ok(())
            }
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("notify-save-settings-failed"))
                        .message(err.clone())
                        .with_type(NotificationType::Error),
                    cx,
                );
                cx.notify();
                Err(err)
            }
        }
    }

    fn snapshot_for_editor(editor: &ProviderEditorState, cx: &App) -> ProviderDraftSnapshot {
        let mut snapshot = ProviderDraftSnapshot::from_draft(&editor.draft);
        let output = editor.form.current_output(cx);
        snapshot.enabled = output.enabled();
        snapshot.fields = output.persistent_fields();
        snapshot.dirty_secret_keys = output
            .dirty_secret_keys()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<BTreeSet<_>>();
        snapshot
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.is_editor_dirty(&self.selected_key, cx)
    }

    fn is_editor_dirty(&self, key: &ProviderEditorKey, cx: &App) -> bool {
        self.editors.get(key).is_some_and(|editor| {
            Self::snapshot_for_editor(editor, cx).is_dirty_against(editor.saved_snapshot.as_ref())
        })
    }

    fn secret_writes_for_output(output: &ProviderSettingsFormOutput) -> Vec<ProviderSecretWrite> {
        output
            .secret_fields()
            .into_iter()
            .filter_map(|secret| {
                (!secret.value.is_empty()).then_some(ProviderSecretWrite {
                    key: secret.key().to_string(),
                    value: secret.value,
                })
            })
            .collect()
    }

    fn secret_refs_for_output(
        existing_secret_refs: &ProviderSecretRefs,
        provider_id: &str,
        writes: &[ProviderSecretWrite],
        output: &ProviderSettingsFormOutput,
    ) -> ProviderSecretRefs {
        let mut refs = ProviderSecretStore::refs_for(provider_id, writes);
        for saved in &existing_secret_refs.refs {
            if !refs.refs.iter().any(|secret| secret.key == saved.key) {
                let changed = output
                    .secret_fields()
                    .into_iter()
                    .any(|secret| secret.key() == saved.key && secret.changed);
                if !changed {
                    refs.refs.push(saved.clone());
                }
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
        let key = self.selected_key.clone();
        let Some(draft) = self.editors.get(&key).map(|editor| editor.draft.clone()) else {
            return;
        };
        let Some(provider_id) = draft.provider_id.clone() else {
            return;
        };
        match state::providers::set_provider_model_enabled(cx, &provider_id, &model_id, enabled) {
            Ok(_) => {
                if let Some(editor) = self.editors.get_mut(&key) {
                    editor.models = Self::load_models_for_draft(&draft, cx).unwrap_or_default();
                }
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
        let key = self.selected_key.clone();
        if self
            .editors
            .get(&key)
            .is_some_and(|editor| editor_is_fetching(editor) || editor_is_saving(editor, cx))
        {
            return;
        }
        let Some(spec) = self.selected_spec() else {
            return;
        };
        let support = fetch_support(spec.model_listing);
        let Some(editor) = self.editors.get(&key) else {
            return;
        };
        let precondition = provider_fetch_precondition(
            editor.draft.provider_id.as_ref(),
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
        let Some(provider_id) = editor.draft.provider_id.clone() else {
            return;
        };
        let repository = database::repository(cx);
        let page = cx.entity().downgrade();
        let task_key = key.clone();
        let task = window.spawn(cx, async move |cx| {
            let result = fetch_provider_models_for_provider(repository, provider_id, cx).await;
            if let Err(err) = page.update_in(cx, |page, window, cx| {
                page.finish_fetch(task_key, result, window, cx);
            }) {
                event!(Level::ERROR, error = ?err, "finish provider model fetch failed");
            }
        });
        if let Some(editor) = self.editors.get_mut(&key) {
            editor.fetch_task = Some(task);
        }
        cx.notify();
    }

    fn finish_fetch(
        &mut self,
        key: ProviderEditorKey,
        result: Result<ProviderModelFetchResult, ProviderModelFetchError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.editors.get_mut(&key) {
            editor.fetch_task = None;
        }
        match result {
            Ok(result) => {
                let count = result.models.len();
                if let Err(err) = state::providers::replace_fetched_provider_models(
                    cx,
                    &result.provider_id,
                    result.models,
                ) {
                    window.push_notification(
                        Notification::new()
                            .title(cx.global::<I18n>().t("provider-notification-fetch-failed"))
                            .message(err.to_string())
                            .with_type(NotificationType::Error),
                        cx,
                    );
                    cx.notify();
                    return;
                }
                let draft = self.editors.get(&key).map(|editor| editor.draft.clone());
                if let (Some(editor), Some(draft)) = (self.editors.get_mut(&key), draft) {
                    editor.models = Self::load_models_for_draft(&draft, cx).unwrap_or_default();
                }
                if self.selected_key == key {
                    self.sync_model_list(window, cx);
                }
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
        let Some(editor) = self.selected_editor() else {
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
            .child(self.render_header(spec, editor, cx))
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
                                    .child(self.render_config(spec, editor, cx))
                                    .child(self.render_models(editor, cx)),
                            ),
                    )
                    .vertical_scrollbar(&self.detail_scroll_handle),
            )
            .into_any_element()
    }

    fn render_header(
        &self,
        spec: &ProviderSpec,
        editor: &ProviderEditorState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dirty = self.is_dirty(cx);
        let locked = editor_is_saving(editor, cx);
        let enabled = editor.form.enabled(cx);
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
                            .child(
                                provider_visual_icon(spec.visual)
                                    .size_5()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(Label::new(spec.display_name).text_lg().font_medium())
                            .when(editor.draft.provider_id.is_some() && !dirty, |this| {
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
                    .checked(enabled)
                    .disabled(locked)
                    .on_click(cx.listener(|page, checked, window, cx| {
                        if let Some(editor) = page.selected_editor_mut() {
                            editor.form.set_enabled(*checked, window, cx);
                            editor.validation = ProviderValidationState::Idle;
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_config(
        &self,
        _spec: &ProviderSpec,
        editor: &ProviderEditorState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locked = editor_is_saving(editor, cx);
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
            .children(self.render_config_fields(&editor.form, &editor.components, locked, cx))
            .child(self.render_validation_state(&editor.validation, cx))
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
                            .disabled(locked)
                            .on_click(cx.listener(|page, _, window, cx| {
                                page.validate_current_output(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("provider-settings-save")
                            .icon(IconName::Check)
                            .label(cx.global::<I18n>().t("provider-action-save"))
                            .small()
                            .primary()
                            .loading(editor_is_saving(editor, cx))
                            .disabled(editor_is_saving(editor, cx))
                            .on_click(cx.listener(|page, _, window, cx| page.save(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_config_fields(
        &self,
        form: &ProviderSettingsForm,
        components: &ProviderFormComponents,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match form {
            ProviderSettingsForm::ApiKey(form) => {
                let ProviderFormComponents::ApiKey { api_key, base_url } = components else {
                    unreachable!("provider API key form and controls must match")
                };
                let (api_key_errors, api_key_required, base_url_errors, base_url_required) = {
                    let form = form.read(cx);
                    (
                        field_errors(&form.api_key),
                        form.api_key_required(),
                        field_errors(&form.base_url),
                        form.base_url_required(),
                    )
                };
                vec![
                    self.render_secret_input_row(
                        ProviderFormField::ApiKey,
                        api_key.clone(),
                        api_key_errors,
                        api_key_required,
                        locked,
                        cx,
                    ),
                    self.render_text_input_row(
                        ProviderFormField::BaseUrl,
                        base_url.clone(),
                        base_url_errors,
                        base_url_required,
                        locked,
                        cx,
                    ),
                ]
            }
            ProviderSettingsForm::Ollama(form) => {
                let ProviderFormComponents::Ollama {
                    base_url,
                    bearer_token,
                } = components
                else {
                    unreachable!("provider Ollama form and controls must match")
                };
                let (
                    base_url_errors,
                    base_url_required,
                    bearer_token_errors,
                    bearer_token_required,
                ) = {
                    let form = form.read(cx);
                    (
                        field_errors(&form.base_url),
                        form.base_url_required(),
                        field_errors(&form.bearer_token),
                        form.bearer_token_required(),
                    )
                };
                vec![
                    self.render_text_input_row(
                        ProviderFormField::BaseUrl,
                        base_url.clone(),
                        base_url_errors,
                        base_url_required,
                        locked,
                        cx,
                    ),
                    self.render_secret_input_row(
                        ProviderFormField::BearerToken,
                        bearer_token.clone(),
                        bearer_token_errors,
                        bearer_token_required,
                        locked,
                        cx,
                    ),
                ]
            }
            ProviderSettingsForm::CustomOpenAi(form) => {
                let ProviderFormComponents::CustomOpenAi {
                    name,
                    api_key,
                    base_url,
                    api_mode,
                } = components
                else {
                    unreachable!("provider custom form and controls must match")
                };
                let (
                    name_errors,
                    name_required,
                    api_key_errors,
                    api_key_required,
                    base_url_errors,
                    base_url_required,
                    api_mode_errors,
                    api_mode_required,
                ) = {
                    let form = form.read(cx);
                    (
                        field_errors(&form.name),
                        form.name_required(),
                        field_errors(&form.api_key),
                        form.api_key_required(),
                        field_errors(&form.base_url),
                        form.base_url_required(),
                        field_errors(&form.api_mode),
                        form.api_mode_required(),
                    )
                };
                vec![
                    self.render_text_input_row(
                        ProviderFormField::Name,
                        name.clone(),
                        name_errors,
                        name_required,
                        locked,
                        cx,
                    ),
                    self.render_secret_input_row(
                        ProviderFormField::ApiKey,
                        api_key.clone(),
                        api_key_errors,
                        api_key_required,
                        locked,
                        cx,
                    ),
                    self.render_text_input_row(
                        ProviderFormField::BaseUrl,
                        base_url.clone(),
                        base_url_errors,
                        base_url_required,
                        locked,
                        cx,
                    ),
                    self.render_select_row(
                        ProviderFormField::ApiMode,
                        api_mode.clone(),
                        api_mode_errors,
                        api_mode_required,
                        locked,
                        cx,
                    ),
                ]
            }
        }
    }

    fn render_text_input_row(
        &self,
        field: ProviderFormField,
        input: Entity<gpui_component::input::InputState>,
        errors: Vec<FieldError>,
        required: bool,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        component_form_field()
            .label(cx.global::<I18n>().t(field.label_key()))
            .required(required)
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(Input::new(&input).w_full().disabled(locked))
                    .child(provider_field_error_list(errors, cx)),
            )
            .into_any_element()
    }

    fn render_secret_input_row(
        &self,
        field: ProviderFormField,
        input: Entity<gpui_component::input::InputState>,
        errors: Vec<FieldError>,
        required: bool,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        component_form_field()
            .label(cx.global::<I18n>().t(field.label_key()))
            .required(required)
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(Input::new(&input).w_full().disabled(locked).mask_toggle())
                    .child(provider_field_error_list(errors, cx)),
            )
            .into_any_element()
    }

    fn render_select_row(
        &self,
        field: ProviderFormField,
        select: Entity<gpui_component::select::SelectState<Vec<forms::ApiModeChoice>>>,
        errors: Vec<FieldError>,
        required: bool,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        component_form_field()
            .label(cx.global::<I18n>().t(field.label_key()))
            .required(required)
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(Select::new(&select).w_full().disabled(locked))
                    .child(provider_field_error_list(errors, cx)),
            )
            .into_any_element()
    }

    fn render_validation_state(
        &self,
        validation: &ProviderValidationState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match validation {
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

    fn render_models(&self, editor: &ProviderEditorState, cx: &mut Context<Self>) -> AnyElement {
        let locked = editor_is_saving(editor, cx);
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
                            .loading(editor_is_fetching(editor))
                            .disabled(editor_is_fetching(editor) || locked)
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

fn provider_field_error_list(errors: Vec<FieldError>, cx: &mut App) -> AnyElement {
    if errors.is_empty() {
        return div().into_any_element();
    }

    v_flex()
        .w_full()
        .gap_1()
        .children(
            errors
                .into_iter()
                .map(|error| provider_error_label(provider_field_error_message(&error, cx), cx)),
        )
        .into_any_element()
}

fn provider_field_error_message(error: &FieldError, cx: &App) -> SharedString {
    let i18n = cx.global::<I18n>();
    if let Some(ErrorParamValue::String(field)) = error.params.get("field") {
        let mut args = FluentArgs::new();
        args.set("field", field.to_string());
        return i18n.t_with_args(error.message_key.as_ref(), &args).into();
    }
    i18n.t(error.message_key.as_ref()).into()
}

fn provider_validation_state_from_report(
    report: &FormValidationReport,
    cx: &App,
) -> ProviderValidationState {
    report
        .first_field_error()
        .map(|error| ProviderValidationState::Invalid(provider_field_error_message(error, cx)))
        .unwrap_or(ProviderValidationState::Valid)
}

fn provider_error_label(message: SharedString, cx: &mut App) -> AnyElement {
    Label::new(message)
        .text_xs()
        .line_height(px(16.))
        .text_color(cx.theme().danger)
        .into_any_element()
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

async fn write_provider_secrets(
    save: ProviderSaveRequest,
    cx: &mut AsyncWindowContext,
) -> Result<ProviderSaveRequest, String> {
    ProviderSecretStore::write_values(cx, &save.secret_refs, &save.writes).await?;
    Ok(save)
}

fn persist_provider_save(
    save: ProviderSaveRequest,
    cx: &mut App,
) -> jaco_db::Result<ProviderRecord> {
    match save.provider_id {
        Some(provider_id) => state::providers::update_provider(
            cx,
            &provider_id,
            UpdateProvider {
                display_name: save.display_name,
                enabled: save.enabled,
                settings: save.settings,
                secret_refs: save.secret_refs,
            },
        ),
        None => state::providers::insert_provider_with_id(
            cx,
            save.new_provider_id
                .expect("new provider saves must include a preallocated id"),
            NewProvider {
                kind: save.kind,
                display_name: save.display_name,
                enabled: save.enabled,
                settings: save.settings,
                secret_refs: save.secret_refs,
            },
        ),
    }
}

async fn fetch_provider_models_for_provider(
    repository: FreshRepository,
    provider_id: ProviderId,
    cx: &mut AsyncWindowContext,
) -> Result<ProviderModelFetchResult, ProviderModelFetchError> {
    let provider = repository
        .get_provider(&provider_id)
        .map_err(|err| ProviderModelFetchError::InvalidConfig {
            message: err.to_string(),
        })?
        .ok_or_else(|| ProviderModelFetchError::InvalidConfig {
            message: format!("provider `{provider_id}` was not found"),
        })?;
    let secrets = ProviderSecretStore::read_values(cx, &provider.secret_refs)
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
    Ok(ProviderModelFetchResult {
        provider_id: provider.id,
        models,
    })
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
        fields: default_fields_for_form_kind(spec.form_kind),
        existing_secret_refs: ProviderSecretRefs { refs: Vec::new() },
        dirty: false,
    }
}

fn default_fields_for_form_kind(
    form_kind: ProviderFormKind,
) -> BTreeMap<String, ProviderDraftValue> {
    match form_kind {
        ProviderFormKind::ApiKey => BTreeMap::new(),
        ProviderFormKind::Ollama => BTreeMap::from([(
            "base_url".to_string(),
            ProviderDraftValue::String("http://localhost:11434".to_string()),
        )]),
        ProviderFormKind::CustomOpenAiCompatible => BTreeMap::from([(
            "api_mode".to_string(),
            ProviderDraftValue::String("responses".to_string()),
        )]),
    }
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
        ModelFetchSupport, ProviderEditorKey, ProviderFetchPrecondition, ProviderFormComponents,
        ProviderListItem, ProviderSettingsPage, draft_from_record, draft_from_spec, fetch_support,
        provider_fetch_precondition,
    };
    use crate::database::{self, FreshStoreGlobal};
    use crate::features::settings::provider::catalog::{
        ModelListingStrategy, ProviderKindKey, builtin_provider_specs,
    };
    use crate::features::settings::provider::draft::{
        ProviderDraftSnapshot, ProviderDraftValue, ProviderModelDraft,
    };
    use crate::features::settings::provider::forms::{
        ProviderApiMode, ProviderFormField, ProviderSecretValue, ProviderSettingsForm,
        ProviderSettingsFormOutput,
    };
    use crate::features::settings::provider::list_delegates::{
        ProviderListDelegate, ProviderModelListDelegate, model_list_rows, provider_list_rows,
    };
    use crate::foundation::{
        I18n,
        assets::{IconName, ProviderLogoName},
    };
    use crate::state::provider_secrets::{ProviderSecretStore, ProviderSecretWrite};
    use fluent_bundle::FluentArgs;
    use gpui::{App, AppContext as _, Entity, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::list::ListEvent;
    use gpui_component::{IndexPath, Root};
    use jaco_core::{
        ProviderId, ProviderModelMetadata, ProviderSecretRef, ProviderSecretRefs,
        ProviderSettingFieldValue, ProviderSettingValue, ProviderSettingsPayload,
        conservative_model_capabilities,
    };
    use jaco_db::{FreshStore, NewProvider, NewProviderModel, UpdateProvider};
    use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
    use tempfile::{TempDir, tempdir};

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
    fn builtin_provider_specs_assign_brand_visuals_and_fallbacks() {
        let specs = builtin_provider_specs();
        let anthropic = specs
            .iter()
            .find(|spec| spec.kind.as_str() == "anthropic")
            .expect("anthropic provider spec exists");
        let openai = specs
            .iter()
            .find(|spec| spec.kind.as_str() == "openai")
            .expect("openai provider spec exists");
        let azure_openai = specs
            .iter()
            .find(|spec| spec.kind.as_str() == "azure_openai")
            .expect("azure openai provider spec exists");
        let branded_without_logo = specs
            .iter()
            .filter(|spec| spec.kind.as_str() != "custom_openai_compatible")
            .filter(|spec| spec.visual.logo.is_none())
            .map(|spec| spec.kind.as_str().to_string())
            .collect::<Vec<_>>();
        let custom = specs
            .iter()
            .find(|spec| spec.kind.as_str() == "custom_openai_compatible")
            .expect("custom provider spec exists");

        assert!(branded_without_logo.is_empty(), "{branded_without_logo:?}");
        assert_eq!(anthropic.visual.logo, Some(ProviderLogoName::Anthropic));
        assert_eq!(anthropic.visual.fallback, IconName::Cloud);
        assert_eq!(openai.visual.logo, Some(ProviderLogoName::OpenAI));
        assert_eq!(openai.visual.fallback, IconName::Cloud);
        assert_eq!(
            azure_openai.visual.logo,
            Some(ProviderLogoName::AzureOpenAI)
        );
        assert_eq!(azure_openai.visual.fallback, IconName::Cloud);
        assert_eq!(custom.visual.logo, None);
        assert_eq!(custom.visual.fallback, IconName::Server);
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
            assert_ne!(
                i18n.t_with_args("provider-validation-url-invalid", &args),
                "provider-validation-url-invalid"
            );
            assert_ne!(
                i18n.t_with_args("provider-validation-url-scheme", &args),
                "provider-validation-url-scheme"
            );
            let mut args = FluentArgs::new();
            args.set("count", 3);
            assert_ne!(
                i18n.t_with_args("provider-fetch-success-message", &args),
                "provider-fetch-success-message"
            );
        }
    }

    #[gpui::test]
    fn validation_rejects_missing_secret_before_repository_write(cx: &mut TestAppContext) {
        let _dir = init_empty_provider_page_test(cx);
        let (window, page) = open_provider_settings_root_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        assert!(cx.update(|_, cx| {
            database::repository(cx)
                .list_providers()
                .unwrap()
                .is_empty()
        }));
    }

    #[gpui::test]
    fn validation_accepts_saved_secret_consistently(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();

        let saved_valid = cx.update(|window, cx| {
            page.update(cx, |page, cx| page.validate_current_output(window, cx))
        });
        assert!(saved_valid);
    }

    #[gpui::test]
    fn validation_accepts_pending_secret_consistently(cx: &mut TestAppContext) {
        let _dir = init_empty_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");

        set_secret_input_value(&page, &openai, "api_key", "sk-pending", &mut cx);
        let pending_valid = cx.update(|window, cx| {
            page.update(cx, |page, cx| page.validate_current_output(window, cx))
        });
        assert!(pending_valid);
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

    #[gpui::test]
    fn secret_dirty_tracking_uses_secret_changed_binding(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");

        let initial_dirty_keys = page.read_with(&cx, |page, cx| {
            ProviderSettingsPage::snapshot_for_editor(
                page.editors.get(&openai).expect("openai editor exists"),
                cx,
            )
            .dirty_secret_keys
        });
        assert!(initial_dirty_keys.is_empty());

        set_secret_input_value(&page, &openai, "api_key", "temporary-token", &mut cx);
        set_secret_input_value(&page, &openai, "api_key", "", &mut cx);

        let changed_dirty_keys = page.read_with(&cx, |page, cx| {
            ProviderSettingsPage::snapshot_for_editor(
                page.editors.get(&openai).expect("openai editor exists"),
                cx,
            )
            .dirty_secret_keys
        });
        assert_eq!(changed_dirty_keys, BTreeSet::from(["api_key".to_string()]));
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
    fn custom_provider_output_display_name_uses_name_field() {
        let output = ProviderSettingsFormOutput::CustomOpenAi {
            enabled: true,
            name: "  Acme Gateway  ".to_string(),
            api_key: ProviderSecretValue::new(ProviderFormField::ApiKey, String::new(), false),
            base_url: String::new(),
            api_mode: ProviderApiMode::Responses,
        };

        assert_eq!(output.display_name("Custom"), "Acme Gateway");
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
    fn provider_list_rows_preserve_provider_visuals() {
        let providers = builtin_provider_specs()
            .into_iter()
            .map(|spec| ProviderListItem {
                spec,
                provider: None,
            })
            .collect::<Vec<_>>();
        let rows = provider_list_rows(&providers, &I18n::english_for_test());
        let anthropic = rows
            .iter()
            .find(|row| row.kind.as_str() == "anthropic")
            .expect("anthropic provider row exists");
        let openai = rows
            .iter()
            .find(|row| row.kind.as_str() == "openai")
            .expect("openai provider row exists");

        assert_eq!(anthropic.visual.logo, Some(ProviderLogoName::Anthropic));
        assert_eq!(openai.visual.logo, Some(ProviderLogoName::OpenAI));
    }

    #[test]
    fn provider_list_rows_use_saved_custom_display_name_for_display_and_search() {
        let custom = builtin_provider_specs()
            .into_iter()
            .find(|spec| spec.kind.as_str() == "custom_openai_compatible")
            .expect("custom provider spec exists");
        let dir = tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        let provider = store
            .repository()
            .insert_provider(NewProvider {
                kind: "custom_openai_compatible".to_string(),
                display_name: "Acme Gateway".to_string(),
                enabled: true,
                settings: ProviderSettingsPayload {
                    provider_kind: "custom_openai_compatible".to_string(),
                    fields: vec![ProviderSettingFieldValue {
                        key: "name".to_string(),
                        value: ProviderSettingValue::String {
                            value: "Acme Gateway".to_string(),
                        },
                    }],
                },
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            })
            .unwrap();
        let providers = vec![ProviderListItem {
            spec: custom,
            provider: Some(provider),
        }];
        let i18n = I18n::english_for_test();
        let rows = provider_list_rows(&providers, &i18n);

        assert_eq!(rows[0].display_name.as_ref(), "Acme Gateway");

        let mut delegate = provider_list_delegate(&providers, &i18n);
        delegate.set_query_for_test("acme");
        assert_eq!(delegate.row_count_for_test(), 1);

        delegate.set_query_for_test("custom openai");
        assert_eq!(delegate.row_count_for_test(), 1);
    }

    #[gpui::test]
    fn provider_editor_text_drafts_survive_selection_changes(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");
        let ollama = provider_editor_key("ollama");

        set_text_input_value(
            &page,
            &openai,
            ProviderFormField::BaseUrl,
            "https://draft.openai.example/v1",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("ollama"), window, cx);
                page.select_provider_from_list(ProviderKindKey::from("openai"), window, cx);
            });
        });

        let (value, openai_dirty, ollama_dirty) = page.read_with(&cx, |page, cx| {
            (
                text_input_value(page, &openai, ProviderFormField::BaseUrl, cx),
                page.is_editor_dirty(&openai, cx),
                page.is_editor_dirty(&ollama, cx),
            )
        });
        assert_eq!(value, "https://draft.openai.example/v1");
        assert!(openai_dirty);
        assert!(!ollama_dirty);
    }

    #[gpui::test]
    fn provider_editor_secret_drafts_stay_with_their_provider(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");
        let ollama = provider_editor_key("ollama");

        set_secret_input_value(&page, &openai, "api_key", "sk-draft-openai", &mut cx);
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("ollama"), window, cx);
                page.select_provider_from_list(ProviderKindKey::from("openai"), window, cx);
            });
        });

        let (secret_value, openai_dirty, ollama_dirty, openai_dirty_secret) =
            page.read_with(&cx, |page, cx| {
                let snapshot = ProviderSettingsPage::snapshot_for_editor(
                    page.editors.get(&openai).expect("openai editor exists"),
                    cx,
                );
                (
                    secret_input_value(page, &openai, "api_key", cx),
                    page.is_editor_dirty(&openai, cx),
                    page.is_editor_dirty(&ollama, cx),
                    snapshot.dirty_secret_keys.contains("api_key"),
                )
            });
        assert_eq!(secret_value, "sk-draft-openai");
        assert!(openai_dirty);
        assert!(!ollama_dirty);
        assert!(openai_dirty_secret);
    }

    #[gpui::test]
    fn provider_list_select_event_preserves_previous_provider_draft(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");

        set_text_input_value(
            &page,
            &openai,
            ProviderFormField::BaseUrl,
            "https://select-preserved.example/v1",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                let provider_list = page.provider_list.clone();
                let ix = provider_list
                    .read(cx)
                    .delegate()
                    .selected_index_for_kind(&ProviderKindKey::from("ollama"))
                    .expect("ollama row is visible");
                page.on_provider_list_event(&provider_list, &ListEvent::Select(ix), window, cx);
                page.select_provider_from_list(ProviderKindKey::from("openai"), window, cx);
            });
        });

        let value = page.read_with(&cx, |page, cx| {
            text_input_value(page, &openai, ProviderFormField::BaseUrl, cx)
        });
        assert_eq!(value, "https://select-preserved.example/v1");
    }

    #[gpui::test]
    fn provider_save_only_commits_selected_provider(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let (window, page) = open_provider_settings_root_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let openai = provider_editor_key("openai");
        let ollama = provider_editor_key("ollama");

        set_text_input_value(
            &page,
            &openai,
            ProviderFormField::BaseUrl,
            "https://unsaved.openai.example/v1",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("ollama"), window, cx);
            });
        });
        set_text_input_value(
            &page,
            &ollama,
            ProviderFormField::BaseUrl,
            "http://ollama-saved.example",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        let (openai_db_url, ollama_db_url) = cx.update(|_, cx| {
            let providers = database::repository(cx).list_providers().unwrap();
            (
                provider_setting_value(&providers, "openai", "base_url"),
                provider_setting_value(&providers, "ollama", "base_url"),
            )
        });
        let (openai_draft, openai_dirty, ollama_dirty) = page.read_with(&cx, |page, cx| {
            (
                text_input_value(page, &openai, ProviderFormField::BaseUrl, cx),
                page.is_editor_dirty(&openai, cx),
                page.is_editor_dirty(&ollama, cx),
            )
        });

        assert_eq!(openai_db_url, "https://api.openai.com/v1");
        assert_eq!(ollama_db_url, "http://ollama-saved.example");
        assert_eq!(openai_draft, "https://unsaved.openai.example/v1");
        assert!(openai_dirty);
        assert!(!ollama_dirty);
    }

    #[gpui::test]
    fn provider_save_rejects_invalid_base_url_before_repository_write(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let (window, page) = open_provider_settings_root_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let openai = provider_editor_key("openai");
        let ollama = provider_editor_key("ollama");

        set_text_input_value(
            &page,
            &openai,
            ProviderFormField::BaseUrl,
            "file:///tmp/openai",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("ollama"), window, cx);
            });
        });
        set_text_input_value(
            &page,
            &ollama,
            ProviderFormField::BaseUrl,
            "not a url",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        let (openai_db_url, ollama_db_url) = cx.update(|_, cx| {
            let providers = database::repository(cx).list_providers().unwrap();
            (
                provider_setting_value(&providers, "openai", "base_url"),
                provider_setting_value(&providers, "ollama", "base_url"),
            )
        });

        assert_eq!(openai_db_url, "https://api.openai.com/v1");
        assert_eq!(ollama_db_url, "http://localhost:11434");
        assert!(page.read_with(&cx, |page, cx| page.is_editor_dirty(&openai, cx)));
        assert!(page.read_with(&cx, |page, cx| page.is_editor_dirty(&ollama, cx)));
    }

    #[gpui::test]
    fn provider_save_running_locks_model_toggle_events(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let provider_id = cx.update(|cx| {
            let provider_id = provider_id_for_kind(cx, "openai");
            database::repository(cx)
                .replace_fetched_provider_models(
                    &provider_id,
                    vec![provider_model_for_test(&provider_id, "gpt-5")],
                )
                .unwrap();
            provider_id
        });
        let window = open_provider_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).unwrap();
        let openai = provider_editor_key("openai");

        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("openai"), window, cx);
                let form = &page
                    .editors
                    .get(&openai)
                    .expect("openai editor exists")
                    .form;
                let secret_refs = page
                    .editors
                    .get(&openai)
                    .expect("openai editor exists")
                    .draft
                    .existing_secret_refs
                    .clone();
                let _ = form.submit_async_save(
                    secret_refs,
                    |output, _window, cx| {
                        let _enabled = output.enabled();
                        cx.spawn(async move |_cx| {
                            std::future::pending::<Result<(), String>>().await
                        })
                    },
                    window,
                    cx,
                );
                page.sync_model_list(window, cx);
            });
        });

        let model_list = page.read_with(&cx, |page, _| page.model_list.clone());
        assert!(model_list.read_with(&cx, |list, _| list.delegate().disabled_for_test()));

        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.on_model_list_event(
                    &model_list,
                    &ListEvent::Confirm(IndexPath::new(0)),
                    window,
                    cx,
                );
            });
        });

        let model_still_enabled = cx.update(|_, cx| {
            database::repository(cx)
                .list_provider_models(&provider_id)
                .unwrap()
                .into_iter()
                .find(|model| model.model_id == "gpt-5")
                .expect("model exists")
                .enabled
        });
        assert!(model_still_enabled);
    }

    #[gpui::test]
    fn provider_save_removes_cleared_optional_secret_ref(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        set_provider_secret_refs_for_test(
            cx,
            "ollama",
            ProviderSecretRefs {
                refs: vec![ProviderSecretRef {
                    key: "bearer_token".to_string(),
                    storage: "keychain".to_string(),
                    ref_id: "ollama-provider:bearer_token".to_string(),
                }],
            },
        );
        let (window, page) = open_provider_settings_root_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let ollama = provider_editor_key("ollama");

        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.select_provider_from_list(ProviderKindKey::from("ollama"), window, cx);
            });
        });
        set_text_input_value(
            &page,
            &ollama,
            ProviderFormField::BaseUrl,
            "http://ollama-preserved.example",
            &mut cx,
        );
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        assert_eq!(
            cx.update(|_, cx| provider_secret_ref_keys(cx, "ollama")),
            vec!["bearer_token".to_string()]
        );

        set_secret_input_value(&page, &ollama, "bearer_token", "temporary-token", &mut cx);
        set_secret_input_value(&page, &ollama, "bearer_token", "", &mut cx);
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        assert_eq!(
            cx.update(|_, cx| provider_secret_ref_keys(cx, "ollama")),
            Vec::<String>::new()
        );
        assert!(!page.read_with(&cx, |page, cx| page.is_editor_dirty(&ollama, cx)));
    }

    #[gpui::test]
    fn provider_save_rejects_cleared_required_secret(cx: &mut TestAppContext) {
        let _dir = init_provider_page_test(cx);
        let (window, page) = open_provider_settings_root_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let openai = provider_editor_key("openai");

        set_secret_input_value(&page, &openai, "api_key", "temporary-token", &mut cx);
        set_secret_input_value(&page, &openai, "api_key", "", &mut cx);
        cx.update(|window, cx| {
            page.update(cx, |page, cx| {
                page.save(window, cx);
            });
        });
        cx.run_until_parked();

        assert_eq!(
            cx.update(|_, cx| provider_secret_ref_keys(cx, "openai")),
            vec!["api_key".to_string()]
        );
        assert!(page.read_with(&cx, |page, cx| page.is_editor_dirty(&openai, cx)));
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

    fn init_empty_provider_page_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            crate::state::providers::init(cx);
            crate::foundation::i18n::init(cx);
        });
        dir
    }

    fn init_provider_page_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            crate::state::providers::init(cx);
            crate::foundation::i18n::init(cx);

            let repository = database::repository(cx);
            repository
                .insert_provider(provider_for_test(
                    "openai",
                    "OpenAI",
                    "https://api.openai.com/v1",
                    ProviderSecretRefs {
                        refs: vec![ProviderSecretRef {
                            key: "api_key".to_string(),
                            storage: "keychain".to_string(),
                            ref_id: "openai-provider:api_key".to_string(),
                        }],
                    },
                ))
                .unwrap();
            repository
                .insert_provider(provider_for_test(
                    "ollama",
                    "Ollama",
                    "http://localhost:11434",
                    ProviderSecretRefs { refs: Vec::new() },
                ))
                .unwrap();
        });
        dir
    }

    fn open_provider_settings_window(
        cx: &mut TestAppContext,
    ) -> WindowHandle<ProviderSettingsPage> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| ProviderSettingsPage::new(window, cx))
            })
        })
        .unwrap()
    }

    fn open_provider_settings_root_window(
        cx: &mut TestAppContext,
    ) -> (WindowHandle<Root>, Entity<ProviderSettingsPage>) {
        let page = Rc::new(RefCell::new(None));
        let page_for_window = page.clone();
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), move |window, cx| {
                    let settings_page = cx.new(|cx| ProviderSettingsPage::new(window, cx));
                    page_for_window.replace(Some(settings_page.clone()));
                    cx.new(|cx| Root::new(settings_page, window, cx))
                })
            })
            .unwrap();
        let page = page
            .borrow()
            .clone()
            .expect("provider settings page is created");
        (window, page)
    }

    fn provider_editor_key(kind: &str) -> ProviderEditorKey {
        ProviderEditorKey::new(ProviderKindKey::from(kind))
    }

    fn set_provider_secret_refs_for_test(
        cx: &mut TestAppContext,
        kind: &str,
        secret_refs: ProviderSecretRefs,
    ) {
        cx.update(|cx| {
            let repository = database::repository(cx);
            let provider = repository
                .list_providers()
                .unwrap()
                .into_iter()
                .find(|provider| provider.kind == kind)
                .expect("provider exists");
            repository
                .update_provider(
                    &provider.id,
                    UpdateProvider {
                        display_name: provider.display_name,
                        enabled: provider.enabled,
                        settings: provider.settings,
                        secret_refs,
                    },
                )
                .unwrap();
        });
    }

    fn provider_secret_ref_keys(cx: &App, kind: &str) -> Vec<String> {
        database::repository(cx)
            .list_providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.kind == kind)
            .expect("provider exists")
            .secret_refs
            .refs
            .into_iter()
            .map(|secret| secret.key)
            .collect()
    }

    fn provider_for_test(
        kind: &str,
        display_name: &str,
        base_url: &str,
        secret_refs: ProviderSecretRefs,
    ) -> NewProvider {
        NewProvider {
            kind: kind.to_string(),
            display_name: display_name.to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: kind.to_string(),
                fields: vec![ProviderSettingFieldValue {
                    key: "base_url".to_string(),
                    value: ProviderSettingValue::String {
                        value: base_url.to_string(),
                    },
                }],
            },
            secret_refs,
        }
    }

    fn set_text_input_value(
        page: &Entity<ProviderSettingsPage>,
        key: &ProviderEditorKey,
        field: ProviderFormField,
        value: &str,
        cx: &mut VisualTestContext,
    ) {
        let input = page.read_with(cx, |page, _cx| {
            let editor = page.editors.get(key).expect("editor exists");
            input_state_for_test(&editor.components, field).expect("text input exists")
        });
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx);
                cx.emit(gpui_component::input::InputEvent::Change);
            });
        });
    }

    fn set_secret_input_value(
        page: &Entity<ProviderSettingsPage>,
        key: &ProviderEditorKey,
        field: &str,
        value: &str,
        cx: &mut VisualTestContext,
    ) {
        let input = page.read_with(cx, |page, _cx| {
            let editor = page.editors.get(key).expect("editor exists");
            input_state_for_test(&editor.components, secret_field_for_test(field))
                .expect("secret input exists")
        });
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx);
                cx.emit(gpui_component::input::InputEvent::Change);
            });
        });
    }

    fn text_input_value(
        page: &ProviderSettingsPage,
        key: &ProviderEditorKey,
        field: ProviderFormField,
        cx: &App,
    ) -> String {
        let editor = page.editors.get(key).expect("editor exists");
        field_value_for_test(&editor.form, field, cx).expect("field value exists")
    }

    fn secret_input_value(
        page: &ProviderSettingsPage,
        key: &ProviderEditorKey,
        field: &str,
        cx: &App,
    ) -> String {
        let editor = page.editors.get(key).expect("editor exists");
        input_state_for_test(&editor.components, secret_field_for_test(field))
            .expect("secret input exists")
            .read(cx)
            .value()
            .to_string()
    }

    fn secret_field_for_test(field: &str) -> ProviderFormField {
        match field {
            "api_key" => ProviderFormField::ApiKey,
            "bearer_token" => ProviderFormField::BearerToken,
            _ => panic!("unknown provider secret field `{field}`"),
        }
    }

    fn input_state_for_test(
        components: &ProviderFormComponents,
        field: ProviderFormField,
    ) -> Option<Entity<gpui_component::input::InputState>> {
        match (components, field) {
            (ProviderFormComponents::ApiKey { api_key, .. }, ProviderFormField::ApiKey) => {
                Some(api_key.clone())
            }
            (ProviderFormComponents::ApiKey { base_url, .. }, ProviderFormField::BaseUrl) => {
                Some(base_url.clone())
            }
            (ProviderFormComponents::Ollama { base_url, .. }, ProviderFormField::BaseUrl) => {
                Some(base_url.clone())
            }
            (
                ProviderFormComponents::Ollama { bearer_token, .. },
                ProviderFormField::BearerToken,
            ) => Some(bearer_token.clone()),
            (ProviderFormComponents::CustomOpenAi { name, .. }, ProviderFormField::Name) => {
                Some(name.clone())
            }
            (ProviderFormComponents::CustomOpenAi { api_key, .. }, ProviderFormField::ApiKey) => {
                Some(api_key.clone())
            }
            (ProviderFormComponents::CustomOpenAi { base_url, .. }, ProviderFormField::BaseUrl) => {
                Some(base_url.clone())
            }
            _ => None,
        }
    }

    fn field_value_for_test(
        form: &ProviderSettingsForm,
        field: ProviderFormField,
        cx: &App,
    ) -> Option<String> {
        if matches!(
            field,
            ProviderFormField::ApiKey | ProviderFormField::BearerToken
        ) {
            return form
                .current_output(cx)
                .secret_fields()
                .into_iter()
                .find(|secret| secret.key() == field.key())
                .map(|secret| secret.value);
        }

        form.current_output(cx)
            .persistent_fields()
            .remove(field.key())
            .and_then(|value| match value {
                ProviderDraftValue::String(value) => Some(value),
                _ => None,
            })
    }

    fn provider_setting_value(
        providers: &[jaco_db::ProviderRecord],
        kind: &str,
        field_key: &str,
    ) -> String {
        providers
            .iter()
            .find(|provider| provider.kind == kind)
            .and_then(|provider| {
                provider
                    .settings
                    .fields
                    .iter()
                    .find(|field| field.key == field_key)
            })
            .and_then(|field| match &field.value {
                ProviderSettingValue::String { value } => Some(value.clone()),
                _ => None,
            })
            .expect("provider setting exists")
    }

    fn provider_record_with_base_url(enabled: bool, base_url: &str) -> jaco_db::ProviderRecord {
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

    fn provider_id_for_kind(cx: &App, kind: &str) -> ProviderId {
        database::repository(cx)
            .list_providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.kind == kind)
            .expect("provider exists")
            .id
    }

    fn provider_model_for_test(provider_id: &ProviderId, model_id: &str) -> NewProviderModel {
        NewProviderModel {
            provider_id: provider_id.clone(),
            model_id: model_id.to_string(),
            display_name: None,
            enabled: true,
            capabilities: conservative_model_capabilities("openai"),
            metadata: ProviderModelMetadata {
                display_name: None,
                family: None,
                raw: None,
            },
        }
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
