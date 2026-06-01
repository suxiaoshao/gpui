use std::collections::BTreeMap;

use crate::{
    database,
    foundation::{I18n, assets::IconName},
};
use ai_chat_core::{ProviderId, ProviderSecretRefs, ProviderSettingValue};
use ai_chat_db::{NewProvider, ProviderModelRecord, ProviderRecord, UpdateProvider};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
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
mod model_fetch;
mod secret_store;

use self::{
    catalog::{ProviderFieldKind, ProviderKindKey, ProviderSpec, builtin_provider_specs},
    draft::{
        AsyncActionState, ManualModelEditor, ProviderDraft, ProviderDraftValue, ProviderModelDraft,
        ProviderSecretInput, ProviderSelection, ProviderValidationState,
    },
    model_fetch::{ModelFetchSupport, fetch_support},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderListItem {
    spec: ProviderSpec,
    provider: Option<ProviderRecord>,
}

pub(super) struct ProviderSettingsPage {
    provider_search: Entity<InputState>,
    model_search: Entity<InputState>,
    selected: ProviderSelection,
    providers: Vec<ProviderListItem>,
    models: Vec<ProviderModelDraft>,
    draft: ProviderDraft,
    text_inputs: BTreeMap<String, Entity<InputState>>,
    secret_inputs: BTreeMap<String, Entity<ProviderSecretInput>>,
    validation: ProviderValidationState,
    save_state: AsyncActionState,
    fetch_state: AsyncActionState,
    #[allow(dead_code)]
    manual_model_editor: Option<Entity<ManualModelEditor>>,
    _subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
    _save_task: Option<Task<()>>,
    _fetch_task: Option<Task<()>>,
}

impl ProviderSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let provider_search_placeholder = cx.global::<I18n>().t("provider-search-placeholder");
        let model_search_placeholder = cx.global::<I18n>().t("provider-model-search-placeholder");
        let provider_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(provider_search_placeholder));
        let model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(model_search_placeholder));
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
        let mut this = Self {
            provider_search,
            model_search,
            selected,
            providers,
            models,
            draft,
            text_inputs: BTreeMap::new(),
            secret_inputs: BTreeMap::new(),
            validation: ProviderValidationState::Idle,
            save_state: AsyncActionState::Idle,
            fetch_state: AsyncActionState::Idle,
            manual_model_editor: None,
            _subscriptions: Vec::new(),
            _load_task: None,
            _save_task: None,
            _fetch_task: None,
        };
        this.rebuild_inputs(window, cx);
        this._subscriptions = vec![
            cx.subscribe_in(&this.provider_search, window, Self::on_search_input),
            cx.subscribe_in(&this.model_search, window, Self::on_search_input),
        ];
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
                    let key = field.key.to_string();
                    let secret = cx.new(|cx| {
                        ProviderSecretInput::new(key.clone(), saved_ref_id, input, window, cx)
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
                    self.text_inputs.insert(field.key.to_string(), input);
                }
            }
        }
    }

    fn on_search_input(
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

    fn selected_spec(&self) -> Option<&ProviderSpec> {
        let kind = &self.draft.kind;
        self.providers
            .iter()
            .find(|item| &item.spec.kind == kind)
            .map(|item| &item.spec)
    }

    fn select_provider(
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
        cx.notify();
    }

    fn collect_input_values(&mut self, cx: &mut Context<Self>) {
        for (key, input) in &self.text_inputs {
            let value = input.read(cx).value().to_string();
            self.draft
                .fields
                .insert(key.clone(), ProviderDraftValue::String(value));
        }
    }

    fn validate_draft(&mut self, cx: &mut Context<Self>) -> bool {
        self.collect_input_values(cx);
        let Some(spec) = self.selected_spec() else {
            self.validation = ProviderValidationState::Invalid(
                cx.global::<I18n>()
                    .t("provider-validation-not-registered")
                    .into(),
            );
            return false;
        };
        for field in &spec.fields {
            if !field.required {
                continue;
            }
            match field.kind {
                ProviderFieldKind::Secret => {
                    let valid = self.secret_inputs.get(field.key).is_some_and(|secret| {
                        let secret = secret.read(cx);
                        secret.has_saved_secret || !secret.input.read(cx).value().is_empty()
                    });
                    if !valid {
                        self.validation = ProviderValidationState::Invalid(required_field_message(
                            field.label_key,
                            cx,
                        ));
                        return false;
                    }
                }
                _ if self.draft.field_string(field.key).trim().is_empty() => {
                    self.validation = ProviderValidationState::Invalid(required_field_message(
                        field.label_key,
                        cx,
                    ));
                    return false;
                }
                _ => {}
            }
        }
        self.validation = ProviderValidationState::Valid;
        true
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.validate_draft(cx) {
            cx.notify();
            return;
        }
        self.save_state = AsyncActionState::Running;
        self.collect_input_values(cx);
        let repository = database::repository(cx);
        let result = if let Some(provider_id) = self.draft.provider_id.clone() {
            let secret_refs = self.secret_refs_for_provider(&provider_id, cx);
            repository.update_provider(
                &provider_id,
                UpdateProvider {
                    display_name: self.draft.display_name.clone(),
                    enabled: self.draft.enabled,
                    settings: self.draft.settings_payload(),
                    secret_refs,
                },
            )
        } else {
            repository.insert_provider(NewProvider {
                kind: self.draft.kind.as_str().to_string(),
                display_name: self.draft.display_name.clone(),
                enabled: self.draft.enabled,
                settings: self.draft.settings_payload(),
                secret_refs: self.secret_refs_for_provider(self.draft.kind.as_str(), cx),
            })
        };
        self.save_state = AsyncActionState::Idle;
        match result {
            Ok(provider) => {
                self.draft = draft_from_record(&provider);
                self.providers = Self::load_provider_list(cx).unwrap_or_default();
                self.models = Self::load_models_for_draft(&self.draft, cx).unwrap_or_default();
                self.validation = ProviderValidationState::Valid;
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
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn secret_refs_for_provider(&self, provider_id: &str, cx: &App) -> ProviderSecretRefs {
        let writes = self
            .secret_inputs
            .iter()
            .filter_map(|(key, secret)| {
                let secret = secret.read(cx);
                let value = secret.input.read(cx).value().to_string();
                (!value.is_empty()).then(|| secret_store::ProviderSecretWrite {
                    key: key.clone(),
                    value,
                })
            })
            .collect::<Vec<_>>();
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
        let support = fetch_support(&self.draft.kind);
        self.fetch_state = AsyncActionState::Running;
        self.fetch_state = AsyncActionState::Idle;
        let i18n = cx.global::<I18n>();
        let message = match support {
            ModelFetchSupport::Supported => i18n.t("provider-fetch-supported-placeholder"),
            ModelFetchSupport::ManualOnly => i18n.t("provider-fetch-manual-only"),
        };
        window.push_notification(
            Notification::new()
                .title(i18n.t("provider-notification-fetch-unavailable-title"))
                .message(message)
                .with_type(NotificationType::Warning),
            cx,
        );
    }

    fn render_provider_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.provider_search.read(cx).value().trim().to_lowercase();
        v_flex()
            .w(px(260.))
            .min_w(px(220.))
            .h_full()
            .gap_3()
            .child(
                Input::new(&self.provider_search)
                    .w_full()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .gap_2()
                    .children(self.providers.iter().filter_map(|item| {
                        let haystack = format!(
                            "{} {} {}",
                            item.spec.display_name,
                            item.spec.kind.as_str(),
                            cx.global::<I18n>().t(item.spec.description_key)
                        )
                        .to_lowercase();
                        (query.is_empty() || haystack.contains(&query))
                            .then(|| self.render_provider_row(item, cx))
                    })),
            )
            .into_any_element()
    }

    fn render_provider_row(&self, item: &ProviderListItem, cx: &mut Context<Self>) -> AnyElement {
        let active = item.spec.kind == self.draft.kind;
        let kind = item.spec.kind.clone();
        h_flex()
            .id(format!(
                "provider-settings-provider-{}",
                item.spec.kind.as_str()
            ))
            .w_full()
            .h(px(42.))
            .items_center()
            .gap_2()
            .px_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if active {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(if active {
                cx.theme().accent
            } else {
                cx.theme().background
            })
            .cursor_pointer()
            .on_click(cx.listener(move |page, _, window, cx| {
                page.select_provider(kind.clone(), window, cx);
            }))
            .child(Icon::new(item.spec.icon).text_color(cx.theme().muted_foreground))
            .child(
                Label::new(item.spec.display_name)
                    .text_sm()
                    .font_medium()
                    .truncate(),
            )
            .child(div().flex_1())
            .when(
                item.provider
                    .as_ref()
                    .is_some_and(|provider| provider.enabled),
                |this| this.child(div().size_2().rounded_full().bg(cx.theme().primary)),
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
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .gap_4()
                    .child(self.render_config(spec, cx))
                    .child(self.render_models(cx)),
            )
            .into_any_element()
    }

    fn render_header(&self, spec: &ProviderSpec, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
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
                            .when(self.draft.provider_id.is_some(), |this| {
                                this.child(
                                    Tag::success()
                                        .small()
                                        .child(cx.global::<I18n>().t("provider-status-saved")),
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
                        page.draft.dirty = true;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_config(&self, spec: &ProviderSpec, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
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
                                page.validate_draft(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("provider-settings-save")
                            .icon(IconName::Check)
                            .label(cx.global::<I18n>().t("provider-action-save"))
                            .small()
                            .primary()
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
        let query = self.model_search.read(cx).value().trim().to_lowercase();
        let rows = self
            .models
            .iter()
            .filter(|model| {
                query.is_empty()
                    || model.model_id.to_lowercase().contains(&query)
                    || model
                        .display_name
                        .as_ref()
                        .is_some_and(|name| name.to_lowercase().contains(&query))
            })
            .map(|model| self.render_model_row(model, cx))
            .collect::<Vec<_>>();
        v_flex()
            .w_full()
            .gap_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
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
                            .on_click(cx.listener(|page, _, window, cx| {
                                page.fetch_models(window, cx);
                            })),
                    ),
            )
            .child(
                Input::new(&self.model_search)
                    .w_full()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .children(rows)
                    .when(self.models.is_empty(), |this| {
                        this.child(
                            Label::new(cx.global::<I18n>().t("provider-empty-models"))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_model_row(&self, model: &ProviderModelDraft, cx: &mut Context<Self>) -> AnyElement {
        let model_id = model.model_id.clone();
        h_flex()
            .w_full()
            .gap_3()
            .items_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .px_3()
            .py_2()
            .child(
                Switch::new(format!("provider-model-enabled-{}", model.model_id))
                    .checked(model.enabled)
                    .small()
                    .on_click(cx.listener(move |page, checked, window, cx| {
                        page.toggle_model(model_id.clone(), *checked, window, cx);
                    })),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        Label::new(
                            model
                                .display_name
                                .clone()
                                .unwrap_or_else(|| model.model_id.clone()),
                        )
                        .text_sm()
                        .truncate(),
                    )
                    .child(
                        Label::new(model.model_id.clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .child(capability_tags(&model.capabilities))
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

fn required_field_message(field_label_key: &str, cx: &App) -> SharedString {
    let mut args = FluentArgs::new();
    args.set("field", cx.global::<I18n>().t(field_label_key));
    cx.global::<I18n>()
        .t_with_args("provider-validation-required", &args)
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

fn capability_tags(capabilities: &ai_chat_core::ModelCapabilitiesSnapshot) -> AnyElement {
    h_flex()
        .gap_1()
        .when(capabilities.reasoning.is_some(), |this| {
            this.child(Tag::secondary().small().child("reasoning"))
        })
        .when(capabilities.tool_calling.is_some(), |this| {
            this.child(Tag::secondary().small().child("tools"))
        })
        .when(capabilities.image_input.is_some(), |this| {
            this.child(Tag::secondary().small().child("vision"))
        })
        .when(capabilities.structured_output, |this| {
            this.child(Tag::secondary().small().child("structured"))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{draft_from_record, draft_from_spec};
    use crate::features::settings::provider::catalog::builtin_provider_specs;
    use crate::foundation::I18n;
    use ai_chat_core::{ProviderSecretRefs, ProviderSettingsPayload};
    use ai_chat_db::{FreshStore, NewProvider};
    use fluent_bundle::FluentArgs;
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
            "provider-notification-update-model-failed",
            "provider-notification-fetch-unavailable-title",
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
        }
    }
}
