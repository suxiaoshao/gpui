use crate::{
    components::hotkey_input::format_hotkey_label,
    database::{
        ConversationTemplate, Db, GlobalShortcutBinding, Mode, NewGlobalShortcutBinding,
        ShortcutInputSource,
    },
    features::{
        hotkey::{GlobalHotkeyState, ShortcutRuntimeDiagnostics},
        screenshot,
        settings::shortcut_settings::{
            dialogs::{
                ShortcutStatusActions, ShortcutStatusDetails, ShortcutSummary,
                open_delete_shortcut_dialog, open_shortcut_status_dialog,
            },
            form::{open_add_shortcut_dialog, open_edit_shortcut_dialog},
            segmented::single_selected_index,
            validation::validate_hotkey,
        },
    },
    foundation::{assets::IconName, i18n::I18n, search::field_matches_query},
    llm::{ExtSettingControl, ProviderModel, preset_ext_settings},
    state::ModelStore,
};
use gpui::{AppContext as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use std::{ops::Deref, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShortcutStatus {
    Enabled,
    Disabled,
    ModelUnavailable,
    HotkeyInvalid,
    HotkeyConflict,
    RegistrationFailed,
}

impl ShortcutStatus {
    fn requires_action(self) -> bool {
        matches!(
            self,
            Self::ModelUnavailable
                | Self::HotkeyInvalid
                | Self::HotkeyConflict
                | Self::RegistrationFailed
        )
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Enabled => "shortcut-status-enabled",
            Self::Disabled => "shortcut-status-disabled",
            Self::ModelUnavailable => "shortcut-status-model-unavailable",
            Self::HotkeyInvalid => "shortcut-status-hotkey-invalid",
            Self::HotkeyConflict => "shortcut-status-hotkey-conflict",
            Self::RegistrationFailed => "shortcut-status-registration-failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShortcutStatusFilter {
    All,
    Enabled,
    Disabled,
    NeedsAction,
}

impl ShortcutStatusFilter {
    fn id(self) -> u64 {
        match self {
            Self::All => 0,
            Self::Enabled => 1,
            Self::Disabled => 2,
            Self::NeedsAction => 3,
        }
    }

    fn matches(self, status: ShortcutStatus) -> bool {
        match self {
            Self::All => true,
            Self::Enabled => status == ShortcutStatus::Enabled,
            Self::Disabled => status == ShortcutStatus::Disabled,
            Self::NeedsAction => status.requires_action(),
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::All => "shortcut-filter-all",
            Self::Enabled => "shortcut-filter-enabled",
            Self::Disabled => "shortcut-filter-disabled",
            Self::NeedsAction => "shortcut-filter-needs-action",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModeFilter {
    All,
    Mode(Mode),
}

impl ModeFilter {
    fn index(self) -> usize {
        mode_filter_options()
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(0)
    }

    fn matches(self, mode: Mode) -> bool {
        match self {
            Self::All => true,
            Self::Mode(filter_mode) => filter_mode == mode,
        }
    }

    fn label(self, cx: &App) -> String {
        match self {
            Self::All => cx.global::<I18n>().t("shortcut-filter-all-modes"),
            Self::Mode(mode) => mode_label(mode, cx),
        }
    }
}

struct ShortcutListItem {
    binding: GlobalShortcutBinding,
    title: String,
    subtitle: Option<String>,
    icon: String,
    hotkey_label: String,
    input_label: String,
    mode_label: String,
    preset_summary: String,
    status: ShortcutStatus,
    status_message: SharedString,
    search_text: String,
    model_resolved: bool,
    registration: SharedString,
}

struct ShortcutSearchParts<'a> {
    binding: &'a GlobalShortcutBinding,
    template: Option<&'a ConversationTemplate>,
    title: &'a str,
    subtitle: Option<&'a str>,
    hotkey_label: &'a str,
    input_label: &'a str,
    mode_label: &'a str,
    preset_summary: &'a str,
    status_label: &'a str,
}

pub(crate) struct ShortcutSettingsPage {
    search_input: Entity<InputState>,
    mode_filter: ModeFilter,
    status_filter: ShortcutStatusFilter,
    templates: Vec<ConversationTemplate>,
    bindings: Result<Vec<GlobalShortcutBinding>, String>,
    _subscriptions: Vec<Subscription>,
}

impl ShortcutSettingsPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("shortcut-search-placeholder"))
        });
        let model_store = cx.global::<ModelStore>().deref().clone();
        let model_subscription = cx.observe_in(&model_store, window, |this, _, _window, cx| {
            cx.notify();
            this.reload_from_database(cx);
        });

        let mut this = Self {
            search_input,
            mode_filter: ModeFilter::All,
            status_filter: ShortcutStatusFilter::All,
            templates: Vec::new(),
            bindings: Ok(Vec::new()),
            _subscriptions: Vec::new(),
        };
        this._subscriptions.push(cx.subscribe_in(
            &this.search_input,
            window,
            Self::on_search_input_event,
        ));
        this._subscriptions.push(model_subscription);
        this.reload_from_database(cx);
        this
    }

    fn on_search_input_event(
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

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match Self::load_data(cx) {
            Ok((templates, bindings)) => {
                self.templates = templates;
                self.bindings = Ok(bindings);
            }
            Err(err) => {
                self.bindings = Err(err.to_string());
                notify_error(
                    cx.global::<I18n>().t("notify-load-shortcuts-failed"),
                    err.to_string(),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn reload_from_database(&mut self, cx: &mut Context<Self>) {
        match Self::load_data(cx) {
            Ok((templates, bindings)) => {
                self.templates = templates;
                self.bindings = Ok(bindings);
            }
            Err(err) => {
                self.bindings = Err(err.to_string());
            }
        }
        cx.notify();
    }

    fn load_data(
        cx: &mut Context<Self>,
    ) -> anyhow::Result<(Vec<ConversationTemplate>, Vec<GlobalShortcutBinding>)> {
        let mut conn = cx.global::<Db>().get()?;
        Ok((
            ConversationTemplate::all(&mut conn)?,
            GlobalShortcutBinding::all(&mut conn)?,
        ))
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn current_mode_filter(&self) -> ModeFilter {
        self.mode_filter
    }

    fn available_models(&self, cx: &App) -> Vec<ProviderModel> {
        cx.global::<ModelStore>().read(cx).snapshot().models
    }

    fn all_items(&self, cx: &App) -> Vec<ShortcutListItem> {
        let models = self.available_models(cx);
        let diagnostics = GlobalHotkeyState::diagnostics_snapshot(cx);
        self.bindings
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|binding| self.build_item(binding, &models, &diagnostics, cx))
            .collect()
    }

    fn filtered_items(&self, cx: &App) -> Vec<ShortcutListItem> {
        let query = self.current_query(cx);
        let mode_filter = self.current_mode_filter();
        self.all_items(cx)
            .into_iter()
            .filter(|item| self.status_filter.matches(item.status))
            .filter(|item| mode_filter.matches(item.binding.mode))
            .filter(|item| query.is_empty() || field_matches_query(&item.search_text, &query))
            .collect()
    }

    fn build_item(
        &self,
        binding: &GlobalShortcutBinding,
        models: &[ProviderModel],
        diagnostics: &ShortcutRuntimeDiagnostics,
        cx: &App,
    ) -> ShortcutListItem {
        let template = binding
            .template_id
            .and_then(|id| self.templates.iter().find(|template| template.id == id))
            .cloned();
        let i18n = cx.global::<I18n>();
        let title = template
            .as_ref()
            .map(|template| template.name.clone())
            .unwrap_or_else(|| format!("{} {}", i18n.t("field-id"), binding.id));
        let subtitle = template
            .as_ref()
            .and_then(|template| template.description.clone())
            .or_else(|| Some(format!("{}: {}", i18n.t("field-id"), binding.id)));
        let icon = template
            .as_ref()
            .map(|template| template.icon.clone())
            .unwrap_or_else(|| "⌘".to_string());
        let hotkey_label = format_hotkey_label(&binding.hotkey);
        let input_label = input_source_label(binding.input_source, cx);
        let mode_label = mode_label(binding.mode, cx);
        let model_resolved = models.iter().any(|model| {
            model.provider_name == binding.provider_name && model.id == binding.model_id
        });
        let preset_summary = preset_summary(binding, models, cx);
        let (status, status_message) = resolve_status(
            binding,
            models,
            diagnostics,
            &self.bindings_for_status(),
            cx,
        );
        let registration = registration_label(binding, diagnostics, cx);
        let status_label = i18n.t(status.label_key());
        let search_text = shortcut_search_text(ShortcutSearchParts {
            binding,
            template: template.as_ref(),
            title: &title,
            subtitle: subtitle.as_deref(),
            hotkey_label: &hotkey_label,
            input_label: &input_label,
            mode_label: &mode_label,
            preset_summary: &preset_summary,
            status_label: &status_label,
        });

        ShortcutListItem {
            binding: binding.clone(),
            title,
            subtitle,
            icon,
            hotkey_label,
            input_label,
            mode_label,
            preset_summary,
            status,
            status_message,
            search_text,
            model_resolved,
            registration,
        }
    }

    fn bindings_for_status(&self) -> Vec<GlobalShortcutBinding> {
        self.bindings.as_ref().cloned().unwrap_or_default()
    }

    fn status_counts(&self, cx: &App) -> StatusCounts {
        StatusCounts::from_items(self.all_items(cx).iter().map(|item| item.status))
    }

    fn open_add_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        open_add_shortcut_dialog(
            self.templates.clone(),
            self.bindings.as_ref().cloned().unwrap_or_default(),
            Rc::new(move |window, cx| {
                let _ = page.update(cx, |page, cx| page.reload(window, cx));
            }),
            window,
            cx,
        );
    }

    fn open_edit_dialog(&mut self, binding_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(binding) = self.find_binding(binding_id).cloned() else {
            return;
        };
        let page = cx.entity().downgrade();
        open_edit_shortcut_dialog(
            binding,
            self.templates.clone(),
            self.bindings.as_ref().cloned().unwrap_or_default(),
            Rc::new(move |window, cx| {
                let _ = page.update(cx, |page, cx| page.reload(window, cx));
            }),
            window,
            cx,
        );
    }

    fn open_delete_dialog(&mut self, binding_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self
            .all_items(cx)
            .into_iter()
            .find(|item| item.binding.id == binding_id)
        else {
            return;
        };
        let page = cx.entity().downgrade();
        open_delete_shortcut_dialog(
            item.binding.clone(),
            item.summary(),
            Rc::new(move |binding_id, window, cx| {
                page.update(cx, |page, cx| page.delete_binding(binding_id, window, cx))
                    .unwrap_or(false)
            }),
            window,
            cx,
        );
    }

    fn open_status_dialog(&mut self, binding_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self
            .all_items(cx)
            .into_iter()
            .find(|item| item.binding.id == binding_id)
        else {
            return;
        };
        let page = cx.entity().downgrade();
        let edit_page = page.clone();
        let reload_page = page.clone();
        open_shortcut_status_dialog(
            item.binding.clone(),
            item.summary(),
            item.status_details(cx),
            ShortcutStatusActions {
                on_reload_models: Rc::new(move |window, cx| {
                    let model_store = cx.global::<ModelStore>().deref().clone();
                    model_store.update(cx, |store, cx| store.reload(cx));
                    let _ = reload_page.update(cx, |page, cx| page.reload(window, cx));
                }),
                on_reregister: Rc::new(move |binding_id, window, cx| {
                    let _ = page.update(cx, |page, cx| page.reregister(binding_id, window, cx));
                }),
                on_edit: Rc::new(move |binding_id, window, cx| {
                    let _ = edit_page.update(cx, |page, cx| {
                        page.open_edit_dialog(binding_id, window, cx);
                    });
                }),
            },
            window,
            cx,
        );
    }

    fn find_binding(&self, binding_id: i32) -> Option<&GlobalShortcutBinding> {
        self.bindings
            .as_ref()
            .ok()?
            .iter()
            .find(|binding| binding.id == binding_id)
    }

    fn delete_binding(
        &mut self,
        binding_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match GlobalHotkeyState::delete_global_shortcut_binding(binding_id, cx) {
            Ok(()) => {
                notify_success(
                    cx.global::<I18n>().t("notify-shortcut-deleted-success"),
                    window,
                    cx,
                );
                self.reload(window, cx);
                true
            }
            Err(err) => {
                notify_error(
                    cx.global::<I18n>().t("notify-delete-shortcut-failed"),
                    err.to_string(),
                    window,
                    cx,
                );
                false
            }
        }
    }

    fn toggle_enabled(
        &mut self,
        binding_id: i32,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(binding) = self.find_binding(binding_id).cloned() else {
            return;
        };
        let result = GlobalHotkeyState::save_global_shortcut_binding(
            Some(binding.id),
            NewGlobalShortcutBinding {
                hotkey: binding.hotkey,
                enabled,
                template_id: binding.template_id,
                provider_name: binding.provider_name,
                model_id: binding.model_id,
                mode: binding.mode,
                request_template: binding.request_template,
                input_source: binding.input_source,
            },
            cx,
        );
        match result {
            Ok(_) => {
                notify_success(
                    cx.global::<I18n>().t("notify-shortcut-updated-success"),
                    window,
                    cx,
                );
                self.reload(window, cx);
            }
            Err(err) => {
                notify_error(
                    cx.global::<I18n>().t("notify-save-shortcut-failed"),
                    err.to_string(),
                    window,
                    cx,
                );
            }
        }
    }

    fn reregister(&mut self, binding_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        match GlobalHotkeyState::reregister_global_shortcut_binding(binding_id, cx) {
            Ok(()) => {
                notify_success(
                    cx.global::<I18n>()
                        .t("notify-shortcut-reregistered-success"),
                    window,
                    cx,
                );
                self.reload(window, cx);
            }
            Err(err) => {
                notify_error(
                    cx.global::<I18n>().t("notify-shortcut-reregister-failed"),
                    err.to_string(),
                    window,
                    cx,
                );
            }
        }
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (reload_label, add_label) = {
            let i18n = cx.global::<I18n>();
            (i18n.t("button-reload"), i18n.t("dialog-add-shortcut-title"))
        };
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_2()
            .flex_wrap()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .flex_wrap()
                    .child(
                        Input::new(&self.search_input)
                            .w(px(420.))
                            .min_w(px(320.))
                            .max_w(px(420.))
                            .prefix(
                                Icon::new(IconName::Search).text_color(cx.theme().muted_foreground),
                            )
                            .cleanable(true),
                    )
                    .child(self.render_mode_filter(cx)),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        Button::new("shortcut-settings-reload")
                            .icon(IconName::RefreshCcw)
                            .ghost()
                            .tooltip(reload_label)
                            .on_click(cx.listener(|page, _, window, cx| page.reload(window, cx))),
                    )
                    .child(
                        Button::new("shortcut-settings-add")
                            .icon(IconName::Plus)
                            .label(add_label)
                            .primary()
                            .on_click(
                                cx.listener(|page, _, window, cx| page.open_add_dialog(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_mode_filter(&self, cx: &mut Context<Self>) -> AnyElement {
        let current_index = self.mode_filter.index();
        ToggleGroup::new("shortcut-mode-filter")
            .segmented()
            .outline()
            .small()
            .children(
                mode_filter_options()
                    .into_iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        Toggle::new(("shortcut-mode-filter-item", index as u64))
                            .label(filter.label(cx))
                            .checked(index == current_index)
                            .min_w(px(if matches!(filter, ModeFilter::All) {
                                88.
                            } else {
                                104.
                            }))
                    }),
            )
            .on_click(cx.listener(move |page, checkeds: &Vec<bool>, _window, cx| {
                let next_index = single_selected_index(page.mode_filter.index(), checkeds);
                if let Some(filter) = mode_filter_options().get(next_index).copied() {
                    page.mode_filter = filter;
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    fn render_status_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let counts = self.status_counts(cx);
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .children(
                [
                    (ShortcutStatusFilter::All, counts.total),
                    (ShortcutStatusFilter::Enabled, counts.enabled),
                    (ShortcutStatusFilter::Disabled, counts.disabled),
                    (ShortcutStatusFilter::NeedsAction, counts.needs_action),
                ]
                .into_iter()
                .map(|(filter, count)| self.render_status_filter(filter, count, cx)),
            )
            .into_any_element()
    }

    fn render_status_filter(
        &self,
        filter: ShortcutStatusFilter,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = cx.global::<I18n>().t(filter.label_key());
        Button::new(("shortcut-status-filter", filter.id()))
            .small()
            .when(self.status_filter == filter, |button| button.primary())
            .when(self.status_filter != filter, |button| button.ghost())
            .label(format!("{label}  {count}"))
            .on_click(cx.listener(move |page, _, _window, cx| {
                page.status_filter = filter;
                cx.notify();
            }))
            .into_any_element()
    }

    fn render_list(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(bindings) = self.bindings.as_ref().ok() else {
            return self.render_error_state(window, cx);
        };
        let items = self.filtered_items(cx);
        if bindings.is_empty() {
            return self.render_empty_state(
                cx.global::<I18n>().t("empty-shortcut-bindings"),
                true,
                cx,
            );
        }
        if items.is_empty() {
            return self.render_empty_state(
                cx.global::<I18n>().t("shortcut-empty-search"),
                false,
                cx,
            );
        }

        v_flex()
            .w_full()
            .min_w_0()
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(self.render_table_header(cx))
            .children(
                items
                    .iter()
                    .map(|item| self.render_item_row(item, window, cx)),
            )
            .into_any_element()
    }

    fn render_error_state(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let error = self.bindings.as_ref().err().cloned().unwrap_or_default();
        v_flex()
            .w_full()
            .min_h(px(280.))
            .items_center()
            .justify_center()
            .gap_3()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(error).text_center())
            .child(
                Button::new("shortcut-reload-after-error")
                    .icon(IconName::RefreshCcw)
                    .label(cx.global::<I18n>().t("button-reload"))
                    .on_click(cx.listener(|page, _, window, cx| page.reload(window, cx))),
            )
            .into_any_element()
    }

    fn render_empty_state(
        &self,
        message: String,
        show_add: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .min_h(px(280.))
            .items_center()
            .justify_center()
            .gap_3()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(message).text_center())
            .when(show_add, |this| {
                this.child(
                    Button::new("shortcut-empty-add")
                        .icon(IconName::Plus)
                        .label(cx.global::<I18n>().t("dialog-add-shortcut-title"))
                        .primary()
                        .on_click(cx.listener(|page, _, window, cx| {
                            page.open_add_dialog(window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn render_table_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .bg(cx.theme().secondary.opacity(0.45))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(i18n.t("field-template")).w(px(200.)))
            .child(Label::new(i18n.t("field-hotkey")).w(px(104.)))
            .child(Label::new(i18n.t("field-send-content")).w(px(140.)))
            .child(Label::new(i18n.t("field-model")).flex_1())
            .child(Label::new(i18n.t("field-mode")).w(px(116.)))
            .child(Label::new(i18n.t("field-status")).w(px(132.)))
            .child(Label::new(i18n.t("field-enabled")).w(px(60.)))
            .child(Label::new(i18n.t("field-actions")).w(px(132.)))
            .into_any_element()
    }

    fn render_item_row(
        &self,
        item: &ShortcutListItem,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let binding_id = item.binding.id;
        h_flex()
            .id(("shortcut-row", binding_id as u64))
            .w_full()
            .min_w_0()
            .items_center()
            .gap_3()
            .px_3()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().secondary_hover))
            .child(self.render_template_cell(item, cx))
            .child(
                Label::new(item.hotkey_label.clone())
                    .w(px(104.))
                    .text_sm()
                    .truncate(),
            )
            .child(
                Label::new(item.input_label.clone())
                    .w(px(140.))
                    .text_sm()
                    .truncate(),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(item.binding.model_id.clone())
                            .text_sm()
                            .truncate(),
                    )
                    .child(
                        Label::new(item.binding.provider_name.clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .child(
                Label::new(item.mode_label.clone())
                    .w(px(116.))
                    .text_sm()
                    .truncate(),
            )
            .child(self.render_status_badge(item, cx))
            .child(
                Checkbox::new(("shortcut-row-enabled", binding_id as u64))
                    .checked(item.binding.enabled)
                    .on_click(cx.listener(move |page, checked, window, cx| {
                        page.toggle_enabled(binding_id, *checked, window, cx);
                    }))
                    .w(px(60.)),
            )
            .child(self.render_actions(binding_id, cx))
            .into_any_element()
    }

    fn render_template_cell(&self, item: &ShortcutListItem, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w(px(200.))
            .min_w_0()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .bg(cx.theme().border.opacity(0.35))
                    .child(Label::new(item.icon.clone()).text_base()),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(item.title.clone())
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .when_some(item.subtitle.clone(), |this, subtitle| {
                        this.child(
                            Label::new(subtitle)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate(),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_status_badge(&self, item: &ShortcutListItem, cx: &mut Context<Self>) -> AnyElement {
        let (fg, bg) = status_colors(item.status, cx);
        h_flex()
            .w(px(132.))
            .items_center()
            .gap_1()
            .child(
                div()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(fg.opacity(0.42))
                    .bg(bg)
                    .px_2()
                    .py_1()
                    .child(
                        Label::new(cx.global::<I18n>().t(item.status.label_key()))
                            .text_xs()
                            .text_color(fg),
                    ),
            )
            .when(item.status.requires_action(), |this| {
                this.child(
                    Button::new(("shortcut-status-detail", item.binding.id as u64))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Info)
                        .tooltip(cx.global::<I18n>().t("tooltip-view-detail"))
                        .on_click({
                            let binding_id = item.binding.id;
                            cx.listener(move |page, _, window, cx| {
                                page.open_status_dialog(binding_id, window, cx);
                            })
                        }),
                )
            })
            .into_any_element()
    }

    fn render_actions(&self, binding_id: i32, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w(px(132.))
            .items_center()
            .gap_1()
            .child(
                Button::new(("shortcut-view-status", binding_id as u64))
                    .ghost()
                    .small()
                    .icon(IconName::Eye)
                    .tooltip(cx.global::<I18n>().t("shortcut-status-dialog-title"))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.open_status_dialog(binding_id, window, cx);
                    })),
            )
            .child(
                Button::new(("shortcut-edit", binding_id as u64))
                    .ghost()
                    .small()
                    .icon(IconName::Edit)
                    .tooltip(cx.global::<I18n>().t("button-edit"))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.open_edit_dialog(binding_id, window, cx);
                    })),
            )
            .child(
                Button::new(("shortcut-reregister", binding_id as u64))
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCcw)
                    .tooltip(cx.global::<I18n>().t("shortcut-action-reregister"))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.reregister(binding_id, window, cx);
                    })),
            )
            .child(
                Button::new(("shortcut-delete", binding_id as u64))
                    .danger()
                    .small()
                    .icon(IconName::Trash)
                    .tooltip(cx.global::<I18n>().t("button-delete"))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.open_delete_dialog(binding_id, window, cx);
                    })),
            )
            .into_any_element()
    }
}

impl Render for ShortcutSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .min_w_0()
            .gap_4()
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(Label::new(cx.global::<I18n>().t("settings-group-shortcuts")).text_lg())
                    .child(
                        Label::new(cx.global::<I18n>().t("shortcut-settings-description"))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(self.render_toolbar(window, cx))
            .child(self.render_status_filters(cx))
            .child(self.render_list(window, cx))
    }
}

impl ShortcutListItem {
    fn summary(&self) -> ShortcutSummary {
        ShortcutSummary {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            icon: self.icon.clone(),
            hotkey: self.hotkey_label.clone(),
            input_source: self.input_label.clone(),
        }
    }

    fn status_details(&self, cx: &App) -> ShortcutStatusDetails {
        ShortcutStatusDetails {
            status_label: cx.global::<I18n>().t(self.status.label_key()).into(),
            status_message: self.status_message.clone(),
            provider_name: self.binding.provider_name.clone(),
            model_id: self.binding.model_id.clone(),
            registration: self.registration.clone(),
            input_source: self.input_label.clone().into(),
            runtime_state: if screenshot::overlay::is_active(cx) {
                cx.global::<I18n>()
                    .t("shortcut-runtime-screenshot-active")
                    .into()
            } else {
                cx.global::<I18n>().t("shortcut-runtime-waiting").into()
            },
            preset_state: if self.model_resolved {
                self.preset_summary.clone().into()
            } else {
                cx.global::<I18n>()
                    .t("shortcut-ext-settings-unavailable")
                    .into()
            },
        }
    }
}

#[derive(Default)]
struct StatusCounts {
    total: usize,
    enabled: usize,
    disabled: usize,
    needs_action: usize,
}

impl StatusCounts {
    fn from_items(statuses: impl IntoIterator<Item = ShortcutStatus>) -> Self {
        let mut counts = Self::default();
        for status in statuses {
            counts.total += 1;
            match status {
                ShortcutStatus::Enabled => counts.enabled += 1,
                ShortcutStatus::Disabled => counts.disabled += 1,
                _ if status.requires_action() => counts.needs_action += 1,
                _ => {}
            }
        }
        counts
    }
}

fn mode_filter_options() -> [ModeFilter; 4] {
    [
        ModeFilter::All,
        ModeFilter::Mode(Mode::Contextual),
        ModeFilter::Mode(Mode::Single),
        ModeFilter::Mode(Mode::AssistantOnly),
    ]
}

fn resolve_status(
    binding: &GlobalShortcutBinding,
    models: &[ProviderModel],
    diagnostics: &ShortcutRuntimeDiagnostics,
    existing_bindings: &[GlobalShortcutBinding],
    cx: &App,
) -> (ShortcutStatus, SharedString) {
    if !binding.enabled {
        return (
            ShortcutStatus::Disabled,
            cx.global::<I18n>()
                .t("shortcut-status-message-disabled")
                .into(),
        );
    }

    let validation = validate_hotkey(
        Some(binding.id),
        Some(&binding.hotkey),
        existing_bindings,
        diagnostics.temporary_hotkey.as_deref(),
    );
    if let Err(err) = validation {
        let status = if err.is_conflict() {
            ShortcutStatus::HotkeyConflict
        } else {
            ShortcutStatus::HotkeyInvalid
        };
        return (status, err.message(cx));
    }

    if let Some(err) = diagnostics.registration_errors.get(&binding.id) {
        return (ShortcutStatus::RegistrationFailed, err.clone().into());
    }
    if !diagnostics.registered_bindings.contains_key(&binding.id) {
        return (
            ShortcutStatus::RegistrationFailed,
            cx.global::<I18n>()
                .t("shortcut-status-message-not-registered")
                .into(),
        );
    }

    let model_available = models
        .iter()
        .any(|model| model.provider_name == binding.provider_name && model.id == binding.model_id);
    if !model_available {
        return (
            ShortcutStatus::ModelUnavailable,
            cx.global::<I18n>()
                .t("shortcut-status-message-model-unavailable")
                .into(),
        );
    }

    (
        ShortcutStatus::Enabled,
        cx.global::<I18n>()
            .t("shortcut-status-message-enabled")
            .into(),
    )
}

fn registration_label(
    binding: &GlobalShortcutBinding,
    diagnostics: &ShortcutRuntimeDiagnostics,
    cx: &App,
) -> SharedString {
    if !binding.enabled {
        return cx
            .global::<I18n>()
            .t("shortcut-registration-disabled")
            .into();
    }
    if let Some(error) = diagnostics.registration_errors.get(&binding.id) {
        return error.clone().into();
    }
    if diagnostics.registered_bindings.contains_key(&binding.id) {
        return cx
            .global::<I18n>()
            .t("shortcut-registration-registered")
            .into();
    }
    cx.global::<I18n>()
        .t("shortcut-registration-not-registered")
        .into()
}

fn status_colors(status: ShortcutStatus, cx: &App) -> (Hsla, Hsla) {
    match status {
        ShortcutStatus::Enabled => (cx.theme().success, cx.theme().success.opacity(0.12)),
        ShortcutStatus::Disabled => (
            cx.theme().muted_foreground,
            cx.theme().muted_foreground.opacity(0.1),
        ),
        ShortcutStatus::ModelUnavailable
        | ShortcutStatus::HotkeyConflict
        | ShortcutStatus::HotkeyInvalid => (cx.theme().warning, cx.theme().warning.opacity(0.12)),
        ShortcutStatus::RegistrationFailed => (cx.theme().danger, cx.theme().danger.opacity(0.12)),
    }
}

fn input_source_label(input_source: ShortcutInputSource, cx: &App) -> String {
    let key = match input_source {
        ShortcutInputSource::SelectionOrClipboard => "send-content-selection-or-clipboard",
        ShortcutInputSource::Screenshot => "send-content-screenshot",
    };
    cx.global::<I18n>().t(key)
}

fn mode_label(mode: Mode, cx: &App) -> String {
    let key = match mode {
        Mode::Contextual => "mode-contextual",
        Mode::Single => "mode-single",
        Mode::AssistantOnly => "mode-assistant-only",
    };
    cx.global::<I18n>().t(key)
}

fn preset_summary(binding: &GlobalShortcutBinding, models: &[ProviderModel], cx: &App) -> String {
    let Some(model) = models
        .iter()
        .find(|model| model.provider_name == binding.provider_name && model.id == binding.model_id)
    else {
        return cx.global::<I18n>().t("shortcut-ext-settings-unavailable");
    };
    let settings = match preset_ext_settings(model, &binding.request_template) {
        Ok(settings) => settings,
        Err(_) => return cx.global::<I18n>().t("shortcut-ext-settings-unavailable"),
    };
    if settings.is_empty() {
        return cx.global::<I18n>().t("field-none");
    }
    settings
        .into_iter()
        .map(|setting| {
            let label = cx.global::<I18n>().t(setting.label_key);
            match setting.control {
                ExtSettingControl::Boolean(value) => {
                    let value = cx
                        .global::<I18n>()
                        .t(if value { "field-on" } else { "field-off" });
                    format!("{label}: {value}")
                }
                ExtSettingControl::Select { value, options } => {
                    let display = options
                        .iter()
                        .find(|option| option.value == value)
                        .map(|option| cx.global::<I18n>().t(option.label_key))
                        .unwrap_or(value.to_string());
                    format!("{label}: {display}")
                }
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn shortcut_search_text(parts: ShortcutSearchParts<'_>) -> String {
    [
        parts.title,
        parts.subtitle.unwrap_or_default(),
        parts
            .template
            .map(|template| template.icon.as_str())
            .unwrap_or_default(),
        parts
            .template
            .and_then(|template| template.description.as_deref())
            .unwrap_or_default(),
        parts.binding.provider_name.as_str(),
        parts.binding.model_id.as_str(),
        parts.binding.hotkey.as_str(),
        parts.hotkey_label,
        parts.input_label,
        parts.mode_label,
        parts.preset_summary,
        parts.status_label,
    ]
    .join(" ")
    .to_lowercase()
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
        ModeFilter, ShortcutSearchParts, ShortcutStatus, ShortcutStatusFilter, StatusCounts,
        shortcut_search_text,
    };
    use crate::database::{GlobalShortcutBinding, Mode, ShortcutInputSource};
    use time::OffsetDateTime;

    fn binding() -> GlobalShortcutBinding {
        let now = OffsetDateTime::now_utc();
        GlobalShortcutBinding {
            id: 12,
            hotkey: "super+shift+k".to_string(),
            enabled: true,
            template_id: None,
            provider_name: "OpenAI".to_string(),
            model_id: "gpt-5.4-mini".to_string(),
            mode: Mode::Single,
            request_template: serde_json::json!({}),
            input_source: ShortcutInputSource::SelectionOrClipboard,
            created_time: now,
            updated_time: now,
        }
    }

    #[test]
    fn status_filter_matches_expected_groups() {
        assert!(ShortcutStatusFilter::All.matches(ShortcutStatus::RegistrationFailed));
        assert!(ShortcutStatusFilter::Enabled.matches(ShortcutStatus::Enabled));
        assert!(!ShortcutStatusFilter::Enabled.matches(ShortcutStatus::ModelUnavailable));
        assert!(ShortcutStatusFilter::NeedsAction.matches(ShortcutStatus::HotkeyConflict));
        assert!(!ShortcutStatusFilter::NeedsAction.matches(ShortcutStatus::Disabled));
    }

    #[test]
    fn status_counts_group_action_required_statuses() {
        let counts = StatusCounts::from_items([
            ShortcutStatus::Enabled,
            ShortcutStatus::Disabled,
            ShortcutStatus::ModelUnavailable,
            ShortcutStatus::RegistrationFailed,
        ]);

        assert_eq!(counts.total, 4);
        assert_eq!(counts.enabled, 1);
        assert_eq!(counts.disabled, 1);
        assert_eq!(counts.needs_action, 2);
    }

    #[test]
    fn mode_filter_matches_expected_modes() {
        assert!(ModeFilter::All.matches(Mode::Contextual));
        assert!(ModeFilter::All.matches(Mode::Single));
        assert!(ModeFilter::All.matches(Mode::AssistantOnly));
        assert!(ModeFilter::Mode(Mode::Contextual).matches(Mode::Contextual));
        assert!(!ModeFilter::Mode(Mode::Contextual).matches(Mode::Single));
        assert!(ModeFilter::Mode(Mode::Single).matches(Mode::Single));
        assert!(ModeFilter::Mode(Mode::AssistantOnly).matches(Mode::AssistantOnly));
    }

    #[test]
    fn shortcut_search_text_contains_core_fields() {
        let binding = binding();
        let search_text = shortcut_search_text(ShortcutSearchParts {
            binding: &binding,
            template: None,
            title: "翻译",
            subtitle: Some("翻译选中文字"),
            hotkey_label: "⌘⇧K",
            input_label: "选中文字 / 剪贴板",
            mode_label: "单轮模式",
            preset_summary: "联网搜索: 开",
            status_label: "已启用",
        });

        assert!(search_text.contains("openai"));
        assert!(search_text.contains("gpt-5.4-mini"));
        assert!(search_text.contains("super+shift+k"));
        assert!(search_text.contains("选中文字"));
        assert!(search_text.contains("联网搜索"));
    }
}
