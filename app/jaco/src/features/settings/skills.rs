use crate::{
    app::file_watch::{self, FileWatchBinding},
    features::skills,
    foundation::{I18n, assets::IconName},
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use super::push_settings_error;
use rows::{
    SkillCatalogEntryView, SkillCatalogRow, SkillContentPanelState, filter_skill_catalog_rows,
    skill_catalog_list_items, skill_catalog_rows,
};

mod rows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SkillWatchRefreshRequest {
    Immediate,
    Pending,
}

fn skill_watch_refresh_request(is_running: bool) -> SkillWatchRefreshRequest {
    if is_running {
        SkillWatchRefreshRequest::Pending
    } else {
        SkillWatchRefreshRequest::Immediate
    }
}

fn take_pending_skill_refresh(pending: &mut bool) -> bool {
    std::mem::take(pending)
}

fn expanded_skill_content(
    expanded: &BTreeSet<PathBuf>,
    skill_file_path: &Path,
    content: Option<&skills::LoadedSkillContent>,
) -> Option<SkillContentPanelState> {
    if !expanded.contains(skill_file_path) {
        return None;
    }

    Some(
        content
            .map(|content| SkillContentPanelState::Loaded {
                content: content.content.clone().into(),
                content_sha256: content.content_sha256.clone().into(),
            })
            .unwrap_or_else(|| SkillContentPanelState::Failed {
                message: "skill detail is unavailable".into(),
            }),
    )
}

pub(super) struct SkillsSettingsPage {
    search_input: Entity<InputState>,
    skill_catalog: skills::SkillCatalogOperation,
    list: ListState,
    rows: Vec<SkillCatalogRow>,
    items: Vec<PathBuf>,
    expanded: BTreeSet<PathBuf>,
    _watch_binding: FileWatchBinding,
    pending_dirty: bool,
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
        let watch_binding = Self::create_watch_binding(cx);
        let mut page = Self {
            search_input,
            skill_catalog: skills::SkillCatalogOperation::new(),
            list: ListState::new(0, ListAlignment::Top, px(2048.)).measure_all(),
            rows: Vec::new(),
            items: Vec::new(),
            expanded: BTreeSet::new(),
            _watch_binding: watch_binding,
            pending_dirty: false,
            _subscriptions: vec![search_subscription],
        };
        page.start_skill_load(cx);
        page
    }

    fn create_watch_binding(cx: &mut Context<Self>) -> FileWatchBinding {
        let targets = skills::watch_roots(&skills::SkillCatalogScope::Global)
            .into_iter()
            .filter_map(|path| match file_watch::directory_tree(path) {
                Ok(target) => Some(target),
                Err(problem) => {
                    file_watch::report_problem(problem, cx);
                    None
                }
            })
            .collect();
        file_watch::bind(targets, cx, |page, cx| page.on_skill_watch_dirty(cx))
    }

    fn on_skill_watch_dirty(&mut self, cx: &mut Context<Self>) {
        match skill_watch_refresh_request(self.skill_catalog.is_running()) {
            SkillWatchRefreshRequest::Pending => self.pending_dirty = true,
            SkillWatchRefreshRequest::Immediate => self.start_skill_load(cx),
        }
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
                if take_pending_skill_refresh(&mut page.pending_dirty) {
                    page.start_skill_load(cx);
                }
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
        if self.expanded.remove(&skill_file_path) {
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

        self.expanded.insert(skill_file_path.clone());
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
        self.expanded.retain(|path| all_paths.contains(path));

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
                    .loading(self.skill_catalog.is_running())
                    .disabled(self.skill_catalog.is_running())
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
        let content = expanded_skill_content(
            &self.expanded,
            &path,
            self.skill_catalog
                .data()
                .and_then(|data| data.details().get(&path)),
        );
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
        if self.skill_catalog.data().is_none() && self.skill_catalog.is_running() {
            return v_flex()
                .size_full()
                .min_h(px(260.))
                .items_center()
                .justify_center()
                .gap_2()
                .child(gpui_component::spinner::Spinner::new())
                .child(
                    Label::new(cx.global::<I18n>().t("resource-status-loading"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element();
        }
        if self.skill_catalog.data().is_none() && self.skill_catalog.problem().is_some() {
            return v_flex()
                .size_full()
                .min_h(px(260.))
                .items_center()
                .justify_center()
                .child(
                    Label::new(cx.global::<I18n>().t("resource-status-unavailable"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element();
        }
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
    use super::{
        SkillWatchRefreshRequest, expanded_skill_content, skill_watch_refresh_request,
        take_pending_skill_refresh,
    };
    use crate::features::{settings::skills::rows::SkillContentPanelState, skills};
    use gpui::{ListAlignment, ListState, Task, px};
    use gpui_operation::{Complete, Load, Refresh, Transition, refresh};
    use std::{collections::BTreeSet, path::PathBuf};

    #[derive(Debug, PartialEq, Eq)]
    struct TestProblem;

    impl std::fmt::Display for TestProblem {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("test skill refresh failed")
        }
    }

    impl std::error::Error for TestProblem {}

    #[derive(Debug, PartialEq, Eq)]
    struct CatalogSnapshot {
        data: Vec<&'static str>,
        rows: Vec<&'static str>,
    }

    #[test]
    fn sync_skill_list_handles_unchanged_keys() {
        let list = ListState::new(2, ListAlignment::Top, px(100.)).measure_all();
        let path = PathBuf::from("/tmp/a/SKILL.md");
        let keys = vec![path.clone(), PathBuf::from("/tmp/b/SKILL.md")];

        sync_skill_list(&list, &keys, &keys, Some(&path));

        assert_eq!(list.item_count(), 2);
    }

    #[test]
    fn running_skill_watch_dirty_coalesces_to_one_follow_up() {
        let mut pending = false;
        let mut follow_up_count = 0;

        for _ in 0..4 {
            assert_eq!(
                skill_watch_refresh_request(true),
                SkillWatchRefreshRequest::Pending
            );
            pending = true;
        }

        if take_pending_skill_refresh(&mut pending) {
            follow_up_count += 1;
        }
        assert_eq!(follow_up_count, 1);
        assert!(!pending);
        assert!(!take_pending_skill_refresh(&mut pending));
        assert_eq!(
            skill_watch_refresh_request(false),
            SkillWatchRefreshRequest::Immediate
        );
    }

    #[test]
    fn failed_skill_refresh_preserves_last_good_data_and_rows() {
        type Operation = refresh::Operation<CatalogSnapshot, TestProblem, Task<()>>;

        let snapshot = CatalogSnapshot {
            data: vec!["rust", "gpui"],
            rows: vec!["rust", "gpui"],
        };
        let mut operation = Operation::new();
        operation.transition(Load(Task::ready(())));
        operation.transition(Complete(Ok(snapshot)));

        operation.transition(Refresh(Task::ready(())));
        operation.transition(Complete(Err(TestProblem)));

        assert!(matches!(operation, Operation::Degraded(_)));
        assert_eq!(
            operation.data(),
            Some(&CatalogSnapshot {
                data: vec!["rust", "gpui"],
                rows: vec!["rust", "gpui"],
            })
        );
    }

    #[test]
    fn expanded_skill_content_uses_latest_loaded_detail() {
        let path = PathBuf::from("/tmp/example/SKILL.md");
        let expanded = BTreeSet::from([path.clone()]);
        let initial = skills::LoadedSkillContent {
            content: "initial content".to_owned(),
            content_sha256: "initial-hash".to_owned(),
        };
        let refreshed = skills::LoadedSkillContent {
            content: "refreshed content".to_owned(),
            content_sha256: "refreshed-hash".to_owned(),
        };

        assert_eq!(
            expanded_skill_content(&expanded, &path, Some(&initial)),
            Some(SkillContentPanelState::Loaded {
                content: "initial content".into(),
                content_sha256: "initial-hash".into(),
            })
        );
        assert_eq!(
            expanded_skill_content(&expanded, &path, Some(&refreshed)),
            Some(SkillContentPanelState::Loaded {
                content: "refreshed content".into(),
                content_sha256: "refreshed-hash".into(),
            })
        );
    }
}
