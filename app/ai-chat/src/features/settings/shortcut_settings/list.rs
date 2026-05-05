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
            validation::validate_hotkey,
        },
    },
    foundation::{assets::IconName, i18n::I18n, search::field_matches_query},
    llm::{ExtSettingControl, ProviderModel, preset_ext_settings},
    state::ModelStore,
};
use gpui::{AppContext as _, StatefulInteractiveElement as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    scroll::{Scrollbar, ScrollbarShow},
    switch::Switch,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use std::{ops::Deref, rc::Rc};

#[derive(Clone, Copy, Debug)]
struct ShortcutColumnSpec {
    width: f32,
}

const SHORTCUT_TABLE_TEMPLATE_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 220. };
const SHORTCUT_TABLE_HOTKEY_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 120. };
const SHORTCUT_TABLE_INPUT_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 150. };
const SHORTCUT_TABLE_MODEL_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 240. };
const SHORTCUT_TABLE_MODE_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 120. };
const SHORTCUT_TABLE_STATUS_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 160. };
const SHORTCUT_TABLE_ENABLED_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 100. };
const SHORTCUT_TABLE_ACTIONS_COLUMN: ShortcutColumnSpec = ShortcutColumnSpec { width: 176. };
const SHORTCUT_TABLE_MIN_CELL_WIDTH: f32 = 100.;
const SHORTCUT_TABLE_HEADER_HEIGHT: f32 = 38.;
const SHORTCUT_TABLE_ROW_HEIGHT: f32 = 74.;
const SHORTCUT_TABLE_SCROLLBAR_HEIGHT: f32 = 14.;

fn shortcut_table_column_specs() -> [ShortcutColumnSpec; 8] {
    let columns = [
        SHORTCUT_TABLE_TEMPLATE_COLUMN,
        SHORTCUT_TABLE_HOTKEY_COLUMN,
        SHORTCUT_TABLE_INPUT_COLUMN,
        SHORTCUT_TABLE_MODEL_COLUMN,
        SHORTCUT_TABLE_MODE_COLUMN,
        SHORTCUT_TABLE_STATUS_COLUMN,
        SHORTCUT_TABLE_ENABLED_COLUMN,
        SHORTCUT_TABLE_ACTIONS_COLUMN,
    ];
    debug_assert!(
        columns
            .iter()
            .all(|column| column.width >= SHORTCUT_TABLE_MIN_CELL_WIDTH)
    );
    columns
}

fn shortcut_table_width() -> f32 {
    shortcut_table_column_specs()
        .into_iter()
        .map(|column| column.width)
        .sum::<f32>()
}

fn shortcut_table_height(row_count: usize) -> f32 {
    SHORTCUT_TABLE_HEADER_HEIGHT
        + row_count as f32 * SHORTCUT_TABLE_ROW_HEIGHT
        + SHORTCUT_TABLE_SCROLLBAR_HEIGHT
}

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
    table_scroll_handle: ScrollHandle,
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
            table_scroll_handle: ScrollHandle::new(),
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
        self.all_items(cx)
            .into_iter()
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
                    crate::state::chat::reload_models(cx);
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
            .child(
                Input::new(&self.search_input)
                    .flex_1()
                    .min_w_0()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .cleanable(true),
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

        let table_height = shortcut_table_height(items.len());

        div()
            .w_full()
            .min_w_0()
            .max_w(relative(1.))
            .h(px(table_height))
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .relative()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .pb(px(SHORTCUT_TABLE_SCROLLBAR_HEIGHT))
                    .child(
                        div()
                            .id("shortcut-table-scroll")
                            .size_full()
                            .track_scroll(&self.table_scroll_handle)
                            .overflow_x_scroll()
                            .child(
                                div()
                                    .w(px(shortcut_table_width()))
                                    .min_w(px(shortcut_table_width()))
                                    .flex_shrink_0()
                                    .child(self.render_table(&items, window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(px(SHORTCUT_TABLE_SCROLLBAR_HEIGHT))
                            .child(
                                Scrollbar::horizontal(&self.table_scroll_handle)
                                    .scrollbar_show(ScrollbarShow::Always),
                            ),
                    ),
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

    fn render_table(
        &self,
        items: &[ShortcutListItem],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Table::new()
            .small()
            .w(px(shortcut_table_width()))
            .child(self.render_table_header(cx))
            .child(
                TableBody::new().children(
                    items
                        .iter()
                        .map(|item| self.render_item_row(item, window, cx)),
                ),
            )
            .into_any_element()
    }

    fn render_table_header(&self, cx: &mut Context<Self>) -> TableHeader {
        let i18n = cx.global::<I18n>();
        TableHeader::new().child(
            TableRow::new()
                .h(px(SHORTCUT_TABLE_HEADER_HEIGHT))
                .child(
                    self.render_table_head(
                        i18n.t("field-template"),
                        SHORTCUT_TABLE_TEMPLATE_COLUMN,
                    ),
                )
                .child(self.render_table_head(i18n.t("field-hotkey"), SHORTCUT_TABLE_HOTKEY_COLUMN))
                .child(
                    self.render_table_head(
                        i18n.t("field-send-content"),
                        SHORTCUT_TABLE_INPUT_COLUMN,
                    ),
                )
                .child(self.render_table_head(i18n.t("field-model"), SHORTCUT_TABLE_MODEL_COLUMN))
                .child(self.render_table_head(i18n.t("field-mode"), SHORTCUT_TABLE_MODE_COLUMN))
                .child(self.render_table_head(i18n.t("field-status"), SHORTCUT_TABLE_STATUS_COLUMN))
                .child(
                    self.render_table_head(i18n.t("field-enabled"), SHORTCUT_TABLE_ENABLED_COLUMN),
                )
                .child(
                    self.render_table_head(i18n.t("field-actions"), SHORTCUT_TABLE_ACTIONS_COLUMN),
                ),
        )
    }

    fn render_table_head(&self, label: String, column: ShortcutColumnSpec) -> TableHead {
        TableHead::new()
            .w(px(column.width))
            .child(Label::new(label).text_xs().truncate())
    }

    fn render_item_row(
        &self,
        item: &ShortcutListItem,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TableRow {
        let binding_id = item.binding.id;
        TableRow::new()
            .min_h(px(SHORTCUT_TABLE_ROW_HEIGHT))
            .child(self.render_table_cell(
                SHORTCUT_TABLE_TEMPLATE_COLUMN,
                self.render_template_cell(item, cx),
            ))
            .child(self.render_table_cell(
                SHORTCUT_TABLE_HOTKEY_COLUMN,
                Label::new(item.hotkey_label.clone()).text_sm().truncate(),
            ))
            .child(self.render_table_cell(
                SHORTCUT_TABLE_INPUT_COLUMN,
                Label::new(item.input_label.clone()).text_sm().truncate(),
            ))
            .child(
                self.render_table_cell(
                    SHORTCUT_TABLE_MODEL_COLUMN,
                    v_flex()
                        .w_full()
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
                ),
            )
            .child(self.render_table_cell(
                SHORTCUT_TABLE_MODE_COLUMN,
                Label::new(item.mode_label.clone()).text_sm().truncate(),
            ))
            .child(self.render_table_cell(
                SHORTCUT_TABLE_STATUS_COLUMN,
                self.render_status_badge(item, cx),
            ))
            .child(
                self.render_table_cell(
                    SHORTCUT_TABLE_ENABLED_COLUMN,
                    h_flex().w_full().justify_center().child(
                        Switch::new(("shortcut-row-enabled", binding_id as u64))
                            .checked(item.binding.enabled)
                            .small()
                            .on_click(cx.listener(move |page, checked, window, cx| {
                                page.toggle_enabled(binding_id, *checked, window, cx);
                            })),
                    ),
                ),
            )
            .child(self.render_table_cell(
                SHORTCUT_TABLE_ACTIONS_COLUMN,
                self.render_actions(binding_id, cx),
            ))
    }

    fn render_table_cell(&self, column: ShortcutColumnSpec, child: impl IntoElement) -> TableCell {
        TableCell::new().w(px(column.width)).child(child)
    }

    fn render_template_cell(&self, item: &ShortcutListItem, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
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
            .w_full()
            .min_w_0()
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
            .w_full()
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
        SHORTCUT_TABLE_MIN_CELL_WIDTH, ShortcutSearchParts, shortcut_search_text,
        shortcut_table_column_specs, shortcut_table_height, shortcut_table_width,
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
    fn shortcut_table_columns_fit_component_cell_min_width() {
        let [
            template,
            hotkey,
            input,
            model,
            mode,
            status,
            enabled,
            actions,
        ] = shortcut_table_column_specs();

        for column in [
            template, hotkey, input, model, mode, status, enabled, actions,
        ] {
            assert!(column.width >= SHORTCUT_TABLE_MIN_CELL_WIDTH);
        }
        assert!(hotkey.width < template.width);
        assert!(enabled.width < hotkey.width);
        assert!(actions.width < model.width);
        assert!(input.width < model.width);
        assert_eq!(
            shortcut_table_width(),
            shortcut_table_column_specs()
                .into_iter()
                .map(|column| column.width)
                .sum::<f32>()
        );
    }

    #[test]
    fn shortcut_table_height_includes_header_rows_and_scrollbar() {
        assert!(shortcut_table_height(2) > shortcut_table_height(1));
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
        assert!(search_text.contains("单轮模式"));
        assert!(search_text.contains("已启用"));
        assert!(search_text.contains("联网搜索"));
    }
}
