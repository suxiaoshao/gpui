use std::rc::Rc;

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IndexPath, Selectable, Sizable, StyledExt, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
    tag::Tag,
    v_flex,
};
use jaco_core::SkillSourceKind;

use crate::{
    foundation::{I18n, search::field_matches_query},
    state::skills::GlobalSkillEntry,
};

use super::{
    buffer::{clamp_offset, is_word_char, previous_grapheme_boundary},
    token::ComposerSkill,
};

const COMPLETION_ITEM_HEIGHT: f32 = 36.;

type OnConfirm = Rc<dyn Fn(SkillCompletionRow, &mut Window, &mut App) + 'static>;
type OnCancel = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCompletionTrigger {
    pub(super) range: std::ops::Range<usize>,
    pub(super) query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCompletionRow {
    pub(super) skill: ComposerSkill,
    name: SharedString,
    description: SharedString,
    source_label: SharedString,
    search_text: String,
}

#[derive(IntoElement, Clone)]
pub(super) struct SkillCompletionItem {
    row: Rc<SkillCompletionRow>,
    is_selected: bool,
}

impl SkillCompletionItem {
    fn new(row: Rc<SkillCompletionRow>) -> Self {
        Self {
            row,
            is_selected: false,
        }
    }
}

impl Selectable for SkillCompletionItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl RenderOnce for SkillCompletionItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let row = self.row;
        h_flex()
            .id(format!("skill-completion-row-{}", row.skill.name))
            .w_full()
            .h(px(COMPLETION_ITEM_HEIGHT))
            .items_center()
            .gap_2()
            .px_3()
            .cursor_pointer()
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new(row.name.clone())
                            .flex_none()
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(
                        Label::new(row.description.clone())
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .child(
                Tag::secondary()
                    .small()
                    .outline()
                    .flex_none()
                    .child(row.source_label.clone()),
            )
    }
}

pub(super) struct SkillCompletionDelegate {
    ix: Option<IndexPath>,
    all_rows: Vec<Rc<SkillCompletionRow>>,
    rows: Vec<Rc<SkillCompletionRow>>,
    query: String,
    empty_label: SharedString,
    on_confirm: OnConfirm,
    on_cancel: OnCancel,
}

impl SkillCompletionDelegate {
    pub(super) fn new(
        rows: Vec<SkillCompletionRow>,
        empty_label: SharedString,
        on_confirm: OnConfirm,
        on_cancel: OnCancel,
    ) -> Self {
        let all_rows = rows.into_iter().map(Rc::new).collect::<Vec<_>>();
        Self {
            ix: None,
            rows: all_rows.clone(),
            all_rows,
            query: String::new(),
            empty_label,
            on_confirm,
            on_cancel,
        }
    }

    pub(super) fn set_rows(&mut self, rows: Vec<SkillCompletionRow>) {
        self.all_rows = rows.into_iter().map(Rc::new).collect();
        self.apply_query();
    }

    pub(super) fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.apply_query();
    }

    pub(super) fn first_index(&self) -> Option<IndexPath> {
        (!self.rows.is_empty()).then_some(IndexPath::default())
    }

    pub(super) fn selected_row(&self) -> Option<SkillCompletionRow> {
        let ix = self.ix?;
        self.rows.get(ix.row).map(|row| row.as_ref().clone())
    }

    pub(super) fn item_count(&self) -> usize {
        self.rows.len()
    }

    fn apply_query(&mut self) {
        let query = self.query.trim();
        if query.is_empty() {
            self.rows = self.all_rows.clone();
            return;
        }

        self.rows = self
            .all_rows
            .iter()
            .filter(|row| field_matches_query(&row.search_text, query))
            .cloned()
            .collect();
    }
}

impl ListDelegate for SkillCompletionDelegate {
    type Item = SkillCompletionItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.rows.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.rows.get(ix.row).cloned().map(SkillCompletionItem::new)
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .px_4()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(self.empty_label.clone()).text_sm().text_center())
            .into_any_element()
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.ix = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let on_confirm = self.on_confirm.clone();
        // `confirm` runs while `ListState` is locked. The callback updates the
        // composer and may update this completion list, so cross the window
        // boundary before invoking it.
        window.defer(cx, move |window, cx| {
            on_confirm(row, window, cx);
        });
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let on_cancel = self.on_cancel.clone();
        // Keep cancellation on the same boundary as confirmation so the
        // callback cannot re-enter the list if its owner reconciles state.
        window.defer(cx, move |window, cx| {
            on_cancel(window, cx);
        });
    }
}

pub(super) fn skill_completion_rows(
    entries: &[GlobalSkillEntry],
    i18n: &I18n,
) -> Vec<SkillCompletionRow> {
    entries
        .iter()
        .map(|entry| {
            let description = entry
                .description
                .clone()
                .unwrap_or_else(|| i18n.t("skill-description-empty").to_string());
            SkillCompletionRow {
                skill: ComposerSkill::from(entry),
                name: entry.name.clone().into(),
                description: description.into(),
                source_label: skill_source_label(entry.source_kind, i18n),
                search_text: entry.search_text.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod reentrancy_tests {
    use super::{ComposerSkill, SkillCompletionDelegate, SkillCompletionRow};
    use gpui::{
        App, AppContext, Context, Entity, IntoElement, Render, SharedString, TestAppContext,
        Window, div,
    };
    use gpui_component::{
        IndexPath,
        list::{ListDelegate, ListState},
    };
    use jaco_core::SkillSourceKind;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn test_row() -> SkillCompletionRow {
        SkillCompletionRow {
            skill: ComposerSkill {
                name: "rust".to_string(),
                description: Some("Rust skill".to_string()),
                source_kind: SkillSourceKind::User,
                skill_file_path: "/skills/rust/SKILL.md".to_string(),
                directory_path: "/skills/rust".to_string(),
            },
            name: "rust".into(),
            description: "Rust skill".into(),
            source_label: "User".into(),
            search_text: "rust Rust skill".to_string(),
        }
    }

    struct TestRoot;

    impl Render for TestRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn confirm_callback_runs_after_list_update_finishes(cx: &mut TestAppContext) {
        let list_slot = Rc::new(RefCell::new(
            None::<Entity<ListState<SkillCompletionDelegate>>>,
        ));
        let callback_ran = Rc::new(Cell::new(false));
        let on_confirm = Rc::new({
            let list_slot = list_slot.clone();
            let callback_ran = callback_ran.clone();
            move |_row: SkillCompletionRow, _window: &mut Window, cx: &mut App| {
                let list = list_slot
                    .borrow()
                    .as_ref()
                    .cloned()
                    .expect("skill completion list should be initialized");
                list.update(cx, |_, _| callback_ran.set(true));
            }
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let list = cx.new(|cx| {
                let mut list = ListState::new(
                    SkillCompletionDelegate::new(
                        vec![test_row()],
                        SharedString::from("Empty"),
                        on_confirm,
                        Rc::new(|_, _| {}),
                    ),
                    window,
                    cx,
                );
                list.delegate_mut()
                    .set_selected_index(Some(IndexPath::default()), window, cx);
                list
            });
            *list_slot.borrow_mut() = Some(list);
            TestRoot
        });
        let list = list_slot
            .borrow()
            .as_ref()
            .cloned()
            .expect("skill completion list should be initialized");

        cx.update(|window, cx| {
            list.update(cx, |list, cx| {
                list.delegate_mut().confirm(false, window, cx);
            });
        });
        cx.run_until_parked();

        assert!(callback_ran.get());
    }
}

pub(super) fn skill_completion_trigger(
    text: &str,
    cursor: usize,
    marked_range: Option<&std::ops::Range<usize>>,
) -> Option<SkillCompletionTrigger> {
    if marked_range.is_some() {
        return None;
    }

    let cursor = clamp_offset(text, cursor);
    let mut query_start = cursor;
    while query_start > 0 {
        let previous = previous_grapheme_boundary(text, query_start);
        let ch = text[previous..query_start].chars().next()?;
        if !is_skill_name_char(ch) {
            break;
        }
        query_start = previous;
    }

    if query_start == 0 {
        return None;
    }

    let dollar_start = previous_grapheme_boundary(text, query_start);
    if &text[dollar_start..query_start] != "$" {
        return None;
    }

    let before_is_boundary = dollar_start == 0
        || text[..dollar_start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_word_char(ch));
    if !before_is_boundary {
        return None;
    }

    Some(SkillCompletionTrigger {
        range: dollar_start..cursor,
        query: text[query_start..cursor].to_string(),
    })
}

fn is_skill_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-')
}

pub(super) fn skill_source_label(source_kind: SkillSourceKind, i18n: &I18n) -> SharedString {
    i18n.t(match source_kind {
        SkillSourceKind::BuiltIn => "skill-source-builtin",
        SkillSourceKind::User => "skill-source-user",
        SkillSourceKind::Project => "skill-source-project",
        SkillSourceKind::Plugin => "skill-source-plugin",
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::skill_completion_trigger;

    #[test]
    fn trigger_matches_dollar_prefix_at_word_boundary() {
        assert_eq!(
            skill_completion_trigger("ask $bro", "ask $bro".len(), None)
                .map(|trigger| (trigger.range, trigger.query)),
            Some((4..8, "bro".to_string()))
        );
        assert_eq!(
            skill_completion_trigger("$", "$".len(), None)
                .map(|trigger| (trigger.range, trigger.query)),
            Some((0..1, "".to_string()))
        );
        assert!(skill_completion_trigger("ask foo$bar", "ask foo$bar".len(), None).is_none());
        assert!(skill_completion_trigger("ask $$bar", "ask $$bar".len(), None).is_none());
    }

    #[test]
    fn trigger_ignores_marked_text() {
        assert!(skill_completion_trigger("$bro", "$bro".len(), Some(&(0..2))).is_none());
    }
}
