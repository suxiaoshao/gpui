use crate::{
    components::hotkey_input::{
        HotkeyEvent, HotkeyInput, format_hotkey_label, string_to_keystroke,
    },
    database::{
        ConversationTemplate, Db, GlobalShortcutBinding, Mode, NewGlobalShortcutBinding,
        ShortcutInputSource,
    },
    features::{
        hotkey::GlobalHotkeyState,
        settings::shortcut_settings::{
            SHORTCUT_DIALOG_MARGIN_TOP, SHORTCUT_DIALOG_MAX_HEIGHT, SHORTCUT_DIALOG_WIDTH,
            choices::{
                ExtSettingChoice, InputSourceChoice, ModeChoice, ModelChoice, TemplateChoice,
            },
            validation::{ShortcutValidationError, validate_hotkey},
        },
    },
    foundation::{assets::IconName, i18n::I18n},
    llm::{
        ExtSettingControl, ExtSettingItem, ProviderModel, apply_ext_setting,
        build_request_template, preset_ext_settings,
    },
    state::{AiChatConfig, ModelStore},
};
use gpui::{AppContext as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::DialogFooter,
    divider::Divider,
    form::{field, v_form},
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    select::{SearchableVec, Select, SelectDelegate, SelectEvent, SelectGroup, SelectState},
    v_flex,
};
use std::rc::Rc;

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

#[derive(Clone, Copy)]
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
    mode_select: Entity<SelectState<Vec<ModeChoice>>>,
    input_source_select: Entity<SelectState<Vec<InputSourceChoice>>>,
    hotkey_input: Entity<HotkeyInput>,
    provider_name: String,
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
        let mode_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
        let input_source_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
        let hotkey_input = cx.new(|cx| HotkeyInput::new("shortcut-form-hotkey", window, cx));
        let mut this = Self {
            mode,
            binding_id: binding.as_ref().map(|binding| binding.id),
            initial_binding: binding.clone(),
            existing_bindings,
            templates,
            template_select,
            model_select,
            mode_select,
            input_source_select,
            hotkey_input,
            provider_name: String::new(),
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
        let available_models = Self::available_models(cx);
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
        let model_selected = self.model_selected_index(
            &model_choices,
            &ModelChoice::key(&self.provider_name, &model_id),
            cx,
        );
        self.model_select = cx.new(|cx| {
            SelectState::new(model_choices, Some(model_selected), window, cx).searchable(true)
        });
        self.mode_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    ModeChoice::new(Mode::Contextual, cx),
                    ModeChoice::new(Mode::Single, cx),
                    ModeChoice::new(Mode::AssistantOnly, cx),
                ],
                Some(self.mode_selected_index(mode)),
                window,
                cx,
            )
        });
        self.input_source_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    InputSourceChoice::new(ShortcutInputSource::SelectionOrClipboard, cx),
                    InputSourceChoice::new(ShortcutInputSource::Screenshot, cx),
                ],
                Some(self.input_source_selected_index(input_source)),
                window,
                cx,
            )
        });
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

        self._subscriptions.push(cx.subscribe_in(
            &self.input_source_select,
            window,
            |this, _state, event: &SelectEvent<Vec<InputSourceChoice>>, _window, cx| {
                let SelectEvent::Confirm(Some(input_source)) = event else {
                    return;
                };
                this.input_source = *input_source;
                this.save_error = None;
                cx.notify();
            },
        ));
    }

    fn handle_model_change(
        &mut self,
        model_value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = Self::available_models(cx);
        if let Some((provider_name, model_id)) = Self::split_model_choice_key(&model_value)
            && available_models
                .iter()
                .any(|model| model.provider_name == provider_name && model.id == model_id)
        {
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
        let mode = self
            .mode_select
            .read(cx)
            .selected_value()
            .copied()
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-mode")))?;

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
            mode,
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
        let mode = self.mode_select.read(cx).selected_value().copied();
        self.hotkey.as_deref() != Some(binding.hotkey.as_str())
            || self.enabled != binding.enabled
            || template_id != binding.template_id
            || provider_name != binding.provider_name
            || model_id != binding.model_id
            || mode != Some(binding.mode)
            || self.input_source != binding.input_source
            || self.request_template != binding.request_template
    }

    fn available_models(cx: &App) -> Vec<ProviderModel> {
        cx.global::<ModelStore>().read(cx).snapshot().models
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
        &self,
        choices: &SearchableVec<SelectGroup<ModelChoice>>,
        value: &str,
        _cx: &App,
    ) -> IndexPath {
        choices.position(&value.to_string()).unwrap_or_default()
    }

    fn mode_selected_index(&self, mode: Mode) -> IndexPath {
        match mode {
            Mode::Contextual => IndexPath::default().row(0),
            Mode::Single => IndexPath::default().row(1),
            Mode::AssistantOnly => IndexPath::default().row(2),
        }
    }

    fn input_source_selected_index(&self, input_source: ShortcutInputSource) -> IndexPath {
        match input_source {
            ShortcutInputSource::SelectionOrClipboard => IndexPath::default().row(0),
            ShortcutInputSource::Screenshot => IndexPath::default().row(1),
        }
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
                    .child(Checkbox::new(item.key).checked(value).on_click(cx.listener(
                        move |this, checked, window, cx| {
                            this.handle_boolean_ext_setting(setting_key, *checked, window, cx);
                        },
                    )))
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
            empty_model_picker,
            unsaved_changes,
            field_template,
            field_model,
            field_mode,
            field_send_content,
            field_hotkey,
            field_enabled,
            hotkey_placeholder,
            preset_settings,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("empty-model-picker"),
                i18n.t("shortcut-unsaved-changes"),
                i18n.t("field-template"),
                i18n.t("field-model"),
                i18n.t("field-mode"),
                i18n.t("field-send-content"),
                i18n.t("field-hotkey"),
                i18n.t("field-enabled"),
                i18n.t("shortcut-hotkey-placeholder"),
                i18n.t("shortcut-preset-settings"),
            )
        };
        let formatted_hotkey = self
            .hotkey
            .as_deref()
            .map(format_hotkey_label)
            .unwrap_or_default();

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
                                .empty(Label::new(empty_model_picker).text_sm())
                                .w_full(),
                        ),
                    )
                    .child(
                        field().label(field_mode.clone()).child(
                            Select::new(&self.mode_select)
                                .placeholder(field_mode.clone())
                                .w_full(),
                        ),
                    )
                    .child(
                        field().label(field_send_content.clone()).child(
                            Select::new(&self.input_source_select)
                                .placeholder(field_send_content.clone())
                                .menu_width(px(320.))
                                .w_full(),
                        ),
                    )
                    .child(
                        field().required(true).label(field_hotkey).child(
                            v_flex()
                                .gap_1()
                                .child(self.hotkey_input.clone())
                                .child(self.render_hotkey_error(cx))
                                .child(
                                    Label::new(if formatted_hotkey.is_empty() {
                                        hotkey_placeholder
                                    } else {
                                        formatted_hotkey
                                    })
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                    )
                    .child(
                        field().label(field_enabled).child(
                            Checkbox::new("shortcut-form-enabled")
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
