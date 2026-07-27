use crate::{
    features::skills,
    foundation::{I18n, assets::IconName},
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
use gpui_operation::{Complete, Load, Refresh, Retry, Transition};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use super::push_settings_error;
use rows::{
    SkillCatalogEntryView, SkillCatalogRow, SkillContentPanelState, filter_skill_catalog_rows,
    skill_catalog_list_items, skill_catalog_rows,
};

mod rows;

pub(super) struct SkillsSettingsPage {
    search_input: Entity<InputState>,
    skill_catalog: skills::SkillCatalogOperation,
    list: ListState,
    rows: Vec<SkillCatalogRow>,
    items: Vec<PathBuf>,
    expanded: BTreeMap<PathBuf, SkillContentPanelState>,
    _subscriptions: Vec<Subscription>,
}

impl SkillsSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("skill-search-placeholder"))
        });
        let search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let mut page = Self {
            search_input,
            skill_catalog: skills::SkillCatalogOperation::new(),
            list: ListState::new(0, ListAlignment::Top, px(2048.)).measure_all(),
            rows: Vec::new(),
            items: Vec::new(),
            expanded: BTreeMap::new(),
            _subscriptions: vec![search_subscription],
        };
        page.start_skill_load(cx);
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

    fn start_skill_load(&mut self, cx: &mut Context<Self>) {
        if self.skill_catalog.is_running() {
            return;
        }
        let load =
            cx.background_spawn(async { skills::load_catalog(skills::SkillCatalogScope::Global) });
        let task = cx.spawn(async move |page, cx| {
            let result = load.await;
            let Some(page) = page.upgrade() else {
                return;
            };
            page.update(cx, |page, cx| {
                page.skill_catalog.transition(Complete(result));
                page.sync_list_items(cx, None);
            });
        });
        match &self.skill_catalog {
            skills::SkillCatalogOperation::Idle(_) => {
                self.skill_catalog.transition(Load(task));
            }
            skills::SkillCatalogOperation::Ready(_)
            | skills::SkillCatalogOperation::Degraded(_) => {
                self.skill_catalog.transition(Refresh(task));
            }
            skills::SkillCatalogOperation::Unavailable(_) => {
                self.skill_catalog.transition(Retry(task));
            }
            skills::SkillCatalogOperation::Loading(_)
            | skills::SkillCatalogOperation::Refreshing(_)
            | skills::SkillCatalogOperation::RefreshingDegraded(_)
            | skills::SkillCatalogOperation::Retrying(_) => unreachable!(),
        }
        cx.notify();
    }

    fn toggle_skill_content(
        &mut self,
        skill_file_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.expanded.remove(&skill_file_path).is_some() {
            self.sync_list_items(cx, Some(&skill_file_path));
            return;
        }

        let Some(_row) = self
            .rows
            .iter()
            .find(|row| row.key == skill_file_path)
            .cloned()
        else {
            let title = cx.global::<I18n>().t("notify-load-skill-content-failed");
            push_settings_error(window, cx, title, skill_file_path.display());
            return;
        };

        let next_state = self
            .skill_catalog
            .data()
            .and_then(|data| data.details().get(&skill_file_path))
            .map(|content| SkillContentPanelState::Loaded {
                content: content.content.clone().into(),
                content_sha256: content.content_sha256.clone().into(),
            })
            .unwrap_or_else(|| SkillContentPanelState::Failed {
                message: "skill detail is unavailable".into(),
            });
        self.expanded.insert(skill_file_path.clone(), next_state);
        self.sync_list_items(cx, Some(&skill_file_path));
    }

    fn sync_list_items(&mut self, cx: &mut Context<Self>, remeasure_hint: Option<&PathBuf>) {
        let previous_keys = self.items.clone();
        let entries = self
            .skill_catalog
            .data()
            .map(skills::SkillCatalogData::entries)
            .unwrap_or_default();
        let all_paths = entries
            .iter()
            .map(|entry| entry.skill_file_path.clone())
            .collect::<BTreeSet<_>>();
        self.expanded.retain(|path, _| all_paths.contains(path));

        let rows = skill_catalog_rows(entries, cx.global::<I18n>());
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
                    .on_click(cx.listener(|page, _, _window, cx| {
                        page.start_skill_load(cx);
                    })),
            )
            .into_any_element()
    }

    fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.skill_catalog.problem().map(|problem| {
            let error = problem.to_string();
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().warning)
                .bg(cx.theme().tokens.warning.background.opacity(0.08))
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
        let message = if self
            .skill_catalog
            .data()
            .is_none_or(|data| data.entries().is_empty())
        {
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
