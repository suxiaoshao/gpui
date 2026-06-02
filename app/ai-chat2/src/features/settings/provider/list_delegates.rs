use ai_chat_core::ModelCapabilitiesSnapshot;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, Sizable, StyledExt, h_flex,
    label::Label,
    list::{ListDelegate, ListEvent, ListState},
    switch::Switch,
    tag::Tag,
    v_flex,
};

use crate::foundation::{I18n, assets::IconName};

use super::{ProviderListItem, catalog::ProviderKindKey, draft::ProviderModelDraft};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderListRow {
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: SharedString,
    pub(super) icon: IconName,
    pub(super) enabled: bool,
    search_text: String,
}

pub(super) struct ProviderListDelegate {
    all_rows: Vec<ProviderListRow>,
    rows: Vec<ProviderListRow>,
    last_query: String,
    empty_label: SharedString,
}

#[derive(IntoElement, Clone)]
pub(super) struct ProviderListEntry {
    row: ProviderListRow,
    selected: bool,
    show_separator: bool,
}

impl ProviderListEntry {
    fn new(row: ProviderListRow, show_separator: bool) -> Self {
        Self {
            row,
            selected: false,
            show_separator,
        }
    }
}

impl Selectable for ProviderListEntry {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ProviderListEntry {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(format!(
                "provider-settings-provider-wrapper-{}",
                self.row.kind.as_str()
            ))
            .w_full()
            .h(px(42.))
            .when(self.show_separator, |this| {
                this.border_b_1().border_color(cx.theme().border)
            })
            .child(
                h_flex()
                    .id(format!(
                        "provider-settings-provider-{}",
                        self.row.kind.as_str()
                    ))
                    .w_full()
                    .h_full()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .bg(if self.selected {
                        cx.theme().secondary_active
                    } else {
                        cx.theme().background
                    })
                    .when(!self.selected, |this| {
                        this.hover(|this| this.bg(cx.theme().secondary_hover))
                    })
                    .child(Icon::new(self.row.icon).text_color(cx.theme().muted_foreground))
                    .child(
                        Label::new(self.row.display_name)
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(div().flex_1())
                    .when(self.row.enabled, |this| {
                        this.child(div().size_2().rounded_full().bg(cx.theme().primary))
                    }),
            )
    }
}

impl ProviderListDelegate {
    pub(super) fn new(rows: Vec<ProviderListRow>, empty_label: impl Into<SharedString>) -> Self {
        Self {
            all_rows: rows.clone(),
            rows,
            last_query: String::new(),
            empty_label: empty_label.into(),
        }
    }

    pub(super) fn set_rows(&mut self, rows: Vec<ProviderListRow>) {
        self.all_rows = rows;
        self.apply_query();
    }

    pub(super) fn selected_index_for(
        rows: &[ProviderListRow],
        kind: &ProviderKindKey,
    ) -> Option<IndexPath> {
        rows.iter()
            .position(|row| &row.kind == kind)
            .map(IndexPath::new)
    }

    pub(super) fn selected_index_for_kind(&self, kind: &ProviderKindKey) -> Option<IndexPath> {
        Self::selected_index_for(&self.rows, kind)
    }

    pub(super) fn kind_for_index(&self, ix: IndexPath) -> Option<ProviderKindKey> {
        self.rows.get(ix.row).map(|row| row.kind.clone())
    }

    #[cfg(test)]
    pub(super) fn row_separator_for_test(&self, row: usize) -> bool {
        row + 1 < self.rows.len()
    }

    fn apply_query(&mut self) {
        let query = normalize_query(&self.last_query);
        if query.is_empty() {
            self.rows = self.all_rows.clone();
        } else {
            self.rows = self
                .all_rows
                .iter()
                .filter(|row| row.search_text.contains(&query))
                .cloned()
                .collect();
        }
    }

    #[cfg(test)]
    pub(super) fn set_query_for_test(&mut self, query: &str) {
        self.last_query = query.to_string();
        self.apply_query();
    }

    #[cfg(test)]
    pub(super) fn row_count_for_test(&self) -> usize {
        self.rows.len()
    }
}

impl ListDelegate for ProviderListDelegate {
    type Item = ProviderListEntry;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.last_query = query.to_string();
        self.apply_query();
        if self.rows.is_empty() {
            _cx.emit(ListEvent::Cancel);
        } else {
            _cx.emit(ListEvent::Select(IndexPath::new(0)));
        }
        Task::ready(())
    }

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.rows.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let show_separator = ix.row + 1 < self.rows.len();
        self.rows
            .get(ix.row)
            .cloned()
            .map(|row| ProviderListEntry::new(row, show_separator))
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .justify_center()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(self.empty_label.clone()).text_sm())
            .into_any_element()
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProviderModelRow {
    pub(super) model_id: String,
    pub(super) display_name: Option<String>,
    pub(super) enabled: bool,
    pub(super) capabilities: ModelCapabilitiesSnapshot,
    search_text: String,
}

pub(super) struct ProviderModelListDelegate {
    all_rows: Vec<ProviderModelRow>,
    rows: Vec<ProviderModelRow>,
    last_query: String,
    empty_label: SharedString,
}

#[derive(IntoElement, Clone)]
pub(super) struct ProviderModelListEntry {
    ix: IndexPath,
    row: ProviderModelRow,
    list: WeakEntity<ListState<ProviderModelListDelegate>>,
    selected: bool,
    show_separator: bool,
}

impl ProviderModelListEntry {
    fn new(
        ix: IndexPath,
        row: ProviderModelRow,
        list: WeakEntity<ListState<ProviderModelListDelegate>>,
        show_separator: bool,
    ) -> Self {
        Self {
            ix,
            row,
            list,
            selected: false,
            show_separator,
        }
    }
}

impl Selectable for ProviderModelListEntry {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ProviderModelListEntry {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ix = self.ix;
        let list = self.list.clone();

        div()
            .id(format!("provider-model-row-{}", self.row.model_id))
            .w_full()
            .h(px(58.))
            .when(self.show_separator, |this| {
                this.border_b_1().border_color(cx.theme().border)
            })
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .gap_3()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .bg(if self.selected {
                        cx.theme().accent
                    } else {
                        cx.theme().background
                    })
                    .px_3()
                    .child(
                        Switch::new(format!("provider-model-enabled-{}", self.row.model_id))
                            .checked(self.row.enabled)
                            .small()
                            .on_click(move |_, _window, cx| {
                                let _ = list.update(cx, |_, cx| {
                                    cx.emit(ListEvent::Confirm(ix));
                                });
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Label::new(
                                    self.row
                                        .display_name
                                        .clone()
                                        .unwrap_or_else(|| self.row.model_id.clone()),
                                )
                                .text_sm()
                                .truncate(),
                            )
                            .child(
                                Label::new(self.row.model_id)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate(),
                            ),
                    )
                    .child(capability_tags(&self.row.capabilities)),
            )
    }
}

impl ProviderModelListDelegate {
    pub(super) fn new(rows: Vec<ProviderModelRow>, empty_label: impl Into<SharedString>) -> Self {
        Self {
            all_rows: rows.clone(),
            rows,
            last_query: String::new(),
            empty_label: empty_label.into(),
        }
    }

    pub(super) fn set_rows(&mut self, rows: Vec<ProviderModelRow>) {
        self.all_rows = rows;
        self.apply_query();
    }

    pub(super) fn row_for_index(&self, ix: IndexPath) -> Option<&ProviderModelRow> {
        self.rows.get(ix.row)
    }

    fn apply_query(&mut self) {
        let query = normalize_query(&self.last_query);
        if query.is_empty() {
            self.rows = self.all_rows.clone();
        } else {
            self.rows = self
                .all_rows
                .iter()
                .filter(|row| row.search_text.contains(&query))
                .cloned()
                .collect();
        }
    }

    #[cfg(test)]
    pub(super) fn set_query_for_test(&mut self, query: &str) {
        self.last_query = query.to_string();
        self.apply_query();
    }

    #[cfg(test)]
    pub(super) fn row_count_for_test(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub(super) fn row_separator_for_test(&self, row: usize) -> bool {
        row + 1 < self.rows.len()
    }
}

impl ListDelegate for ProviderModelListDelegate {
    type Item = ProviderModelListEntry;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.last_query = query.to_string();
        self.apply_query();
        Task::ready(())
    }

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.rows.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let show_separator = ix.row + 1 < self.rows.len();
        let list = cx.entity().downgrade();
        self.rows
            .get(ix.row)
            .cloned()
            .map(|row| ProviderModelListEntry::new(ix, row, list, show_separator))
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .justify_center()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(self.empty_label.clone()).text_sm())
            .into_any_element()
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }
}

pub(super) fn provider_list_rows(
    providers: &[ProviderListItem],
    i18n: &I18n,
) -> Vec<ProviderListRow> {
    providers
        .iter()
        .map(|item| {
            let description = i18n.t(item.spec.description_key);
            ProviderListRow {
                kind: item.spec.kind.clone(),
                display_name: item.spec.display_name.into(),
                icon: item.spec.icon,
                enabled: item
                    .provider
                    .as_ref()
                    .is_some_and(|provider| provider.enabled),
                search_text: provider_search_text(item, &description, i18n),
            }
        })
        .collect()
}

pub(super) fn model_list_rows(models: &[ProviderModelDraft]) -> Vec<ProviderModelRow> {
    models
        .iter()
        .map(|model| ProviderModelRow {
            model_id: model.model_id.clone(),
            display_name: model.display_name.clone(),
            enabled: model.enabled,
            capabilities: model.capabilities.clone(),
            search_text: model_search_text(model),
        })
        .collect()
}

fn provider_search_text(item: &ProviderListItem, description: &str, i18n: &I18n) -> String {
    format!(
        "{} {} {} {} {} provider model models",
        item.spec.display_name,
        item.spec.kind.as_str(),
        description,
        i18n.t("settings-page-provider"),
        i18n.t("provider-section-models")
    )
    .to_lowercase()
}

fn model_search_text(model: &ProviderModelDraft) -> String {
    let mut tokens = vec![
        model.model_id.clone(),
        model.display_name.clone().unwrap_or_default(),
        if model.enabled {
            "enabled active 启用 已启用".to_string()
        } else {
            "disabled inactive 禁用 已禁用".to_string()
        },
    ];
    if model.fetched_at.is_none() {
        tokens.push("manual 手动".to_string());
    }
    tokens.extend(capability_search_tokens(&model.capabilities));
    tokens.join(" ").to_lowercase()
}

fn capability_search_tokens(capabilities: &ModelCapabilitiesSnapshot) -> Vec<String> {
    let mut tokens = Vec::new();
    if capabilities.reasoning.is_some() {
        tokens.push("reasoning 推理".to_string());
    }
    if capabilities.tool_calling.is_some() {
        tokens.push("tools tool calling 工具".to_string());
    }
    if capabilities.image_input.is_some() {
        tokens.push("vision image input 视觉 图片".to_string());
    }
    if capabilities.structured_output {
        tokens.push("structured output 结构化输出".to_string());
    }
    if capabilities.hosted_web_search {
        tokens.push("web search 搜索".to_string());
    }
    if capabilities.streaming {
        tokens.push("streaming 流式".to_string());
    }
    tokens
}

fn capability_tags(capabilities: &ModelCapabilitiesSnapshot) -> AnyElement {
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

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}
