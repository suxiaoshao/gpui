use crate::{
    assets::IconName,
    components::{
        delete_confirm::open_delete_confirm_dialog,
        hotkey_input::{HotkeyEvent, HotkeyInput, string_to_keystroke},
    },
    database::{
        ConversationTemplate, Db, GlobalShortcutBinding, Mode, NewGlobalShortcutBinding,
        ShortcutInputSource,
    },
    hotkey::GlobalHotkeyState,
    i18n::I18n,
    llm::{
        ExtSettingControl, ExtSettingItem, ProviderModel, apply_ext_setting,
        build_request_template, preset_ext_settings,
    },
    state::ModelStore,
};
use gpui::{AppContext as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, IndexPath, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    popover::Popover,
    select::{
        SearchableVec, Select, SelectDelegate, SelectEvent, SelectGroup, SelectItem, SelectState,
    },
    v_flex,
};
use std::{collections::BTreeMap, ops::Deref};

#[derive(Clone)]
struct TemplateChoice {
    id: Option<i32>,
    label: SharedString,
    template: Option<ConversationTemplate>,
}

impl TemplateChoice {
    fn none(cx: &App) -> Self {
        Self {
            id: None,
            label: cx.global::<I18n>().t("field-none").into(),
            template: None,
        }
    }

    fn from_template(template: &ConversationTemplate) -> Self {
        Self {
            id: Some(template.id),
            label: SharedString::from(format!("{} {}", template.icon, template.name)),
            template: Some(template.clone()),
        }
    }
}

impl SelectItem for TemplateChoice {
    type Value = Option<i32>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn matches(&self, query: &str) -> bool {
        self.template.as_ref().map_or_else(
            || self.label.to_lowercase().contains(&query.to_lowercase()),
            |template| template.matches_search_query(query),
        )
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

#[derive(Clone)]
struct ModelChoice {
    value: String,
    title: SharedString,
}

impl ModelChoice {
    fn key(provider_name: &str, model_id: &str) -> String {
        format!("{provider_name}\u{1f}{model_id}")
    }

    fn from_model(model: &ProviderModel) -> Self {
        Self {
            value: Self::key(&model.provider_name, &model.id),
            title: model.id.clone().into(),
        }
    }

    fn unresolved(provider_name: &str, model_id: &str, cx: &App) -> Self {
        Self {
            value: Self::key(provider_name, model_id),
            title: SharedString::from(format!(
                "{} ({})",
                model_id,
                cx.global::<I18n>().t("shortcut-model-unavailable")
            )),
        }
    }
}

impl SelectItem for ModelChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
struct ModeChoice {
    value: Mode,
    label: SharedString,
}

impl ModeChoice {
    fn new(value: Mode, cx: &App) -> Self {
        let label = {
            let key = match value {
                Mode::Contextual => "mode-contextual",
                Mode::Single => "mode-single",
                Mode::AssistantOnly => "mode-assistant-only",
            };
            cx.global::<I18n>().t(key).into()
        };
        Self { value, label }
    }

    fn label(&self) -> SharedString {
        self.label.clone()
    }
}

impl SelectItem for ModeChoice {
    type Value = Mode;

    fn title(&self) -> SharedString {
        self.value.to_string().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
struct InputSourceChoice {
    value: ShortcutInputSource,
    label: SharedString,
}

impl InputSourceChoice {
    fn new(value: ShortcutInputSource, cx: &App) -> Self {
        let label = {
            let key = match value {
                ShortcutInputSource::SelectionOrClipboard => "send-content-selection-or-clipboard",
                ShortcutInputSource::Screenshot => "send-content-screenshot",
            };
            cx.global::<I18n>().t(key).into()
        };
        Self { value, label }
    }

    fn label(&self) -> SharedString {
        self.label.clone()
    }
}

impl SelectItem for InputSourceChoice {
    type Value = ShortcutInputSource;

    fn title(&self) -> SharedString {
        self.value.to_string().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
struct ExtSettingChoice {
    value: String,
    label: SharedString,
}

impl SelectItem for ExtSettingChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.value.clone().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label.clone()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

enum RowExtSettingState {
    Select {
        item: ExtSettingItem,
        state: Entity<SelectState<Vec<ExtSettingChoice>>>,
    },
    Boolean {
        item: ExtSettingItem,
    },
}

struct ShortcutBindingRowState {
    key: u64,
    binding_id: Option<i32>,
    template_select: Entity<SelectState<Vec<TemplateChoice>>>,
    model_select: Entity<SelectState<SearchableVec<SelectGroup<ModelChoice>>>>,
    mode_select: Entity<SelectState<Vec<ModeChoice>>>,
    input_source_select: Entity<SelectState<Vec<InputSourceChoice>>>,
    hotkey_input: Entity<HotkeyInput>,
    provider_name: String,
    hotkey: Option<String>,
    invalid_hotkey: Option<String>,
    enabled: bool,
    input_source: ShortcutInputSource,
    request_template: serde_json::Value,
    ext_settings: Vec<RowExtSettingState>,
    model_resolved: bool,
    saved_binding: Option<GlobalShortcutBinding>,
    _subscriptions: Vec<Subscription>,
}

struct ShortcutSaveData {
    row_key: u64,
    binding_id: Option<i32>,
    hotkey: String,
    enabled: bool,
    template_id: Option<i32>,
    provider_name: String,
    model_id: String,
    mode: Mode,
    request_template: serde_json::Value,
    input_source: ShortcutInputSource,
}

pub(crate) struct ShortcutSettingsPage {
    rows: Vec<ShortcutBindingRowState>,
    templates: Vec<ConversationTemplate>,
    next_temp_key: u64,
    _subscriptions: Vec<Subscription>,
}

impl ShortcutSettingsPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model_store = cx.global::<ModelStore>().deref().clone();
        let model_subscription = cx.observe_in(&model_store, window, |this, _, window, cx| {
            this.refresh_model_backed_rows(window, cx);
        });

        model_store.update(cx, |store, cx| store.ensure_loaded(window, cx));

        let mut this = Self {
            rows: Vec::new(),
            templates: Vec::new(),
            next_temp_key: 1,
            _subscriptions: vec![model_subscription],
        };
        this.reload(window, cx);
        this
    }

    fn notify_error(
        &self,
        title_key: &str,
        message: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t(title_key))
                .message(message.into())
                .with_type(NotificationType::Error),
            cx,
        );
    }

    fn notify_success(&self, title_key: &str, window: &mut Window, cx: &mut App) {
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t(title_key))
                .with_type(NotificationType::Success),
            cx,
        );
    }

    fn refresh_view(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn available_models(&self, cx: &App) -> Vec<ProviderModel> {
        cx.global::<ModelStore>().read(cx).snapshot().models
    }

    fn resolve_model_from(
        available_models: &[ProviderModel],
        provider_name: &str,
        model_id: &str,
    ) -> Option<ProviderModel> {
        available_models
            .iter()
            .filter(|model| model.provider_name == provider_name)
            .find(|model| model.id == model_id)
            .cloned()
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let load = || -> anyhow::Result<(Vec<ConversationTemplate>, Vec<GlobalShortcutBinding>)> {
            let mut conn = cx.global::<Db>().get()?;
            Ok((
                ConversationTemplate::all(&mut conn)?,
                GlobalShortcutBinding::all(&mut conn)?,
            ))
        };

        let (templates, bindings) = match load() {
            Ok(data) => data,
            Err(err) => {
                self.notify_error("notify-load-shortcuts-failed", err.to_string(), window, cx);
                return;
            }
        };

        self.templates = templates;
        self.next_temp_key = bindings
            .iter()
            .map(|binding| binding.id as u64)
            .max()
            .unwrap_or(0)
            + 1;
        self.rows = bindings
            .into_iter()
            .map(|binding| self.build_row(Some(binding), window, cx))
            .collect();
        self.refresh_view(cx);
        cx.notify();
    }

    fn build_row(
        &mut self,
        binding: Option<GlobalShortcutBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ShortcutBindingRowState {
        let key = binding
            .as_ref()
            .map(|binding| binding.id as u64)
            .unwrap_or_else(|| {
                let next = self.next_temp_key;
                self.next_temp_key += 1;
                next
            });

        let hotkey = binding.as_ref().map(|binding| binding.hotkey.clone());
        let enabled = binding
            .as_ref()
            .map(|binding| binding.enabled)
            .unwrap_or(true);
        let template_id = binding.as_ref().and_then(|binding| binding.template_id);
        let provider_name = binding
            .as_ref()
            .map(|binding| binding.provider_name.clone())
            .or_else(|| {
                self.available_models(cx)
                    .first()
                    .map(|model| model.provider_name.clone())
            })
            .unwrap_or_default();
        let mode = binding
            .as_ref()
            .map(|binding| binding.mode)
            .unwrap_or(Mode::Contextual);
        let input_source = binding
            .as_ref()
            .map(|binding| binding.input_source)
            .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
        let model_id = binding
            .as_ref()
            .map(|binding| binding.model_id.clone())
            .or_else(|| {
                self.available_models(cx)
                    .first()
                    .map(|model| model.id.clone())
            })
            .unwrap_or_default();

        let template_select = cx.new(|cx| {
            SelectState::new(
                self.template_options(cx),
                Some(self.template_selected_index(template_id, cx)),
                window,
                cx,
            )
            .searchable(true)
        });

        let available_models = self.available_models(cx);
        let model_choices =
            Self::model_choices_from(&available_models, Some((&provider_name, &model_id)), cx);
        let model_selected = self.model_selected_index(
            &model_choices,
            &ModelChoice::key(&provider_name, &model_id),
            cx,
        );
        let model_select = cx.new(|cx| {
            SelectState::new(model_choices.clone(), Some(model_selected), window, cx)
                .searchable(true)
        });

        let mode_select = cx.new(|cx| {
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

        let input_source_select = cx.new(|cx| {
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

        let parsed_hotkey = hotkey.as_deref().and_then(string_to_keystroke);
        let invalid_hotkey = hotkey
            .as_ref()
            .filter(|hotkey| string_to_keystroke(hotkey).is_none())
            .cloned();
        let hotkey_input = cx.new(|cx| {
            HotkeyInput::new(("shortcut-hotkey", key), window, cx)
                .small()
                .w_full()
                .default_value(parsed_hotkey.clone())
        });

        let mut row = ShortcutBindingRowState {
            key,
            binding_id: binding.as_ref().map(|binding| binding.id),
            template_select,
            model_select,
            mode_select,
            input_source_select,
            hotkey_input,
            provider_name,
            hotkey: parsed_hotkey.map(|_| hotkey.clone().unwrap_or_default()),
            invalid_hotkey,
            enabled,
            input_source,
            request_template: binding
                .as_ref()
                .map(|binding| binding.request_template.clone())
                .unwrap_or_else(|| serde_json::json!({})),
            ext_settings: Vec::new(),
            model_resolved: false,
            saved_binding: binding.clone(),
            _subscriptions: Vec::new(),
        };

        Self::refresh_row_request_template_with_models(
            &mut row,
            &available_models,
            binding.as_ref().map(|binding| &binding.request_template),
            false,
            window,
            cx,
        );
        self.bind_row_subscriptions(&mut row, window, cx);
        row
    }

    fn bind_row_subscriptions(
        &self,
        row: &mut ShortcutBindingRowState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.entity().downgrade();
        let row_key = row.key;
        row._subscriptions.push(cx.subscribe_in(
            &row.hotkey_input,
            window,
            move |_this, _input, event: &HotkeyEvent, window, cx| {
                let event = event.clone();
                let page = page.clone();
                window.defer(cx, move |_window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.handle_hotkey_event(row_key, &event, cx);
                    });
                });
            },
        ));

        let page = cx.entity().downgrade();
        row._subscriptions.push(cx.subscribe_in(
            &row.model_select,
            window,
            move |_this,
                  _state,
                  event: &SelectEvent<SearchableVec<SelectGroup<ModelChoice>>>,
                  window,
                  cx| {
                let SelectEvent::Confirm(Some(model_id)) = event else {
                    return;
                };
                let model_id = model_id.clone();
                let page = page.clone();
                window.defer(cx, move |window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        page.handle_model_change(row_key, model_id, window, cx);
                    });
                });
            },
        ));

        let page = cx.entity().downgrade();
        row._subscriptions.push(cx.subscribe_in(
            &row.input_source_select,
            window,
            move |_this, _state, event: &SelectEvent<Vec<InputSourceChoice>>, window, cx| {
                let SelectEvent::Confirm(Some(input_source)) = event else {
                    return;
                };
                let input_source = *input_source;
                let page = page.clone();
                window.defer(cx, move |_window, cx| {
                    let _ = page.update(cx, |page, cx| {
                        let Some(row) = page.rows.iter_mut().find(|row| row.key == row_key) else {
                            return;
                        };
                        row.input_source = input_source;
                        cx.notify();
                    });
                });
            },
        ));
    }

    fn handle_hotkey_event(&mut self, row_key: u64, event: &HotkeyEvent, cx: &mut Context<Self>) {
        let Some(row) = self.rows.iter_mut().find(|row| row.key == row_key) else {
            return;
        };
        match event {
            HotkeyEvent::Confirm(value) => {
                row.hotkey = Some(value.to_string());
                row.invalid_hotkey = None;
            }
            HotkeyEvent::Cancel => {
                row.hotkey = None;
                row.invalid_hotkey = None;
            }
        }
        cx.notify();
    }

    fn handle_model_change(
        &mut self,
        row_key: u64,
        model_value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = self.available_models(cx);
        let Some(index) = self.rows.iter().position(|row| row.key == row_key) else {
            return;
        };
        let row = &mut self.rows[index];
        if let Some((provider_name, model_id)) = Self::split_model_choice_key(&model_value)
            && let Some(model) = available_models
                .iter()
                .find(|model| model.provider_name == provider_name && model.id == model_id)
        {
            row.provider_name = model.provider_name.clone();
        }
        Self::refresh_row_request_template_with_models(
            row,
            &available_models,
            None,
            true,
            window,
            cx,
        );
        self.refresh_view(cx);
        cx.notify();
    }

    fn handle_boolean_ext_setting(
        &mut self,
        row_key: u64,
        setting_key: &'static str,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = self.available_models(cx);
        let Some(index) = self.rows.iter().position(|row| row.key == row_key) else {
            return;
        };
        let row = &mut self.rows[index];
        let Some(model) = Self::current_model_from(row, &available_models, cx) else {
            return;
        };

        let Some(RowExtSettingState::Boolean { item }) = row
            .ext_settings
            .iter_mut()
            .find(|setting| matches!(setting, RowExtSettingState::Boolean { item } if item.key == setting_key))
        else {
            return;
        };

        item.control = ExtSettingControl::Boolean(value);
        if apply_ext_setting(&model, &mut row.request_template, item).is_ok() {
            Self::refresh_row_ext_settings(row, &model, window, cx);
        }
    }

    fn handle_select_ext_setting(
        &mut self,
        row_key: u64,
        setting_key: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available_models = self.available_models(cx);
        let Some(index) = self.rows.iter().position(|row| row.key == row_key) else {
            return;
        };
        let row = &mut self.rows[index];
        let Some(model) = Self::current_model_from(row, &available_models, cx) else {
            return;
        };

        let Some(RowExtSettingState::Select { item, .. }) = row
            .ext_settings
            .iter_mut()
            .find(|setting| matches!(setting, RowExtSettingState::Select { item, .. } if item.key == setting_key))
        else {
            return;
        };

        let options = match &item.control {
            ExtSettingControl::Select { options, .. } => options.clone(),
            ExtSettingControl::Boolean(_) => return,
        };
        item.control = ExtSettingControl::Select { value, options };
        if apply_ext_setting(&model, &mut row.request_template, item).is_ok() {
            Self::refresh_row_ext_settings(row, &model, window, cx);
        }
    }

    fn refresh_row_request_template_with_models(
        row: &mut ShortcutBindingRowState,
        available_models: &[ProviderModel],
        saved_template: Option<&serde_json::Value>,
        reset_when_unresolved: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(model) = Self::current_model_from(row, available_models, cx) else {
            row.model_resolved = false;
            row.ext_settings.clear();
            if reset_when_unresolved {
                row.request_template = serde_json::json!({});
            }
            return;
        };

        row.model_resolved = true;
        row.request_template =
            build_request_template(&model, saved_template).unwrap_or_else(|_| {
                saved_template
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}))
            });
        Self::refresh_row_ext_settings(row, &model, window, cx);
    }

    fn refresh_row_ext_settings(
        row: &mut ShortcutBindingRowState,
        model: &ProviderModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        row.ext_settings.clear();
        let settings = match preset_ext_settings(model, &row.request_template) {
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
                    let page = cx.entity().downgrade();
                    let row_key = row.key;
                    let setting_key = setting.key;
                    row._subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |_this,
                              _state,
                              event: &SelectEvent<Vec<ExtSettingChoice>>,
                              window,
                              cx| {
                            let SelectEvent::Confirm(Some(value)) = event else {
                                return;
                            };
                            let value = value.clone();
                            let page = page.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = page.update(cx, |page, cx| {
                                    page.handle_select_ext_setting(
                                        row_key,
                                        setting_key,
                                        value,
                                        window,
                                        cx,
                                    );
                                });
                            });
                        },
                    ));
                    row.ext_settings.push(RowExtSettingState::Select {
                        item: setting,
                        state,
                    });
                }
                ExtSettingControl::Boolean(_) => {
                    row.ext_settings
                        .push(RowExtSettingState::Boolean { item: setting });
                }
            }
        }
    }

    fn current_model_from(
        row: &ShortcutBindingRowState,
        available_models: &[ProviderModel],
        cx: &App,
    ) -> Option<ProviderModel> {
        let value = row.model_select.read(cx).selected_value()?.clone();
        let (provider_name, model_id) = Self::split_model_choice_key(&value)?;
        Self::resolve_model_from(available_models, provider_name, model_id)
    }

    fn template_options(&self, cx: &App) -> Vec<TemplateChoice> {
        std::iter::once(TemplateChoice::none(cx))
            .chain(self.templates.iter().map(TemplateChoice::from_template))
            .collect()
    }

    fn template_selected_index(&self, template_id: Option<i32>, cx: &App) -> IndexPath {
        self.template_options(cx)
            .iter()
            .position(|template| template.id == template_id)
            .map(|ix| IndexPath::default().row(ix))
            .unwrap_or_default()
    }

    fn model_choices_from(
        available_models: &[ProviderModel],
        selected_model: Option<(&str, &str)>,
        cx: &App,
    ) -> SearchableVec<SelectGroup<ModelChoice>> {
        let mut groups = BTreeMap::<String, Vec<ModelChoice>>::new();
        for model in available_models {
            groups
                .entry(model.provider_name.clone())
                .or_default()
                .push(ModelChoice::from_model(model));
        }
        if let Some((provider_name, model_id)) = selected_model
            && !model_id.is_empty()
            && !available_models
                .iter()
                .any(|model| model.provider_name == provider_name && model.id == model_id)
        {
            groups
                .entry(provider_name.to_string())
                .or_default()
                .insert(0, ModelChoice::unresolved(provider_name, model_id, cx));
        }
        SearchableVec::new(
            groups
                .into_iter()
                .map(|(provider_name, mut items)| {
                    items.sort_by_key(|item| item.title());
                    SelectGroup::new(provider_name).items(items)
                })
                .collect::<Vec<_>>(),
        )
    }

    fn model_selected_index(
        &self,
        items: &SearchableVec<SelectGroup<ModelChoice>>,
        model_key: &str,
        _: &App,
    ) -> IndexPath {
        items.position(&model_key.to_string()).unwrap_or_default()
    }

    fn split_model_choice_key(value: &str) -> Option<(&str, &str)> {
        value.split_once('\u{1f}')
    }

    fn mode_selected_index(&self, mode: Mode) -> IndexPath {
        let ix = match mode {
            Mode::Contextual => 0,
            Mode::Single => 1,
            Mode::AssistantOnly => 2,
        };
        IndexPath::default().row(ix)
    }

    fn input_source_selected_index(&self, input_source: ShortcutInputSource) -> IndexPath {
        let ix = match input_source {
            ShortcutInputSource::SelectionOrClipboard => 0,
            ShortcutInputSource::Screenshot => 1,
        };
        IndexPath::default().row(ix)
    }

    fn refresh_model_backed_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let available_models = self.available_models(cx);
        for row in &mut self.rows {
            let selected_model = row.model_select.read(cx).selected_value().cloned();
            let model_choices = Self::model_choices_from(
                &available_models,
                selected_model
                    .as_deref()
                    .and_then(Self::split_model_choice_key),
                cx,
            );
            row.model_select.update(cx, |state, cx| {
                state.set_items(model_choices.clone(), window, cx);
                if let Some(model_value) = selected_model.as_ref() {
                    state.set_selected_value(model_value, window, cx);
                }
            });

            let current_template = row.request_template.clone();
            Self::refresh_row_request_template_with_models(
                row,
                &available_models,
                Some(&current_template),
                false,
                window,
                cx,
            );
        }
        self.refresh_view(cx);
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .child(
                Label::new(cx.global::<I18n>().t("settings-group-shortcuts"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Button::new("shortcut-add")
                    .ghost()
                    .icon(IconName::Plus)
                    .tooltip(cx.global::<I18n>().t("button-add"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let row = this.build_row(None, window, cx);
                        this.rows.insert(0, row);
                        this.refresh_view(cx);
                        cx.notify();
                    })),
            )
    }

    fn render_ext_settings_content(
        &self,
        row: &ShortcutBindingRowState,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !row.model_resolved {
            return div()
                .w(px(320.))
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(cx.global::<I18n>().t("shortcut-ext-settings-unavailable"))
                .into_any_element();
        }

        if row.ext_settings.is_empty() {
            return div()
                .w(px(320.))
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(cx.global::<I18n>().t("field-none"))
                .into_any_element();
        }

        let controls = row.ext_settings.iter().map(|setting| match setting {
            RowExtSettingState::Select { item, state } => v_flex()
                .gap_1()
                .child(
                    Label::new(cx.global::<I18n>().t(item.label_key))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(Select::new(state).small().w(px(180.)))
                .into_any_element(),
            RowExtSettingState::Boolean { item } => {
                let ExtSettingControl::Boolean(value) = item.control else {
                    unreachable!();
                };
                let row_key = row.key;
                let setting_key = item.key;
                v_flex()
                    .gap_1()
                    .child(
                        Label::new(cx.global::<I18n>().t(item.label_key))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Checkbox::new((
                            ElementId::from(("shortcut-ext-setting", row.key)),
                            item.key,
                        ))
                        .checked(value)
                        .on_click(cx.listener(
                            move |this, checked, window, cx| {
                                this.handle_boolean_ext_setting(
                                    row_key,
                                    setting_key,
                                    *checked,
                                    window,
                                    cx,
                                );
                            },
                        )),
                    )
                    .into_any_element()
            }
        });

        v_flex()
            .w(px(320.))
            .p_3()
            .gap_3()
            .children(controls)
            .into_any_element()
    }

    fn render_ext_settings_cell(
        &self,
        row: &ShortcutBindingRowState,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = cx.entity().clone();
        let row_key = row.key;
        let has_ext_settings = row.model_resolved && !row.ext_settings.is_empty();
        let trigger_tooltip = if has_ext_settings {
            cx.global::<I18n>().t("button-edit")
        } else {
            cx.global::<I18n>().t("field-none")
        };

        Popover::new(("shortcut-preset-popover", row.key))
            .anchor(Anchor::BottomLeft)
            .appearance(false)
            .trigger(
                Button::new(("shortcut-preset-trigger", row.key))
                    .ghost()
                    .small()
                    .icon(IconName::Edit)
                    .tooltip(trigger_tooltip)
                    .disabled(!has_ext_settings),
            )
            .content(move |_, window, cx| {
                page.update(cx, |page, cx| {
                    let Some(row) = page.rows.iter().find(|row| row.key == row_key) else {
                        return div().into_any_element();
                    };
                    div()
                        .occlude()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(px(8.))
                        .shadow_md()
                        .child(page.render_ext_settings_content(row, window, cx))
                        .into_any_element()
                })
            })
            .into_any_element()
    }

    fn render_row_field(
        &self,
        label: SharedString,
        min_width: Pixels,
        child: impl IntoElement,
        cx: &App,
    ) -> AnyElement {
        v_flex()
            .flex_none()
            .w(min_width)
            .min_w(min_width)
            .gap_1()
            .child(
                Label::new(label)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(child)
            .into_any_element()
    }

    fn render_enabled_field(
        &self,
        row: &ShortcutBindingRowState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row_key = row.key;
        v_flex()
            .gap_1()
            .min_w(px(92.))
            .child(
                Label::new(cx.global::<I18n>().t("field-enabled"))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Checkbox::new(("shortcut-enabled", row.key))
                    .checked(row.enabled)
                    .on_click(cx.listener(move |this, checked, _window, cx| {
                        let Some(row) = this.rows.iter_mut().find(|row| row.key == row_key) else {
                            return;
                        };
                        row.enabled = *checked;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_shortcut_actions(
        &self,
        row: &ShortcutBindingRowState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row_key = row.key;
        let is_new = row.binding_id.is_none();
        let (save_tooltip, reset_tooltip, delete_tooltip) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t(if is_new {
                    "button-create"
                } else {
                    "button-save-shortcut"
                }),
                i18n.t(if is_new {
                    "button-cancel"
                } else {
                    "button-reset"
                }),
                i18n.t("button-delete"),
            )
        };

        h_flex()
            .flex_initial()
            .items_center()
            .gap_1()
            .child(
                Button::new(("shortcut-save", row.key))
                    .small()
                    .ghost()
                    .icon(if is_new {
                        IconName::Upload
                    } else {
                        IconName::Save
                    })
                    .tooltip(save_tooltip)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.save_row(row_key, window, cx);
                    })),
            )
            .child(
                Button::new(("shortcut-reset", row.key))
                    .small()
                    .ghost()
                    .icon(if is_new {
                        IconName::X
                    } else {
                        IconName::RefreshCcw
                    })
                    .tooltip(reset_tooltip)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.reset_row(row_key, window, cx);
                    })),
            )
            .when(!is_new, |this| {
                this.child(
                    Button::new(("shortcut-delete", row.key))
                        .small()
                        .danger()
                        .icon(IconName::Trash)
                        .tooltip(delete_tooltip)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_delete_row(row_key, window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn render_shortcut_row(
        &self,
        row: &ShortcutBindingRowState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (
            field_template,
            field_model,
            field_mode,
            field_hotkey,
            field_send_content,
            field_preset,
            field_actions,
            empty_model_picker,
            invalid_hotkey_label,
        ) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-template"),
                i18n.t("field-model"),
                i18n.t("field-mode"),
                i18n.t("field-hotkey"),
                i18n.t("field-send-content"),
                i18n.t("field-preset"),
                i18n.t("field-actions"),
                i18n.t("empty-model-picker"),
                i18n.t("notify-invalid-shortcut-hotkey"),
            )
        };
        v_flex()
            .id(("shortcut-row-card", row.key))
            .w_full()
            .gap_3()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        self.render_row_field(
                            field_template.clone().into(),
                            px(180.),
                            Select::new(&row.template_select)
                                .small()
                                .placeholder(field_template.clone())
                                .w_full(),
                            cx,
                        ),
                    )
                    .child(
                        self.render_row_field(
                            field_model.clone().into(),
                            px(220.),
                            Select::new(&row.model_select)
                                .small()
                                .placeholder(field_model.clone())
                                .empty(Label::new(empty_model_picker).text_sm())
                                .w_full(),
                            cx,
                        ),
                    )
                    .child(
                        self.render_row_field(
                            field_mode.clone().into(),
                            px(140.),
                            Select::new(&row.mode_select)
                                .small()
                                .placeholder(field_mode.clone())
                                .w_full(),
                            cx,
                        ),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        self.render_row_field(
                            field_send_content.clone().into(),
                            px(300.),
                            Select::new(&row.input_source_select)
                                .small()
                                .placeholder(field_send_content.clone())
                                .menu_width(px(320.))
                                .w_full(),
                            cx,
                        ),
                    )
                    .child(self.render_row_field(
                        field_hotkey.into(),
                        px(156.),
                        v_flex().gap_1().child(row.hotkey_input.clone()).when_some(
                            row.invalid_hotkey.as_ref(),
                            |this, hotkey| {
                                this.child(
                                    Label::new(format!("{}: {}", invalid_hotkey_label, hotkey))
                                        .text_xs()
                                        .text_color(cx.theme().danger),
                                )
                            },
                        ),
                        cx,
                    ))
                    .child(self.render_enabled_field(row, cx))
                    .child(self.render_row_field(
                        field_preset.into(),
                        px(92.),
                        self.render_ext_settings_cell(row, window, cx),
                        cx,
                    ))
                    .child(self.render_row_field(
                        field_actions.into(),
                        px(112.),
                        self.render_shortcut_actions(row, cx),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn save_payload(&self, row_key: u64, cx: &App) -> Result<ShortcutSaveData, SharedString> {
        let row = self
            .rows
            .iter()
            .find(|row| row.key == row_key)
            .ok_or_else(|| SharedString::from("missing row"))?;

        if row.invalid_hotkey.is_some() {
            return Err(cx
                .global::<I18n>()
                .t("notify-invalid-shortcut-hotkey")
                .into());
        }
        let hotkey = row
            .hotkey
            .clone()
            .filter(|hotkey| !hotkey.trim().is_empty())
            .ok_or_else(|| {
                SharedString::from(cx.global::<I18n>().t("notify-invalid-shortcut-hotkey"))
            })?;

        let model_value = row
            .model_select
            .read(cx)
            .selected_value()
            .cloned()
            .filter(|model| !model.is_empty())
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-model")))?;
        let (provider_name, model_id) = Self::split_model_choice_key(&model_value)
            .map(|(provider_name, model_id)| (provider_name.to_string(), model_id.to_string()))
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-model")))?;
        let mode = row
            .mode_select
            .read(cx)
            .selected_value()
            .copied()
            .ok_or_else(|| SharedString::from(cx.global::<I18n>().t("notify-select-mode")))?;

        Ok(ShortcutSaveData {
            row_key,
            binding_id: row.binding_id,
            hotkey,
            enabled: row.enabled,
            template_id: row
                .template_select
                .read(cx)
                .selected_value()
                .cloned()
                .flatten(),
            provider_name,
            model_id,
            mode,
            request_template: row.request_template.clone(),
            input_source: row.input_source,
        })
    }

    fn save_row(&mut self, row_key: u64, window: &mut Window, cx: &mut Context<Self>) {
        let payload = match self.save_payload(row_key, cx) {
            Ok(payload) => payload,
            Err(message) => {
                self.notify_error("notify-save-shortcut-failed", message, window, cx);
                return;
            }
        };

        let result = GlobalHotkeyState::save_global_shortcut_binding(
            payload.binding_id,
            NewGlobalShortcutBinding {
                hotkey: payload.hotkey.clone(),
                enabled: payload.enabled,
                template_id: payload.template_id,
                provider_name: payload.provider_name.clone(),
                model_id: payload.model_id.clone(),
                mode: payload.mode,
                request_template: payload.request_template.clone(),
                input_source: payload.input_source,
            },
            cx,
        );

        let saved = match result {
            Ok(saved) => saved,
            Err(err) => {
                self.notify_error("notify-save-shortcut-failed", err.to_string(), window, cx);
                return;
            }
        };

        let Some(index) = self.rows.iter().position(|row| row.key == payload.row_key) else {
            return;
        };
        self.rows[index] = self.build_row(Some(saved.clone()), window, cx);
        self.refresh_view(cx);
        self.notify_success(
            if payload.binding_id.is_some() {
                "notify-shortcut-updated-success"
            } else {
                "notify-shortcut-created-success"
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn reset_row(&mut self, row_key: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.rows.iter().position(|row| row.key == row_key) else {
            return;
        };
        if let Some(saved) = self.rows[index].saved_binding.clone() {
            self.rows[index] = self.build_row(Some(saved), window, cx);
        } else {
            self.rows.remove(index);
        }
        self.refresh_view(cx);
        cx.notify();
    }

    fn confirm_delete_row(&mut self, row_key: u64, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.global::<I18n>().t("dialog-delete-shortcut-title");
        let message = cx.global::<I18n>().t("dialog-delete-shortcut-message");
        let page = cx.entity().downgrade();
        open_delete_confirm_dialog(
            title,
            message,
            move |window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.delete_row(row_key, window, cx);
                });
            },
            window,
            cx,
        );
    }

    fn delete_row(&mut self, row_key: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.rows.iter().position(|row| row.key == row_key) else {
            return;
        };
        let Some(binding_id) = self.rows[index].binding_id else {
            self.rows.remove(index);
            self.refresh_view(cx);
            cx.notify();
            return;
        };

        let result = GlobalHotkeyState::delete_global_shortcut_binding(binding_id, cx);

        match result {
            Ok(()) => {
                self.rows.remove(index);
                self.refresh_view(cx);
                self.notify_success("notify-shortcut-deleted-success", window, cx);
                cx.notify();
            }
            Err(err) => {
                self.notify_error("notify-delete-shortcut-failed", err.to_string(), window, cx);
            }
        }
    }
}

impl Render for ShortcutSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row_elements = self
            .rows
            .iter()
            .map(|row| self.render_shortcut_row(row, window, cx))
            .collect::<Vec<_>>();
        v_flex()
            .w_full()
            .gap_3()
            .child(self.render_toolbar(cx))
            .child(if row_elements.is_empty() {
                div()
                    .w_full()
                    .min_h(px(280.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(cx.global::<I18n>().t("empty-shortcut-bindings"))
                    .into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(row_elements)
                    .into_any_element()
            })
    }
}
