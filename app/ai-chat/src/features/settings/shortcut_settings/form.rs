use crate::{
    components::hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    database::{
        ConversationTemplate, Db, GlobalShortcutBinding, Mode, NewGlobalShortcutBinding,
        ShortcutInputSource,
    },
    features::{
        hotkey::GlobalHotkeyState,
        settings::shortcut_settings::{
            SHORTCUT_DIALOG_MARGIN_TOP, SHORTCUT_DIALOG_MAX_HEIGHT, SHORTCUT_DIALOG_WIDTH,
            choices::{ExtSettingChoice, ModelChoice, TemplateChoice},
            segmented::single_selected_index,
            validation::{ShortcutValidationError, validate_hotkey},
        },
    },
    foundation::{assets::IconName, i18n::I18n},
    llm::{
        ExtSettingControl, ExtSettingItem, ProviderModel, apply_ext_setting,
        build_request_template, preset_ext_settings,
    },
    state::{AiChatConfig, ModelStore, ModelStoreSnapshot, ModelStoreStatus},
};
use gpui::{AppContext as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::DialogFooter,
    divider::Divider,
    form::{field, v_form},
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    select::{SearchableVec, Select, SelectDelegate, SelectEvent, SelectGroup, SelectState},
    switch::Switch,
    v_flex,
};
use std::{ops::Deref, rc::Rc};

type OnSaved = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

enum FormExtSettingState {
    Select {
        item: ExtSettingItem,
        state: Entity<SelectState<Vec<ExtSettingChoice>>>,
    },
    Boolean {
        item: ExtSettingItem,
    },
}

#[derive(Clone, Copy, Debug)]
enum ShortcutDialogMode {
    Add,
    Edit,
}

impl ShortcutDialogMode {
    fn title_key(self) -> &'static str {
        match self {
            Self::Add => "dialog-add-shortcut-title",
            Self::Edit => "dialog-edit-shortcut-title",
        }
    }

    fn submit_label_key(self) -> &'static str {
        match self {
            Self::Add => "button-create",
            Self::Edit => "button-save-shortcut",
        }
    }

    fn submit_icon(self) -> IconName {
        match self {
            Self::Add => IconName::Upload,
            Self::Edit => IconName::Save,
        }
    }
}

struct ShortcutFormState {
    mode: ShortcutDialogMode,
    binding_id: Option<i32>,
    initial_binding: Option<GlobalShortcutBinding>,
    existing_bindings: Vec<GlobalShortcutBinding>,
    templates: Vec<ConversationTemplate>,
    template_select: Entity<SelectState<Vec<TemplateChoice>>>,
    model_select: Entity<SelectState<SearchableVec<SelectGroup<ModelChoice>>>>,
    hotkey_input: Entity<HotkeyInput>,
    model_store_models: Vec<ProviderModel>,
    model_store_status: Option<ModelStoreStatus>,
    provider_name: String,
    chat_mode: Mode,
    hotkey: Option<String>,
    hotkey_error: Option<ShortcutValidationError>,
    enabled: bool,
    input_source: ShortcutInputSource,
    request_template: serde_json::Value,
    ext_settings: Vec<FormExtSettingState>,
    model_resolved: bool,
    save_error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

pub(super) fn open_add_shortcut_dialog(
    templates: Vec<ConversationTemplate>,
    existing_bindings: Vec<GlobalShortcutBinding>,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    open_shortcut_form_dialog(
        ShortcutDialogMode::Add,
        None,
        templates,
        existing_bindings,
        on_saved,
        window,
        cx,
    );
}

pub(super) fn open_edit_shortcut_dialog(
    binding: GlobalShortcutBinding,
    templates: Vec<ConversationTemplate>,
    existing_bindings: Vec<GlobalShortcutBinding>,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    open_shortcut_form_dialog(
        ShortcutDialogMode::Edit,
        Some(binding),
        templates,
        existing_bindings,
        on_saved,
        window,
        cx,
    );
}

fn open_shortcut_form_dialog(
    mode: ShortcutDialogMode,
    binding: Option<GlobalShortcutBinding>,
    templates: Vec<ConversationTemplate>,
    existing_bindings: Vec<GlobalShortcutBinding>,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let title = cx.global::<I18n>().t(mode.title_key());
    let cancel_label = cx.global::<I18n>().t("button-cancel");
    let reset_label = cx.global::<I18n>().t("button-reset");
    let submit_label = cx.global::<I18n>().t(mode.submit_label_key());
    let submit_icon = mode.submit_icon();
    let form = cx
        .new(|cx| ShortcutFormState::new(mode, binding, templates, existing_bindings, window, cx));

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let on_saved = on_saved.clone();
        dialog
            .w(px(SHORTCUT_DIALOG_WIDTH))
            .h(px(SHORTCUT_DIALOG_MAX_HEIGHT))
            .max_h(px(SHORTCUT_DIALOG_MAX_HEIGHT))
            .margin_top(px(SHORTCUT_DIALOG_MARGIN_TOP))
            .title(title.clone())
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("shortcut-form-cancel")
                            .label(cancel_label.clone())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .when(matches!(mode, ShortcutDialogMode::Edit), |this| {
                        this.child(
                            Button::new("shortcut-form-reset")
                                .icon(IconName::RefreshCcw)
                                .label(reset_label.clone())
                                .on_click({
                                    let form = form.clone();
                                    move |_, window, cx| {
                                        form.update(cx, |form, cx| form.reset(window, cx));
                                    }
                                }),
                        )
                    })
                    .child(
                        Button::new("shortcut-form-submit")
                            .primary()
                            .icon(submit_icon)
                            .label(submit_label.clone())
                            .on_click({
                                let form = form.clone();
                                let on_saved = on_saved.clone();
                                move |_, window, cx| {
                                    let saved = form.update(cx, |form, cx| form.save(window, cx));
                                    if saved {
                                        on_saved(window, cx);
                                        window.close_dialog(cx);
                                    }
                                }
                            }),
                    ),
            )
    });
}

impl ShortcutFormState {
    fn new(
        mode: ShortcutDialogMode,
        binding: Option<GlobalShortcutBinding>,
        templates: Vec<ConversationTemplate>,
        existing_bindings: Vec<GlobalShortcutBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let template_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
        let model_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<SelectGroup<ModelChoice>>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        let hotkey_input = cx.new(|cx| HotkeyInput::new("shortcut-form-hotkey", window, cx));
        let mut this = Self {
            mode,
            binding_id: binding.as_ref().map(|binding| binding.id),
            initial_binding: binding.clone(),
            existing_bindings,
            templates,
            template_select,
            model_select,
            hotkey_input,
            model_store_models: Vec::new(),
            model_store_status: None,
            provider_name: String::new(),
            chat_mode: Mode::Contextual,
            hotkey: None,
            hotkey_error: None,
            enabled: true,
            input_source: ShortcutInputSource::SelectionOrClipboard,
            request_template: serde_json::json!({}),
            ext_settings: Vec::new(),
            model_resolved: false,
            save_error: None,
            _subscriptions: Vec::new(),
        };
        this.rebuild_controls(binding.as_ref(), window, cx);
        this.refresh_model_choices_from_store(window, cx);
        this
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let binding = self.initial_binding.clone();
        self.save_error = None;
        self.hotkey_error = None;
        self.rebuild_controls(binding.as_ref(), window, cx);
        cx.notify();
    }

    fn rebuild_controls(
        &mut self,
        binding: Option<&GlobalShortcutBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._subscriptions.clear();
        let ModelStoreSnapshot {
            models: available_models,
            status,
            ..
        } = Self::model_store_snapshot(cx);
        self.model_store_models = available_models.clone();
        self.model_store_status = status;
        let template_id = binding.and_then(|binding| binding.template_id);
        let provider_name = binding
            .map(|binding| binding.provider_name.clone())
            .or_else(|| {
                available_models
                    .first()
                    .map(|model| model.provider_name.clone())
            })
            .unwrap_or_default();
        let model_id = binding
            .map(|binding| binding.model_id.clone())
            .or_else(|| available_models.first().map(|model| model.id.clone()))
            .unwrap_or_default();
        let mode = binding
            .map(|binding| binding.mode)
            .unwrap_or(Mode::Contextual);
        let input_source = binding
            .map(|binding| binding.input_source)
            .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
        let hotkey = binding.map(|binding| binding.hotkey.clone());
        let parsed_hotkey = hotkey.as_deref().and_then(string_to_keystroke);

        self.binding_id = binding.map(|binding| binding.id);
        self.provider_name = provider_name;
        self.chat_mode = mode;
        self.hotkey = hotkey.filter(|hotkey| string_to_keystroke(hotkey).is_some());
        self.enabled = binding.map(|binding| binding.enabled).unwrap_or(true);
        self.input_source = input_source;
        self.request_template = binding
            .map(|binding| binding.request_template.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        self.template_select = cx.new(|cx| {
            SelectState::new(
                self.template_options(cx),
                Some(self.template_selected_index(template_id, cx)),
                window,
                cx,
            )
            .searchable(true)
        });
        let model_choices = Self::model_choices_from(
            &available_models,
            Some((&self.provider_name, &model_id)),
            cx,
        );
        let model_selected = Self::model_selected_index(
            &model_choices,
            &ModelChoice::key(&self.provider_name, &model_id),
        );
        self.model_select = cx
            .new(|cx| SelectState::new(model_choices, model_selected, window, cx).searchable(true));
        self.hotkey_input = cx.new(|cx| {
            HotkeyInput::new("shortcut-form-hotkey", window, cx)
                .small()
                .w_full()
                .default_value(parsed_hotkey)
        });
        Self::refresh_request_template_with_models(
            self,
            &available_models,
            binding.map(|binding| &binding.request_template),
            false,
            window,
            cx,
        );
        self.bind_subscriptions(window, cx);
    }

    fn bind_subscriptions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_store = cx.global::<ModelStore>().deref().clone();
        self._subscriptions
            .push(cx.observe_in(&model_store, window, |this, _, window, cx| {
                this.refresh_model_choices_from_store(window, cx);
            }));

        self._subscriptions.push(cx.subscribe_in(
            &self.hotkey_input,
            window,
            |this, _input, event: &HotkeyEvent, _window, cx| {
                match event {
                    HotkeyEvent::Confirm(value) => {
                        this.hotkey = Some(value.to_string());
                        this.hotkey_error = None;
                        this.save_error = None;
                    }
                    HotkeyEvent::Cancel => {
                        this.hotkey = None;
                        this.hotkey_error = None;
                        this.save_error = None;
                    }
                }
                cx.notify();
            },
        ));

        self._subscriptions.push(cx.subscribe_in(
            &self.model_select,
            window,
            |this,
             _state,
             event: &SelectEvent<SearchableVec<SelectGroup<ModelChoice>>>,
             window,
             cx| {
                let SelectEvent::Confirm(Some(model_value)) = event else {
                    return;
                };
                this.handle_model_change(model_value.clone(), window, cx);
            },
        ));
    }

    fn refresh_model_choices_from_store(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_model_value = self.model_select.read(cx).selected_value().cloned();
        let ModelStoreSnapshot { models, status, .. } = Self::model_store_snapshot(cx);
        let models_changed = self.model_store_models != models;
        let status_changed = self.model_store_status != status;

        self.model_store_status = status;
        if !models_changed {
            if status_changed {
                cx.notify();
            }
            return;
        }

        self.model_store_models = models.clone();
        let (fallback_provider, fallback_model) = self
            .initial_binding
            .as_ref()
            .filter(|binding| Some(binding.id) == self.binding_id)
            .map(|binding| (binding.provider_name.as_str(), binding.model_id.as_str()))
            .unwrap_or((self.provider_name.as_str(), ""));
        let selected_key = selected_model_key_for_refresh(
            previous_model_value.as_deref(),
            fallback_provider,
            fallback_model,
            &models,
        );
        let unresolved_model = selected_key
            .as_deref()
            .and_then(Self::split_model_choice_key);
        let model_choices = Self::model_choices_from(&models, unresolved_model, cx);
        let selected_index = selected_key
            .as_deref()
            .and_then(|key| Self::model_selected_index(&model_choices, key));

        self.model_select.update(cx, |select, cx| {
            select.set_items(model_choices, window, cx);
            select.set_selected_index(selected_index, window, cx);
        });

        let current_model_value = self.model_select.read(cx).selected_value().cloned();
        if let Some((provider_name, _)) = current_model_value
            .as_deref()
            .and_then(Self::split_model_choice_key)
        {
            self.provider_name = provider_name.to_string();
        }

        let saved_template = should_preserve_request_template_on_model_refresh(
            previous_model_value.as_deref(),
            current_model_value.as_deref(),
        )
        .then(|| self.request_template.clone());
        Self::refresh_request_template_with_models(
            self,
            &models,
            saved_template.as_ref(),
            false,
            window,
            cx,
        );
        cx.notify();
    }

    fn handle_model_change(
        &mut self,
        model_value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = Self::available_models(cx);
        let resolved_model =
            Self::split_model_choice_key(&model_value).and_then(|(provider_name, model_id)| {
                available_models
                    .iter()
                    .find(|model| model.provider_name == provider_name && model.id == model_id)
                    .map(|_| provider_name)
            });
        if let Some(provider_name) = resolved_model {
            self.provider_name = provider_name.to_string();
        }
        Self::refresh_request_template_with_models(self, &available_models, None, true, window, cx);
        self.save_error = None;
        cx.notify();
    }

    fn handle_boolean_ext_setting(
        &mut self,
        setting_key: &'static str,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = Self::available_models(cx);
        let Some(model) = Self::current_model_from(self, &available_models, cx) else {
            return;
        };
        let Some(FormExtSettingState::Boolean { item }) = self
            .ext_settings
            .iter_mut()
            .find(|setting| matches!(setting, FormExtSettingState::Boolean { item } if item.key == setting_key))
        else {
            return;
        };
        item.control = ExtSettingControl::Boolean(value);
        if apply_ext_setting(&model, &mut self.request_template, item).is_ok() {
            Self::refresh_ext_settings(self, &model, window, cx);
        }
        self.save_error = None;
        cx.notify();
    }

    fn handle_select_ext_setting(
        &mut self,
        setting_key: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = Self::available_models(cx);
        let Some(model) = Self::current_model_from(self, &available_models, cx) else {
            return;
        };
        let Some(FormExtSettingState::Select { item, .. }) = self
            .ext_settings
            .iter_mut()
            .find(|setting| matches!(setting, FormExtSettingState::Select { item, .. } if item.key == setting_key))
        else {
            return;
        };
        let options = match &item.control {
            ExtSettingControl::Select { options, .. } => options.clone(),
            ExtSettingControl::Boolean(_) => return,
        };
        item.control = ExtSettingControl::Select { value, options };
        if apply_ext_setting(&model, &mut self.request_template, item).is_ok() {
            Self::refresh_ext_settings(self, &model, window, cx);
        }
        self.save_error = None;
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Ok(mut conn) = cx.global::<Db>().get()
            && let Ok(bindings) = GlobalShortcutBinding::all(&mut conn)
        {
            self.existing_bindings = bindings;
        }
        let payload = match self.save_payload(cx) {
            Ok(payload) => payload,
            Err(err) => {
                self.save_error = Some(err.clone());
                notify_error(
                    cx.global::<I18n>().t("notify-save-shortcut-failed"),
                    err,
                    window,
                    cx,
                );
                cx.notify();
                return false;
            }
        };

        match GlobalHotkeyState::save_global_shortcut_binding(self.binding_id, payload, cx) {
            Ok(_) => {
                notify_success(
                    cx.global::<I18n>().t(if self.binding_id.is_some() {
                        "notify-shortcut-updated-success"
                    } else {
                        "notify-shortcut-created-success"
                    }),
                    window,
                    cx,
                );
                true
            }
            Err(err) => {
                let message: SharedString = err.to_string().into();
                self.save_error = Some(message.clone());
                notify_error(
                    cx.global::<I18n>().t("notify-save-shortcut-failed"),
                    message,
                    window,
                    cx,
                );
                cx.notify();
                false
            }
        }
    }

    fn save_payload(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<NewGlobalShortcutBinding, SharedString> {
        let temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.as_deref();
        let hotkey = match validate_hotkey(
            self.binding_id,
            self.hotkey.as_deref(),
            &self.existing_bindings,
            temporary_hotkey,
        ) {
            Ok(hotkey) => hotkey,
            Err(err) => {
                self.hotkey_error = Some(err.clone());
                return Err(err.message(cx));
            }
        };

        let model_value = self
            .model_select
            .read(cx)
            .selected_value()
            .cloned()
            .filter(|model| !model.is_empty())
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-model")))?;
        let (provider_name, model_id) = Self::split_model_choice_key(&model_value)
            .map(|(provider_name, model_id)| (provider_name.to_string(), model_id.to_string()))
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-model")))?;
        Ok(NewGlobalShortcutBinding {
            hotkey,
            enabled: self.enabled,
            template_id: self
                .template_select
                .read(cx)
                .selected_value()
                .cloned()
                .flatten(),
            provider_name,
            model_id,
            mode: self.chat_mode,
            request_template: self.request_template.clone(),
            input_source: self.input_source,
        })
    }

    fn is_dirty(&self, cx: &App) -> bool {
        let Some(binding) = self.initial_binding.as_ref() else {
            return false;
        };
        let template_id = self
            .template_select
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
        let model_value = self.model_select.read(cx).selected_value().cloned();
        let (provider_name, model_id) = model_value
            .as_deref()
            .and_then(Self::split_model_choice_key)
            .unwrap_or(("", ""));
        self.hotkey.as_deref() != Some(binding.hotkey.as_str())
            || self.enabled != binding.enabled
            || template_id != binding.template_id
            || provider_name != binding.provider_name
            || model_id != binding.model_id
            || self.chat_mode != binding.mode
            || self.input_source != binding.input_source
            || self.request_template != binding.request_template
    }

    fn available_models(cx: &App) -> Vec<ProviderModel> {
        Self::model_store_snapshot(cx).models
    }

    fn model_store_snapshot(cx: &App) -> ModelStoreSnapshot {
        cx.global::<ModelStore>().read(cx).snapshot()
    }

    fn template_options(&self, cx: &App) -> Vec<TemplateChoice> {
        let mut options = vec![TemplateChoice::none(cx)];
        options.extend(self.templates.iter().map(TemplateChoice::from_template));
        options
    }

    fn template_selected_index(&self, template_id: Option<i32>, cx: &App) -> IndexPath {
        let options = self.template_options(cx);
        options
            .iter()
            .position(|option| option.id == template_id)
            .map(|index| IndexPath::default().row(index))
            .unwrap_or_default()
    }

    fn model_choices_from(
        models: &[ProviderModel],
        unresolved_model: Option<(&str, &str)>,
        cx: &App,
    ) -> SearchableVec<SelectGroup<ModelChoice>> {
        let mut grouped = std::collections::BTreeMap::<String, Vec<ModelChoice>>::new();
        for model in models {
            grouped
                .entry(model.provider_name.clone())
                .or_default()
                .push(ModelChoice::from_model(model));
        }
        if let Some((provider_name, model_id)) = unresolved_model {
            let exists = models
                .iter()
                .any(|model| model.provider_name == provider_name && model.id == model_id);
            if !exists && !provider_name.is_empty() && !model_id.is_empty() {
                grouped
                    .entry(provider_name.to_string())
                    .or_default()
                    .insert(0, ModelChoice::unresolved(provider_name, model_id, cx));
            }
        }
        SearchableVec::new(
            grouped
                .into_iter()
                .map(|(provider, models)| SelectGroup::new(provider).items(models))
                .collect::<Vec<_>>(),
        )
    }

    fn model_selected_index(
        choices: &SearchableVec<SelectGroup<ModelChoice>>,
        value: &str,
    ) -> Option<IndexPath> {
        (!value.is_empty())
            .then(|| choices.position(&value.to_string()))
            .flatten()
    }

    fn split_model_choice_key(value: &str) -> Option<(&str, &str)> {
        value.split_once('\u{1f}')
    }

    fn current_model_from(
        form: &ShortcutFormState,
        available_models: &[ProviderModel],
        cx: &App,
    ) -> Option<ProviderModel> {
        let model_value = form.model_select.read(cx).selected_value().cloned()?;
        let (provider_name, model_id) = Self::split_model_choice_key(&model_value)?;
        available_models
            .iter()
            .find(|model| model.provider_name == provider_name && model.id == model_id)
            .cloned()
    }

    fn refresh_request_template_with_models(
        form: &mut ShortcutFormState,
        available_models: &[ProviderModel],
        saved_template: Option<&serde_json::Value>,
        reset_when_unresolved: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(model) = Self::current_model_from(form, available_models, cx) else {
            form.model_resolved = false;
            form.ext_settings.clear();
            if reset_when_unresolved {
                form.request_template = serde_json::json!({});
            }
            return;
        };

        form.model_resolved = true;
        form.request_template =
            build_request_template(&model, saved_template).unwrap_or_else(|_| {
                saved_template
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}))
            });
        Self::refresh_ext_settings(form, &model, window, cx);
    }

    fn refresh_ext_settings(
        form: &mut ShortcutFormState,
        model: &ProviderModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        form.ext_settings.clear();
        let settings = match preset_ext_settings(model, &form.request_template) {
            Ok(settings) => settings,
            Err(_) => return,
        };

        for setting in settings {
            match &setting.control {
                ExtSettingControl::Select { value, options } => {
                    let items = options
                        .iter()
                        .map(|option| ExtSettingChoice {
                            value: option.value.to_string(),
                            label: cx.global::<I18n>().t(option.label_key).into(),
                        })
                        .collect::<Vec<_>>();
                    let selected_index = items
                        .iter()
                        .position(|item| &item.value == value)
                        .unwrap_or_default();
                    let state = cx.new(|cx| {
                        SelectState::new(
                            items.clone(),
                            Some(IndexPath::default().row(selected_index)),
                            window,
                            cx,
                        )
                    });
                    let setting_key = setting.key;
                    form._subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this,
                              _state,
                              event: &SelectEvent<Vec<ExtSettingChoice>>,
                              window,
                              cx| {
                            let SelectEvent::Confirm(Some(value)) = event else {
                                return;
                            };
                            this.handle_select_ext_setting(setting_key, value.clone(), window, cx);
                        },
                    ));
                    form.ext_settings.push(FormExtSettingState::Select {
                        item: setting,
                        state,
                    });
                }
                ExtSettingControl::Boolean(_) => {
                    form.ext_settings
                        .push(FormExtSettingState::Boolean { item: setting });
                }
            }
        }
    }

    fn render_hotkey_error(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = self
            .hotkey_error
            .as_ref()
            .map(|err| err.message(cx))
            .or_else(|| Some(SharedString::from("")));
        div()
            .h_5()
            .text_xs()
            .text_color(cx.theme().danger)
            .child(message.unwrap_or_default())
            .into_any_element()
    }

    fn render_mode_segments(&self, cx: &mut Context<Self>) -> AnyElement {
        let current_index = mode_option_index(self.chat_mode);
        ToggleGroup::new("shortcut-form-mode-segments")
            .segmented()
            .outline()
            .w_full()
            .children(mode_options().into_iter().enumerate().map(|(index, mode)| {
                render_segment_toggle(
                    ("shortcut-form-mode-segment", index as u64),
                    mode_label(mode, cx),
                    mode_description(mode, cx),
                    index == current_index,
                )
            }))
            .on_click(cx.listener(move |form, checkeds: &Vec<bool>, _window, cx| {
                let next_index = single_selected_index(mode_option_index(form.chat_mode), checkeds);
                if let Some(mode) = mode_options().get(next_index).copied() {
                    form.chat_mode = mode;
                    form.save_error = None;
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    fn render_input_source_segments(&self, cx: &mut Context<Self>) -> AnyElement {
        let current_index = input_source_option_index(self.input_source);
        ToggleGroup::new("shortcut-form-input-source-segments")
            .segmented()
            .outline()
            .w_full()
            .children(input_source_options().into_iter().enumerate().map(
                |(index, input_source)| {
                    render_segment_toggle(
                        ("shortcut-form-input-source-segment", index as u64),
                        input_source_label(input_source, cx),
                        input_source_description(input_source, cx),
                        index == current_index,
                    )
                },
            ))
            .on_click(cx.listener(move |form, checkeds: &Vec<bool>, _window, cx| {
                let next_index =
                    single_selected_index(input_source_option_index(form.input_source), checkeds);
                if let Some(input_source) = input_source_options().get(next_index).copied() {
                    form.input_source = input_source;
                    form.save_error = None;
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    fn render_preset_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.model_resolved {
            return v_flex()
                .w_full()
                .gap_2()
                .rounded(px(8.))
                .border_1()
                .border_color(cx.theme().border)
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(cx.global::<I18n>().t("shortcut-ext-settings-unavailable"))
                .into_any_element();
        }

        if self.ext_settings.is_empty() {
            return div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(cx.global::<I18n>().t("field-none"))
                .into_any_element();
        }

        let controls = self.ext_settings.iter().map(|setting| match setting {
            FormExtSettingState::Select { item, state } => h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    v_flex()
                        .min_w_0()
                        .gap_1()
                        .child(Label::new(cx.global::<I18n>().t(item.label_key)).text_sm())
                        .when_some(item.tooltip, |this, tooltip| {
                            this.child(
                                Label::new(cx.global::<I18n>().t(tooltip))
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate(),
                            )
                        }),
                )
                .child(Select::new(state).small().w(px(180.)))
                .into_any_element(),
            FormExtSettingState::Boolean { item } => {
                let ExtSettingControl::Boolean(value) = item.control else {
                    unreachable!();
                };
                let setting_key = item.key;
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(Label::new(cx.global::<I18n>().t(item.label_key)).text_sm())
                            .when_some(item.tooltip, |this, tooltip| {
                                this.child(
                                    Label::new(cx.global::<I18n>().t(tooltip))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate(),
                                )
                            }),
                    )
                    .child(
                        Switch::new(item.key)
                            .checked(value)
                            .small()
                            .on_click(cx.listener(move |this, checked, window, cx| {
                                this.handle_boolean_ext_setting(setting_key, *checked, window, cx);
                            })),
                    )
                    .into_any_element()
            }
        });

        let _ = window;
        v_flex()
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border)
            .children(controls.enumerate().map(|(index, control)| {
                v_flex()
                    .w_full()
                    .when(index > 0, |this| {
                        this.border_t_1().border_color(cx.theme().border)
                    })
                    .p_3()
                    .child(control)
                    .into_any_element()
            }))
            .into_any_element()
    }
}

impl Render for ShortcutFormState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = matches!(self.mode, ShortcutDialogMode::Edit) && self.is_dirty(cx);
        let (
            loading_model_picker,
            empty_model_picker,
            unsaved_changes,
            field_template,
            field_model,
            field_mode,
            field_send_content,
            field_hotkey,
            field_enabled,
            preset_settings,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("shortcut-models-loading"),
                i18n.t("empty-model-picker"),
                i18n.t("shortcut-unsaved-changes"),
                i18n.t("field-template"),
                i18n.t("field-model"),
                i18n.t("field-mode"),
                i18n.t("field-send-content"),
                i18n.t("field-hotkey"),
                i18n.t("field-enabled"),
                i18n.t("shortcut-preset-settings"),
            )
        };
        let model_empty_label = if model_store_is_loading(self.model_store_status) {
            loading_model_picker
        } else {
            empty_model_picker
        };

        v_flex()
            .w_full()
            .min_w_0()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .when(dirty, |this| {
                        this.child(
                            div()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(cx.theme().warning.opacity(0.55))
                                .bg(cx.theme().warning.opacity(0.12))
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child(unsaved_changes.clone()),
                        )
                    })
                    .when_some(self.save_error.clone(), |this, error| {
                        this.child(
                            Label::new(error)
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .truncate(),
                        )
                    }),
            )
            .child(
                v_form()
                    .w_full()
                    .min_w_0()
                    .child(
                        field().label(field_template.clone()).child(
                            Select::new(&self.template_select)
                                .placeholder(field_template.clone())
                                .w_full(),
                        ),
                    )
                    .child(
                        field().label(field_model.clone()).child(
                            Select::new(&self.model_select)
                                .placeholder(field_model.clone())
                                .empty(Label::new(model_empty_label).text_sm())
                                .w_full(),
                        ),
                    )
                    .child(
                        field()
                            .label(field_mode.clone())
                            .child(self.render_mode_segments(cx)),
                    )
                    .child(
                        field()
                            .label(field_send_content.clone())
                            .child(self.render_input_source_segments(cx)),
                    )
                    .child(
                        field().required(true).label(field_hotkey).child(
                            v_flex()
                                .gap_1()
                                .child(self.hotkey_input.clone())
                                .child(self.render_hotkey_error(cx)),
                        ),
                    )
                    .child(
                        field().label(field_enabled).child(
                            Switch::new("shortcut-form-enabled")
                                .checked(self.enabled)
                                .on_click(cx.listener(|this, checked, _window, cx| {
                                    this.enabled = *checked;
                                    this.save_error = None;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(field().child(Divider::horizontal()))
                    .child(
                        field()
                            .label(preset_settings)
                            .child(self.render_preset_settings(window, cx)),
                    ),
            )
    }
}

fn mode_options() -> [Mode; 3] {
    [Mode::Contextual, Mode::Single, Mode::AssistantOnly]
}

fn mode_option_index(mode: Mode) -> usize {
    mode_options()
        .iter()
        .position(|option| *option == mode)
        .unwrap_or(0)
}

fn input_source_options() -> [ShortcutInputSource; 2] {
    [
        ShortcutInputSource::SelectionOrClipboard,
        ShortcutInputSource::Screenshot,
    ]
}

fn input_source_option_index(input_source: ShortcutInputSource) -> usize {
    input_source_options()
        .iter()
        .position(|option| *option == input_source)
        .unwrap_or(0)
}

fn mode_label(mode: Mode, cx: &App) -> String {
    let key = match mode {
        Mode::Contextual => "mode-contextual",
        Mode::Single => "mode-single",
        Mode::AssistantOnly => "mode-assistant-only",
    };
    cx.global::<I18n>().t(key)
}

fn mode_description(mode: Mode, cx: &App) -> String {
    let key = match mode {
        Mode::Contextual => "shortcut-mode-contextual-description",
        Mode::Single => "shortcut-mode-single-description",
        Mode::AssistantOnly => "shortcut-mode-assistant-only-description",
    };
    cx.global::<I18n>().t(key)
}

fn input_source_label(input_source: ShortcutInputSource, cx: &App) -> String {
    let key = match input_source {
        ShortcutInputSource::SelectionOrClipboard => "send-content-selection-or-clipboard",
        ShortcutInputSource::Screenshot => "send-content-screenshot",
    };
    cx.global::<I18n>().t(key)
}

fn input_source_description(input_source: ShortcutInputSource, cx: &App) -> String {
    let key = match input_source {
        ShortcutInputSource::SelectionOrClipboard => {
            "shortcut-input-selection-or-clipboard-description"
        }
        ShortcutInputSource::Screenshot => "shortcut-input-screenshot-description",
    };
    cx.global::<I18n>().t(key)
}

fn model_store_is_loading(status: Option<ModelStoreStatus>) -> bool {
    matches!(
        status,
        Some(ModelStoreStatus::InitialLoading | ModelStoreStatus::Refreshing)
    )
}

fn selected_model_key_for_refresh(
    current_value: Option<&str>,
    fallback_provider_name: &str,
    fallback_model_id: &str,
    models: &[ProviderModel],
) -> Option<String> {
    current_value
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            (!fallback_provider_name.is_empty() && !fallback_model_id.is_empty())
                .then(|| ModelChoice::key(fallback_provider_name, fallback_model_id))
        })
        .or_else(|| {
            models
                .first()
                .map(|model| ModelChoice::key(&model.provider_name, &model.id))
        })
}

fn should_preserve_request_template_on_model_refresh(
    previous_value: Option<&str>,
    current_value: Option<&str>,
) -> bool {
    matches!((previous_value, current_value), (Some(previous), Some(current)) if previous == current)
}

fn render_segment_toggle(
    id: impl Into<ElementId>,
    title: String,
    description: String,
    checked: bool,
) -> Toggle {
    Toggle::new(id)
        .checked(checked)
        .flex_1()
        .min_w_0()
        .h_auto()
        .min_h(px(58.))
        .px_3()
        .py_2()
        .child(
            v_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .gap_1()
                .child(Label::new(title).text_sm().font_medium().truncate())
                .child(Label::new(description).text_xs().truncate()),
        )
}

fn notify_error(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    window.push_notification(
        Notification::new()
            .title(title)
            .message(message)
            .with_type(NotificationType::Error),
        cx,
    );
}

fn notify_success(title: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
    window.push_notification(
        Notification::new()
            .title(title)
            .with_type(NotificationType::Success),
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        input_source_option_index, input_source_options, mode_option_index, mode_options,
        model_store_is_loading, selected_model_key_for_refresh,
        should_preserve_request_template_on_model_refresh,
    };
    use crate::database::{Mode, ShortcutInputSource};
    use crate::{
        features::settings::shortcut_settings::choices::ModelChoice,
        llm::{ProviderModel, ProviderModelCapability},
        state::ModelStoreStatus,
    };

    fn model(provider_name: &str, model_id: &str) -> ProviderModel {
        ProviderModel::new(provider_name, model_id, ProviderModelCapability::Streaming)
    }

    #[test]
    fn mode_segments_cover_saved_modes_in_stable_order() {
        assert_eq!(
            mode_options(),
            [Mode::Contextual, Mode::Single, Mode::AssistantOnly]
        );
        assert_eq!(mode_option_index(Mode::Contextual), 0);
        assert_eq!(mode_option_index(Mode::Single), 1);
        assert_eq!(mode_option_index(Mode::AssistantOnly), 2);
    }

    #[test]
    fn input_source_segments_cover_saved_sources_in_stable_order() {
        assert_eq!(
            input_source_options(),
            [
                ShortcutInputSource::SelectionOrClipboard,
                ShortcutInputSource::Screenshot
            ]
        );
        assert_eq!(
            input_source_option_index(ShortcutInputSource::SelectionOrClipboard),
            0
        );
        assert_eq!(
            input_source_option_index(ShortcutInputSource::Screenshot),
            1
        );
    }

    #[test]
    fn model_store_loading_status_matches_refresh_states() {
        assert!(model_store_is_loading(Some(
            ModelStoreStatus::InitialLoading
        )));
        assert!(model_store_is_loading(Some(ModelStoreStatus::Refreshing)));
        assert!(!model_store_is_loading(Some(ModelStoreStatus::Idle)));
        assert!(!model_store_is_loading(None));
    }

    #[test]
    fn model_refresh_selects_default_when_previous_selection_is_empty() {
        let models = vec![model("OpenAI", "gpt-5.4-mini")];

        assert_eq!(
            selected_model_key_for_refresh(None, "", "", &models),
            Some(ModelChoice::key("OpenAI", "gpt-5.4-mini"))
        );
    }

    #[test]
    fn model_refresh_preserves_unresolved_selection_until_it_resolves() {
        let unresolved = ModelChoice::key("OpenAI", "gpt-5.4-mini");
        let models = vec![model("OpenAI", "gpt-5.4-mini")];

        assert_eq!(
            selected_model_key_for_refresh(Some(&unresolved), "", "", &[]),
            Some(unresolved.clone())
        );
        assert_eq!(
            selected_model_key_for_refresh(Some(&unresolved), "", "", &models),
            Some(unresolved)
        );
    }

    #[test]
    fn model_refresh_does_not_clear_user_selected_model() {
        let selected = ModelChoice::key("Ollama", "qwen3");
        let models = vec![model("OpenAI", "gpt-5.4-mini"), model("Ollama", "qwen3")];

        assert_eq!(
            selected_model_key_for_refresh(Some(&selected), "", "", &models),
            Some(selected)
        );
    }

    #[test]
    fn request_template_is_preserved_only_when_model_selection_stays_same() {
        let selected = ModelChoice::key("OpenAI", "gpt-5.4-mini");
        let changed = ModelChoice::key("Ollama", "qwen3");

        assert!(should_preserve_request_template_on_model_refresh(
            Some(&selected),
            Some(&selected)
        ));
        assert!(!should_preserve_request_template_on_model_refresh(
            None,
            Some(&selected)
        ));
        assert!(!should_preserve_request_template_on_model_refresh(
            Some(&selected),
            Some(&changed)
        ));
    }
}
