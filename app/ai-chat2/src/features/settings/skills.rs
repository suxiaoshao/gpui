use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_store::StoreSelection;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use tracing::{Level, event};

use super::push_settings_error;
use rows::{
    SkillCatalogEntryView, SkillCatalogRow, SkillContentPanelState, filter_skill_catalog_rows,
    skill_catalog_list_items, skill_catalog_rows,
};

mod rows;

pub(super) struct SkillsSettingsPage {
    search_input: Entity<InputState>,
    skills: StoreSelection<Vec<state::skills::GlobalSkillEntry>>,
    last_error: StoreSelection<Option<String>>,
    list: ListState,
    rows: Vec<SkillCatalogRow>,
    items: Vec<PathBuf>,
    expanded: BTreeMap<PathBuf, SkillContentPanelState>,
    load_tasks: BTreeMap<PathBuf, Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl SkillsSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("skill-search-placeholder"))
        });
        let skill_catalog = state::skills::catalog(cx);
        let skills =
            skill_catalog.select_cloned(cx, state::skills::GlobalSkillCatalogState::entry_records);
        let last_error =
            skill_catalog.select(cx, |state| state.last_error().map(ToOwned::to_owned));
        let search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let catalog_subscription = skill_catalog.observe_select_in(
            cx,
            window,
            |state| {
                (
                    state.entries().to_vec(),
                    state.last_error().map(ToOwned::to_owned),
                    state.last_refreshed_at(),
                )
            },
            |page, _, _window, cx| {
                page.sync_list_items(cx, None);
            },
        );
        let mut page = Self {
            search_input,
            skills,
            last_error,
            list: ListState::new(0, ListAlignment::Top, px(2048.)).measure_all(),
            rows: Vec::new(),
            items: Vec::new(),
            expanded: BTreeMap::new(),
            load_tasks: BTreeMap::new(),
            _subscriptions: vec![search_subscription, catalog_subscription],
        };
        page.sync_list_items(cx, None);
        page
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.sync_list_items(cx, None);
        }
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn refresh_skills(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        state::skills::refresh_global_catalog(cx);
        self.sync_list_items(cx, None);
        let refresh_error =
            state::skills::catalog(cx).read(cx, |state| state.last_error().map(ToOwned::to_owned));
        if let Some(error) = refresh_error {
            let title = cx.global::<I18n>().t("notify-refresh-skills-failed");
            push_settings_error(window, cx, title, error);
        }
    }

    fn toggle_skill_content(
        &mut self,
        skill_file_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.expanded.remove(&skill_file_path).is_some() {
            self.load_tasks.remove(&skill_file_path);
            self.sync_list_items(cx, Some(&skill_file_path));
            return;
        }

        let Some(row) = self
            .rows
            .iter()
            .find(|row| row.key == skill_file_path)
            .cloned()
        else {
            let title = cx.global::<I18n>().t("notify-load-skill-content-failed");
            push_settings_error(window, cx, title, skill_file_path.display());
            return;
        };

        self.expanded
            .insert(skill_file_path.clone(), SkillContentPanelState::Loading);
        self.sync_list_items(cx, Some(&skill_file_path));

        let page = cx.entity().downgrade();
        let task_path = skill_file_path.clone();
        let load = cx.background_spawn(async move { state::skills::load_skill_content(row.entry) });
        let task = window.spawn(cx, async move |cx| {
            let result = load.await;
            if let Err(err) = page.update_in(cx, |page, _window, cx| {
                page.finish_skill_content_load(task_path, result, cx);
            }) {
                event!(
                    Level::ERROR,
                    error = ?err,
                    "finish skill settings content load failed"
                );
            }
        });
        self.load_tasks.insert(skill_file_path, task);
    }

    fn finish_skill_content_load(
        &mut self,
        skill_file_path: PathBuf,
        result: ai_chat_agent::Result<state::skills::LoadedSkillContent>,
        cx: &mut Context<Self>,
    ) {
        self.load_tasks.remove(&skill_file_path);
        if !self.expanded.contains_key(&skill_file_path) {
            return;
        }

        let next_state = match result {
            Ok(content) => SkillContentPanelState::Loaded {
                content: content.content.into(),
                content_sha256: content.content_sha256.into(),
            },
            Err(err) => SkillContentPanelState::Failed {
                message: err.to_string().into(),
            },
        };
        self.expanded.insert(skill_file_path.clone(), next_state);
        self.sync_list_items(cx, Some(&skill_file_path));
    }

    fn sync_list_items(&mut self, cx: &mut Context<Self>, remeasure_hint: Option<&PathBuf>) {
        let previous_keys = self.items.clone();
        let entries = self.skills.snapshot();
        let all_paths = entries
            .iter()
            .map(|entry| entry.skill_file_path.clone())
            .collect::<BTreeSet<_>>();
        self.expanded.retain(|path, _| all_paths.contains(path));
        self.load_tasks.retain(|path, _| all_paths.contains(path));

        let rows = skill_catalog_rows(entries.as_slice(), cx.global::<I18n>());
        let query = self.current_query(cx);
        self.rows = filter_skill_catalog_rows(&rows, &query);
        self.items = skill_catalog_list_items(&self.rows);
        sync_skill_list(&self.list, &previous_keys, &self.items, remeasure_hint);
        cx.notify();
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                Input::new(&self.search_input)
                    .flex_1()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .cleanable(true),
            )
            .child(
                Button::new("skill-settings-refresh")
                    .icon(IconName::RefreshCcw)
                    .label(cx.global::<I18n>().t("button-refresh-skills"))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.refresh_skills(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.last_error.cloned().map(|error| {
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().warning)
                .bg(cx.theme().warning.opacity(0.08))
                .text_color(cx.theme().warning)
                .p_3()
                .child(Icon::new(IconName::CircleAlert).with_size(px(16.)))
                .child(
                    Label::new(error.clone())
                        .text_sm()
                        .line_height(relative(1.4)),
                )
                .into_any_element()
        })
    }

    fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = if self.skills.snapshot().is_empty() {
            "skill-empty"
        } else {
            "skill-search-empty"
        };

        v_flex()
            .size_full()
            .min_h(px(260.))
            .items_center()
            .justify_center()
            .child(
                Label::new(cx.global::<I18n>().t(message))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element()
    }

    fn render_list_item(&self, page: WeakEntity<Self>, ix: usize) -> AnyElement {
        let Some(path) = self.items.get(ix).cloned() else {
            return div().into_any_element();
        };
        let Some(row) = self.rows.iter().find(|row| row.key == path).cloned() else {
            return div().into_any_element();
        };
        let content = self.expanded.get(&path).cloned();
        let toggle_page = page.clone();
        let scroll_page = page.clone();
        SkillCatalogEntryView::new(row, content)
            .on_toggle_content(move |path, window, cx| {
                let _ = toggle_page.update(cx, |page, cx| {
                    page.toggle_skill_content(path, window, cx);
                });
            })
            .on_chain_content_scroll(move |distance, _window, cx| {
                let _ = scroll_page.update(cx, |page, cx| {
                    page.list.scroll_by(distance);
                    cx.notify();
                });
            })
            .into_any_element()
    }

    fn render_body(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.items.is_empty() {
            return self.render_empty_state(cx);
        }

        let list_state = self.list.clone();
        let page = cx.entity().downgrade();

        div()
            .size_full()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(
                list(list_state.clone(), move |ix, _window, cx| {
                    page.upgrade()
                        .map(|page| page.read(cx).render_list_item(page.downgrade(), ix))
                        .unwrap_or_else(|| div().into_any_element())
                })
                .size_full(),
            )
            .vertical_scrollbar(&list_state)
            .into_any_element()
    }
}

impl Render for SkillsSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .h_full()
            .min_h_0()
            .gap_3()
            .child(self.render_toolbar(window, cx))
            .children(self.render_error_banner(cx))
            .child(div().flex_1().min_h_0().child(self.render_body(window, cx)))
    }
}

fn sync_skill_list(
    list_state: &ListState,
    previous_keys: &[PathBuf],
    next_keys: &[PathBuf],
    remeasure_hint: Option<&PathBuf>,
) {
    if previous_keys == next_keys {
        if let Some(row_ix) = remeasure_hint
            .and_then(|key| next_keys.iter().position(|current_key| current_key == key))
        {
            list_state.remeasure_items(row_ix..row_ix + 1);
        } else {
            list_state.remeasure();
        }
        return;
    }

    let first_diff = previous_keys
        .iter()
        .zip(next_keys.iter())
        .position(|(previous, next)| previous != next)
        .unwrap_or_else(|| previous_keys.len().min(next_keys.len()));

    list_state.splice(
        first_diff..previous_keys.len(),
        next_keys.len().saturating_sub(first_diff),
    );
}

#[cfg(test)]
mod tests {
    use super::sync_skill_list;
    use gpui::{ListAlignment, ListState, px};
    use std::path::PathBuf;

    #[test]
    fn sync_skill_list_handles_unchanged_keys() {
        let list = ListState::new(2, ListAlignment::Top, px(100.)).measure_all();
        let path = PathBuf::from("/tmp/a/SKILL.md");
        let keys = vec![path.clone(), PathBuf::from("/tmp/b/SKILL.md")];

        sync_skill_list(&list, &keys, &keys, Some(&path));

        assert_eq!(list.item_count(), 2);
    }
}
