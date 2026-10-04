use feiwen_data::{service::Novel, types::Author};
use gpui_kit::component::{
    ActiveTheme, StyledExt,
    label::Label,
    link::Link,
    table::{Column, ColumnFixed, ColumnSort, TableDelegate, TableState},
    tag::Tag as TagComponent,
};
use gpui_kit::{App, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div};

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
}

impl ResultsTableDelegate {
    pub(crate) fn new(rem_size: gpui_kit::Pixels) -> Self {
        Self {
            rem_size,
            novels: Vec::new(),
            order: Vec::new(),
            loading: false,
        }
    }

    pub(crate) fn set_novels(&mut self, novels: Vec<Novel>) {
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

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
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

fn number_cell(value: Option<i32>) -> impl IntoElement {
    Label::new(
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_owned()),
    )
    .text_sm()
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
