use feiwen_data::{service::Novel, types::Author};
use gpui_kit::component::{
    ActiveTheme, StyledExt,
    label::Label,
    link::Link,
    table::{Column, ColumnFixed, ColumnSort, TableDelegate, TableState},
    tag::Tag as TagComponent,
};
use gpui_kit::{App, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div};

// Fit is an explicit approximation of visible content, never a full-result scan.
const FIT_MAX_ROWS: usize = 64;
const FIT_MAX_CELL_BYTES: usize = 512;
const FIT_MAX_BYTES: usize = 8 * 1024;
const FIT_CACHE_ENTRIES: usize = 512;
const FIT_MAX_REMS: f32 = 45.0;

#[derive(Default)]
struct TextWidthCache {
    widths:
        std::collections::HashMap<(gpui_kit::SharedString, gpui_kit::Font, u32), gpui_kit::Pixels>,
}

impl TextWidthCache {
    fn measure(
        &mut self,
        text: gpui_kit::SharedString,
        font: gpui_kit::Font,
        font_size: gpui_kit::Pixels,
        bytes_left: &mut usize,
        shape: impl FnOnce(gpui_kit::SharedString, gpui_kit::Font, gpui_kit::Pixels) -> gpui_kit::Pixels,
    ) -> Option<gpui_kit::Pixels> {
        // Guard before hashing, copying or shaping text; cache hits consume work too.
        if text.len() > FIT_MAX_CELL_BYTES || text.len() > *bytes_left {
            return None;
        }
        *bytes_left -= text.len();
        let key = (text.clone(), font.clone(), f32::from(font_size).to_bits());
        if let Some(width) = self.widths.get(&key) {
            return Some(*width);
        }
        if self.widths.len() >= FIT_CACHE_ENTRIES {
            self.widths.clear();
        }
        let width = shape(text, font, font_size);
        self.widths.insert(key, width);
        Some(width)
    }
}

const SITE_ORIGIN: &str = "https://xn--pxtr7m.com";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultColumn {
    Title,
    Description,
    Author,
    WordCount,
    ReadCount,
    ReplyCount,
    IsLimit,
    LatestChapter,
    Tags,
}

impl ResultColumn {
    const ALL: [Self; 9] = [
        Self::Title,
        Self::Description,
        Self::Author,
        Self::WordCount,
        Self::ReadCount,
        Self::ReplyCount,
        Self::IsLimit,
        Self::LatestChapter,
        Self::Tags,
    ];
}

pub(crate) struct ResultsTableDelegate {
    pub(crate) rem_size: gpui_kit::Pixels,
    novels: Vec<Novel>,
    order: Vec<usize>,
    loading: bool,
    fit_visible: std::ops::Range<usize>,
    fit_cache: TextWidthCache,
}

impl ResultsTableDelegate {
    pub(crate) fn new(rem_size: gpui_kit::Pixels) -> Self {
        Self {
            rem_size,
            novels: Vec::new(),
            order: Vec::new(),
            loading: false,
            fit_visible: 0..0,
            fit_cache: Default::default(),
        }
    }

    pub(crate) fn set_novels(&mut self, novels: Vec<Novel>) {
        self.fit_cache.widths.clear();
        self.order = (0..novels.len()).collect();
        self.novels = novels;
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    fn column_spec(column: ResultColumn, rem_size: gpui_kit::Pixels) -> Column {
        match column {
            ResultColumn::Title => Column::new("title", "标题")
                .width(rem_size * 15.0)
                .fixed(ColumnFixed::Left)
                .sortable(),
            ResultColumn::Description => Column::new("description", "简介").width(rem_size * 20.0),
            ResultColumn::Author => Column::new("author", "作者")
                .width(rem_size * 8.75)
                .sortable(),
            ResultColumn::WordCount => Column::new("word_count", "字数")
                .width(rem_size * 6.0)
                .text_right()
                .sortable(),
            ResultColumn::ReadCount => Column::new("read_count", "阅读")
                .width(rem_size * 6.0)
                .text_right()
                .sortable(),
            ResultColumn::ReplyCount => Column::new("reply_count", "回复")
                .width(rem_size * 6.0)
                .text_right()
                .sortable(),
            ResultColumn::IsLimit => Column::new("is_limit", "受限")
                .width(rem_size * 4.5)
                .text_center()
                .sortable(),
            ResultColumn::LatestChapter => Column::new("latest_chapter", "最新章节")
                .width(rem_size * 11.25)
                .sortable(),
            ResultColumn::Tags => Column::new("tags", "标签").width(rem_size * 22.5),
        }
    }

    fn novel_at(&self, row_ix: usize) -> Option<&Novel> {
        self.order.get(row_ix).and_then(|ix| self.novels.get(*ix))
    }

    fn author_label(novel: &Novel) -> gpui_kit::SharedString {
        match &novel.author {
            Author::Anonymous(name) => name.clone(),
            Author::Known(title) => title.name.clone(),
        }
    }

    fn sort_by_column(&mut self, col_ix: usize, sort: ColumnSort) {
        let descending = matches!(sort, ColumnSort::Descending);
        let novels = &self.novels;
        self.order.sort_by(|left, right| {
            let left = &novels[*left];
            let right = &novels[*right];
            let ordering = match ResultColumn::ALL.get(col_ix).copied() {
                Some(ResultColumn::Title) => left.title.name.cmp(&right.title.name),
                Some(ResultColumn::Author) => {
                    author_name(&left.author).cmp(author_name(&right.author))
                }
                Some(ResultColumn::WordCount) => left.count.word_count.cmp(&right.count.word_count),
                Some(ResultColumn::IsLimit) => left.is_limit.cmp(&right.is_limit),
                Some(ResultColumn::LatestChapter) => {
                    left.latest_chapter.name.cmp(&right.latest_chapter.name)
                }
                Some(ResultColumn::ReadCount) => {
                    return compare_missing_last(
                        left.count.read_count,
                        right.count.read_count,
                        descending,
                    );
                }
                Some(ResultColumn::ReplyCount) => {
                    return compare_missing_last(
                        left.count.reply_count,
                        right.count.reply_count,
                        descending,
                    );
                }
                _ => std::cmp::Ordering::Equal,
            };
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    fn restore_original_order(&mut self) {
        self.order = (0..self.novels.len()).collect();
    }
}

impl TableDelegate for ResultsTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        ResultColumn::ALL.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.novels.len()
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        ResultColumn::ALL
            .get(col_ix)
            .copied()
            .map(|column| Self::column_spec(column, self.rem_size))
            .unwrap_or_else(|| Column::new("unknown", ""))
    }

    fn fit_column(
        &mut self,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Option<gpui_kit::Pixels> {
        use gpui_kit::{Font, FontWeight, Hsla, TextRun, px};
        let column = *ResultColumn::ALL.get(col_ix)?;
        // Badge wrapping/padding is not equivalent to a single text measurement.
        if matches!(column, ResultColumn::Tags | ResultColumn::IsLimit) {
            return None;
        }
        let rem = window.rem_size();
        let spec = Self::column_spec(column, rem);
        let padding = spec
            .paddings
            .unwrap_or_else(|| gpui_kit::component::Size::default().table_cell_padding());
        let scale = window.scale_factor();
        // Reserve table-owned padding and one device pixel for upstream rounding.
        // This only caps explicit fit; the ordinary manual-resize bounds are unchanged.
        let content_cap = px(((f32::from(rem * FIT_MAX_REMS) * scale).floor()
            - (f32::from(padding.left) * scale).ceil()
            - (f32::from(padding.right) * scale).ceil()
            - 1.)
            .max(0.)
            / scale);
        let mut font = window.text_style().font();
        font.family = cx.theme().font_family.clone();
        let mut bytes_left = FIT_MAX_BYTES;
        let mut measure = |text, weight| {
            self.fit_cache.measure(
                text,
                Font {
                    weight,
                    ..font.clone()
                },
                rem * 0.875,
                &mut bytes_left,
                |text, font, size| {
                    let run = TextRun {
                        len: text.len(),
                        font,
                        color: Hsla::default(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(text, size, &[run], None)
                        .width()
                },
            )
        };
        let sort_icon = if spec.sort.is_some() {
            rem * 0.75 + px(4.)
        } else {
            px(0.)
        };
        let mut width = measure(spec.name, FontWeight::NORMAL).unwrap_or(content_cap) + sort_icon;
        let start_row = self.fit_visible.start.min(self.order.len());
        let end_row = self
            .fit_visible
            .end
            .min(start_row.saturating_add(FIT_MAX_ROWS))
            .min(self.order.len());
        for novel in self.order[start_row..end_row]
            .iter()
            .filter_map(|ix| self.novels.get(*ix))
        {
            let text = match column {
                ResultColumn::Title => novel.title.name.clone(),
                ResultColumn::Description => novel.desc.clone(),
                ResultColumn::Author => Self::author_label(novel),
                ResultColumn::WordCount => number_text(Some(novel.count.word_count)),
                ResultColumn::ReadCount => number_text(novel.count.read_count),
                ResultColumn::ReplyCount => number_text(novel.count.reply_count),
                ResultColumn::LatestChapter => novel.latest_chapter.name.clone(),
                ResultColumn::Tags | ResultColumn::IsLimit => unreachable!(),
            };
            let weight = if column == ResultColumn::Title {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            };
            // Oversized cells or an exhausted callback budget choose the bounded
            // automatic width, without hashing or shaping the rest of that text.
            let Some(measured) = measure(text, weight) else {
                return Some(content_cap);
            };
            width = width.max(measured);
            if width >= content_cap {
                break;
            }
        }
        Some(width.min(content_cap))
    }

    fn visible_rows_changed(
        &mut self,
        range: std::ops::Range<usize>,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.fit_visible = range;
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        // Sorting must not bubble into column selection and clear the selected row.
        cx.stop_propagation();
        let old_row_ids: Vec<_> = self
            .order
            .iter()
            .map(|ix| self.novels[*ix].title.id)
            .collect();
        if sort == ColumnSort::Default {
            self.restore_original_order();
        } else {
            self.sort_by_column(col_ix, sort);
        }
        cx.defer_in(window, move |table, _, cx| {
            if let Some(selected) = table.selected_row()
                && let Some(original) = old_row_ids.get(selected)
                && let Some(next) = table
                    .delegate()
                    .order
                    .iter()
                    .position(|ix| table.delegate().novels[*ix].title.id == *original)
            {
                table.set_selected_row(next, cx);
            }
        });
        cx.notify();
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        div().id((
            "novel-row",
            self.novel_at(row_ix).map_or(0, |novel| novel.title.id) as u64,
        ))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(novel) = self.novel_at(row_ix) else {
            return div().into_any_element();
        };
        match ResultColumn::ALL[col_ix] {
            ResultColumn::Title => Link::new(("novel-title-link", novel.title.id as u64))
                .href(novel_url(novel.title.id))
                .child(
                    Label::new(novel.title.name.clone())
                        .text_sm()
                        .font_medium()
                        .truncate(),
                )
                .into_any_element(),
            ResultColumn::Description => Label::new(novel.desc.clone())
                .text_sm()
                .truncate()
                .into_any_element(),
            ResultColumn::Author => {
                let label = Label::new(Self::author_label(novel))
                    .text_sm()
                    .truncate()
                    .into_any_element();
                match author_url(&novel.author) {
                    Some(url) => Link::new(author_link_id(novel))
                        .href(url)
                        .child(label)
                        .into_any_element(),
                    None => label,
                }
            }
            ResultColumn::WordCount => number_cell(Some(novel.count.word_count)).into_any_element(),
            ResultColumn::ReadCount => number_cell(novel.count.read_count).into_any_element(),
            ResultColumn::ReplyCount => number_cell(novel.count.reply_count).into_any_element(),
            ResultColumn::IsLimit => {
                let tag = if novel.is_limit {
                    TagComponent::warning().outline().child("是")
                } else {
                    TagComponent::secondary().outline().child("否")
                };
                tag.into_any_element()
            }
            ResultColumn::LatestChapter => Label::new(novel.latest_chapter.name.clone())
                .text_sm()
                .truncate()
                .into_any_element(),
            ResultColumn::Tags => {
                let mut tags = novel
                    .tags
                    .iter()
                    .map(|tag| tag.name.clone())
                    .collect::<Vec<_>>();
                tags.sort();
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(
                        tags.into_iter()
                            .take(6)
                            .map(|tag| TagComponent::secondary().outline().child(tag)),
                    )
                    .into_any_element()
            }
        }
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().muted_foreground)
            .child("暂无查询结果")
    }
}

fn number_text(value: Option<i32>) -> gpui_kit::SharedString {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
        .into()
}

fn number_cell(value: Option<i32>) -> impl IntoElement {
    Label::new(number_text(value)).text_sm()
}

fn novel_url(id: i32) -> String {
    format!("{SITE_ORIGIN}/threads/{id}/profile")
}

fn author_url(author: &Author) -> Option<String> {
    Some(format!("{SITE_ORIGIN}/users/{}", author_id(author)?))
}

fn author_id(author: &Author) -> Option<i32> {
    match author {
        Author::Known(title) => Some(title.id),
        Author::Anonymous(_) => None,
    }
}

fn author_link_id(novel: &Novel) -> String {
    format!(
        "author-link-{}-{}",
        novel.title.id,
        author_id(&novel.author).unwrap_or_default()
    )
}

fn author_name(author: &Author) -> &str {
    match author {
        Author::Anonymous(name) => name.as_ref(),
        Author::Known(title) => title.name.as_ref(),
    }
}
fn compare_missing_last(
    left: Option<i32>,
    right: Option<i32>,
    descending: bool,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => {
            if descending {
                right.cmp(&left)
            } else {
                left.cmp(&right)
            }
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feiwen_data::types::{NovelCount, Title};

    #[test]
    fn loading_defaults_to_false_and_can_be_toggled() {
        let mut delegate = ResultsTableDelegate::new(gpui_kit::px(16.));

        assert!(!delegate.loading);

        delegate.set_loading(true);
        assert!(delegate.loading);

        delegate.set_loading(false);
        assert!(!delegate.loading);
    }

    #[test]
    fn novel_url_uses_title_id() {
        assert_eq!(
            novel_url(165143),
            "https://xn--pxtr7m.com/threads/165143/profile"
        );
    }

    #[test]
    fn known_author_url_uses_author_id() {
        let author = Author::Known(feiwen_data::types::Title::new("作者".into(), 538220));

        assert_eq!(
            author_url(&author).as_deref(),
            Some("https://xn--pxtr7m.com/users/538220")
        );
    }

    #[test]
    fn anonymous_author_has_no_url() {
        assert_eq!(author_url(&Author::Anonymous("匿名".into())), None);
    }

    #[test]
    fn optional_count_sorts_keep_missing_values_last() {
        let mut delegate = ResultsTableDelegate::new(gpui_kit::px(16.));
        delegate.set_novels(vec![
            novel_with_read_count(1, Some(10)),
            novel_with_read_count(2, None),
            novel_with_read_count(3, Some(5)),
        ]);
        let read_count_col = ResultColumn::ALL
            .iter()
            .position(|column| *column == ResultColumn::ReadCount)
            .expect("read count column exists");

        delegate.sort_by_column(read_count_col, ColumnSort::Ascending);
        assert_eq!(delegate.novel_ids(), vec![3, 1, 2]);

        delegate.sort_by_column(read_count_col, ColumnSort::Descending);
        assert_eq!(delegate.novel_ids(), vec![1, 3, 2]);
    }

    #[test]
    fn default_sort_restores_query_result_order() {
        let mut delegate = ResultsTableDelegate::new(gpui_kit::px(16.));
        delegate.set_novels(vec![
            novel_with_read_count(3, Some(30)),
            novel_with_read_count(1, Some(10)),
            novel_with_read_count(2, Some(20)),
        ]);
        let title_col = ResultColumn::ALL
            .iter()
            .position(|column| *column == ResultColumn::Title)
            .expect("title column exists");

        delegate.sort_by_column(title_col, ColumnSort::Ascending);
        assert_eq!(delegate.novel_ids(), vec![1, 2, 3]);

        delegate.restore_original_order();
        assert_eq!(delegate.novel_ids(), vec![3, 1, 2]);
    }

    #[test]
    fn author_link_id_includes_novel_and_author_ids() {
        let mut first = novel_with_read_count(1, Some(10));
        first.author = Author::Known(Title::new("same author".into(), 42));
        let mut second = novel_with_read_count(2, Some(20));
        second.author = Author::Known(Title::new("same author".into(), 42));

        assert_ne!(author_link_id(&first), author_link_id(&second));
        assert_eq!(author_link_id(&first), "author-link-1-42");
        assert_eq!(author_link_id(&second), "author-link-2-42");
    }

    #[test]
    fn fit_budget_guards_oversized_text_and_cache_hits() {
        use gpui_kit::{Font, SharedString, px};
        let mut cache = TextWidthCache::default();
        let mut bytes_left = FIT_MAX_BYTES;
        let oversized: SharedString = "汉".repeat(FIT_MAX_CELL_BYTES / 3 + 1).into();
        assert!(
            cache
                .measure(
                    oversized,
                    Font::default(),
                    px(14.),
                    &mut bytes_left,
                    |_, _, _| panic!("oversized text must not be shaped")
                )
                .is_none()
        );
        assert_eq!(bytes_left, FIT_MAX_BYTES);
        assert!(cache.widths.is_empty());
        let text: SharedString = "a".repeat(FIT_MAX_CELL_BYTES).into();
        let mut shapes = 0;
        for _ in 0..FIT_MAX_BYTES / FIT_MAX_CELL_BYTES {
            assert_eq!(
                cache.measure(
                    text.clone(),
                    Font::default(),
                    px(14.),
                    &mut bytes_left,
                    |_, _, _| {
                        shapes += 1;
                        px(30.)
                    }
                ),
                Some(px(30.))
            );
        }
        assert_eq!(shapes, 1);
        assert_eq!(bytes_left, 0);
        assert!(
            cache
                .measure(
                    text,
                    Font::default(),
                    px(14.),
                    &mut bytes_left,
                    |_, _, _| panic!("exhausted fit must not shape text")
                )
                .is_none()
        );
    }

    #[test]
    fn fit_cache_keys_font_size_and_text_and_bounds_retention() {
        use gpui_kit::{Font, FontWeight, px};
        let mut cache = TextWidthCache::default();
        let mut bytes_left = FIT_MAX_BYTES;
        let font = Font::default();
        assert_eq!(
            cache.measure(
                "same".into(),
                font.clone(),
                px(14.),
                &mut bytes_left,
                |_, _, _| px(10.)
            ),
            Some(px(10.))
        );
        assert_eq!(
            cache.measure(
                "same".into(),
                font.clone(),
                px(14.),
                &mut bytes_left,
                |_, _, _| panic!("cached width should be reused")
            ),
            Some(px(10.))
        );
        assert_eq!(
            cache.measure(
                "same".into(),
                Font {
                    weight: FontWeight::MEDIUM,
                    ..font.clone()
                },
                px(14.),
                &mut bytes_left,
                |_, _, _| px(11.)
            ),
            Some(px(11.))
        );
        assert_eq!(
            cache.measure(
                "same".into(),
                font.clone(),
                px(21.),
                &mut bytes_left,
                |_, _, _| px(15.)
            ),
            Some(px(15.))
        );
        assert_eq!(
            cache.measure(
                "other".into(),
                font.clone(),
                px(14.),
                &mut bytes_left,
                |_, _, _| px(20.)
            ),
            Some(px(20.))
        );
        for id in 0..=FIT_CACHE_ENTRIES {
            cache
                .measure(
                    id.to_string().into(),
                    font.clone(),
                    px(14.),
                    &mut bytes_left,
                    |_, _, _| px(10.),
                )
                .unwrap();
            assert!(cache.widths.len() <= FIT_CACHE_ENTRIES);
        }
    }

    #[gpui_kit::test]
    fn fit_follows_visible_rows_and_caps_only_automatic_width(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::px;
        cx.update(gpui_kit::init);
        let (table, cx) = cx.add_window_view(|window, cx| {
            let mut delegate = ResultsTableDelegate::new(window.rem_size());
            let mut novels = (1..=200)
                .map(|id| novel_with_read_count(id, Some(id)))
                .collect::<Vec<_>>();
            novels.last_mut().unwrap().title.name = "汉".repeat(FIT_MAX_CELL_BYTES * 2).into();
            delegate.set_novels(novels);
            TableState::new(delegate, window, cx).col_resizable(true)
        });
        cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                assert!(!table.visible_range().rows().contains(&199));
                assert!(table.fit_col(0, window, cx));
                assert!(table.col_width(0).unwrap() < window.rem_size() * 15.);
                assert_eq!(table.delegate().column(0, cx).max_width, px(1200.));
                assert!(table.fit_col(4, window, cx));
                // The heading plus sort affordance must fit even when counts are short.
                assert!(table.col_width(4).unwrap() >= px(40.));
                assert!(!table.fit_col(6, window, cx));
                assert!(!table.fit_col(8, window, cx));
                table.scroll_to_row(199, cx);
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                assert!(table.visible_range().rows().contains(&199));
                assert!(table.fit_col(0, window, cx));
                let fitted = table.col_width(0).unwrap();
                assert!(fitted <= window.rem_size() * FIT_MAX_REMS);
                assert!(fitted > window.rem_size() * 40.);
                assert!(
                    table
                        .delegate()
                        .fit_cache
                        .widths
                        .keys()
                        .all(|(text, _, _)| text.len() <= FIT_MAX_CELL_BYTES)
                );
                assert_eq!(table.delegate().column(0, cx).max_width, px(1200.));
            })
        });
    }

    #[gpui_kit::test]
    fn fit_caps_visible_row_work_and_uses_display_order(cx: &mut gpui_kit::TestAppContext) {
        cx.update(gpui_kit::init);
        let (table, cx) = cx.add_window_view(|window, cx| {
            let mut delegate = ResultsTableDelegate::new(window.rem_size());
            let mut novels = (1..=128)
                .map(|id| novel_with_read_count(id, None))
                .collect::<Vec<_>>();
            novels[FIT_MAX_ROWS].title.name = "x".repeat(FIT_MAX_CELL_BYTES + 1).into();
            delegate.set_novels(novels);
            TableState::new(delegate, window, cx).col_resizable(true)
        });
        cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                // Model a tall viewport through the delegate's normal range callback.
                table
                    .delegate_mut()
                    .visible_rows_changed(0..128, window, cx);
                assert!(table.fit_col(0, window, cx));
                assert!(table.col_width(0).unwrap() < window.rem_size() * 15.);
                // The same novel becomes measurable when sorting puts it in range.
                table.delegate_mut().order.swap(0, FIT_MAX_ROWS);
                assert!(table.fit_col(0, window, cx));
                assert!(table.col_width(0).unwrap() > window.rem_size() * 40.);
                assert!(table.col_width(0).unwrap() <= window.rem_size() * FIT_MAX_REMS);
            })
        });
    }

    #[gpui_kit::test]
    fn fit_remeasures_theme_size_and_replacement_results(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::px;
        cx.update(gpui_kit::init);
        let (table, cx) = cx.add_window_view(|window, cx| {
            let mut delegate = ResultsTableDelegate::new(window.rem_size());
            let mut novel = novel_with_read_count(1, Some(1));
            novel.title.name = "A visible title that needs fitting".into();
            delegate.set_novels(vec![novel]);
            TableState::new(delegate, window, cx).col_resizable(true)
        });
        let before = cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                assert!(table.fit_col(0, window, cx));
                table.col_width(0).unwrap()
            })
        });
        cx.update(|window, cx| {
            gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(24.));
            window.set_rem_size(px(24.));
            table.update(cx, |table, cx| {
                table.delegate_mut().rem_size = window.rem_size();
                table.refresh(cx);
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                assert!(table.fit_col(0, window, cx));
                assert!(table.col_width(0).unwrap() > before);
                table
                    .delegate_mut()
                    .set_novels(vec![novel_with_read_count(2, None)]);
                assert!(table.delegate().fit_cache.widths.is_empty());
                table.refresh(cx);
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            table.update(cx, |table, cx| {
                assert!(table.fit_col(0, window, cx));
                assert!(table.col_width(0).unwrap() < before);
                assert!(
                    table
                        .delegate()
                        .fit_cache
                        .widths
                        .keys()
                        .any(|(text, _, _)| text.as_ref() == "title 2")
                );
            })
        });
    }

    #[gpui_kit::test]
    fn sorting_preserves_the_selected_novel(cx: &mut gpui_kit::TestAppContext) {
        cx.update(gpui_kit::init);
        let (table, cx) = cx.add_window_view(|window, cx| {
            let mut delegate = ResultsTableDelegate::new(window.rem_size());
            delegate.set_novels(vec![
                novel_with_read_count(1, Some(10)),
                novel_with_read_count(2, Some(5)),
            ]);
            TableState::new(delegate, window, cx)
        });
        cx.update(|_, cx| table.update(cx, |table, cx| table.set_selected_row(0, cx)));
        let column = ResultColumn::ALL
            .iter()
            .position(|column| *column == ResultColumn::ReadCount)
            .unwrap();
        for sort in [ColumnSort::Ascending, ColumnSort::Default] {
            cx.update(|window, cx| {
                table.update(cx, |table, cx| {
                    let mut delegate = std::mem::replace(
                        table.delegate_mut(),
                        ResultsTableDelegate::new(window.rem_size()),
                    );
                    delegate.perform_sort(column, sort, window, cx);
                    *table.delegate_mut() = delegate;
                })
            });
            cx.run_until_parked();
            cx.update(|_, cx| {
                let table = table.read(cx);
                assert_eq!(
                    table
                        .delegate()
                        .novel_at(table.selected_row().unwrap())
                        .unwrap()
                        .title
                        .id,
                    1
                );
            });
        }
    }

    #[gpui_kit::test]
    fn clicking_sort_icon_preserves_row_selection(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext as _;
        use gpui_kit::test::TestWindowExt as _;

        cx.update(gpui_kit::init);
        let (handle, table) = cx.update(|cx| {
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds::new(
                        Default::default(),
                        gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(400.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut delegate = ResultsTableDelegate::new(window.rem_size());
                        delegate.set_novels(vec![
                            novel_with_read_count(1, Some(10)),
                            novel_with_read_count(2, Some(5)),
                        ]);
                        let mut table = TableState::new(delegate, window, cx);
                        table.set_selected_row(0, cx);
                        table
                    })
                },
            )
            .unwrap()
        });
        let column = ResultColumn::ALL
            .iter()
            .position(|column| *column == ResultColumn::ReadCount)
            .unwrap();
        for expected_order in [vec![1, 2], vec![2, 1], vec![1, 2]] {
            cx.update_window(handle, |_, window, cx| {
                window.click(("icon-sort", column), cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update(|cx| {
                let table = table.read(cx);
                assert_eq!(table.delegate().novel_ids(), expected_order);
                assert_eq!(
                    table
                        .delegate()
                        .novel_at(
                            table
                                .selected_row()
                                .expect("sorting must retain row selection")
                        )
                        .unwrap()
                        .title
                        .id,
                    1
                );
            });
        }
    }

    fn novel_with_read_count(id: i32, read_count: Option<i32>) -> Novel {
        Novel::new(
            Title::new(format!("title {id}").into(), id),
            Author::Anonymous("匿名".into()),
            Title::new(format!("chapter {id}").into(), id),
            "".into(),
            NovelCount::new(id * 1000, read_count, None),
            Default::default(),
            false,
        )
    }

    impl ResultsTableDelegate {
        fn novel_ids(&self) -> Vec<i32> {
            self.order
                .iter()
                .map(|ix| self.novels[*ix].title.id)
                .collect()
        }
    }
}
