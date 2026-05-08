use crate::store::{service::Novel, types::Author};
use gpui::{App, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_component::{
    ActiveTheme, StyledExt,
    label::Label,
    link::Link,
    table::{Column, ColumnFixed, ColumnSort, TableDelegate, TableState},
    tag::Tag as TagComponent,
};

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
    novels: Vec<Novel>,
    loading: bool,
}

impl ResultsTableDelegate {
    pub(crate) fn new() -> Self {
        Self {
            novels: Vec::new(),
            loading: false,
        }
    }

    pub(crate) fn set_novels(&mut self, novels: Vec<Novel>) {
        self.novels = novels;
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    fn column_spec(column: ResultColumn) -> Column {
        match column {
            ResultColumn::Title => Column::new("title", "标题")
                .width(240.)
                .fixed(ColumnFixed::Left)
                .sortable(),
            ResultColumn::Description => Column::new("description", "简介").width(320.),
            ResultColumn::Author => Column::new("author", "作者").width(140.).sortable(),
            ResultColumn::WordCount => Column::new("word_count", "字数")
                .width(96.)
                .text_right()
                .sortable(),
            ResultColumn::ReadCount => Column::new("read_count", "阅读")
                .width(96.)
                .text_right()
                .sortable(),
            ResultColumn::ReplyCount => Column::new("reply_count", "回复")
                .width(96.)
                .text_right()
                .sortable(),
            ResultColumn::IsLimit => Column::new("is_limit", "受限")
                .width(72.)
                .text_center()
                .sortable(),
            ResultColumn::LatestChapter => Column::new("latest_chapter", "最新章节")
                .width(180.)
                .sortable(),
            ResultColumn::Tags => Column::new("tags", "标签").width(360.),
        }
    }

    fn novel_at(&self, row_ix: usize) -> Option<&Novel> {
        self.novels.get(row_ix)
    }

    fn author_label(novel: &Novel) -> String {
        match &novel.author {
            Author::Anonymous(name) => name.clone(),
            Author::Known(title) => title.name.clone(),
        }
    }

    fn sort_by_column(&mut self, col_ix: usize, sort: ColumnSort) {
        let descending = matches!(sort, ColumnSort::Descending);
        match ResultColumn::ALL.get(col_ix).copied() {
            Some(ResultColumn::Title) => self.sort_by(descending, |novel| novel.title.name.clone()),
            Some(ResultColumn::Description) => {}
            Some(ResultColumn::Author) => self.sort_by(descending, Self::author_label),
            Some(ResultColumn::WordCount) => {
                self.sort_by(descending, |novel| Some(novel.count.word_count))
            }
            Some(ResultColumn::ReadCount) => {
                self.sort_by_missing_last(descending, |novel| novel.count.read_count)
            }
            Some(ResultColumn::ReplyCount) => {
                self.sort_by_missing_last(descending, |novel| novel.count.reply_count)
            }
            Some(ResultColumn::IsLimit) => self.sort_by(descending, |novel| novel.is_limit),
            Some(ResultColumn::LatestChapter) => {
                self.sort_by(descending, |novel| novel.latest_chapter.name.clone())
            }
            Some(ResultColumn::Tags) | None => {}
        }
    }

    fn sort_by<T, F>(&mut self, descending: bool, value: F)
    where
        T: Ord,
        F: Fn(&Novel) -> T,
    {
        self.novels.sort_by(|left, right| {
            let ordering = value(left).cmp(&value(right));
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    fn sort_by_missing_last<T, F>(&mut self, descending: bool, value: F)
    where
        T: Ord,
        F: Fn(&Novel) -> Option<T>,
    {
        self.novels
            .sort_by(|left, right| match (value(left), value(right)) {
                (Some(left), Some(right)) => {
                    let ordering = left.cmp(&right);
                    if descending {
                        ordering.reverse()
                    } else {
                        ordering
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
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
            .map(Self::column_spec)
            .unwrap_or_else(|| Column::new("unknown", ""))
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        if sort == ColumnSort::Default {
            self.novels.sort_by_key(|novel| novel.title.id);
        } else {
            self.sort_by_column(col_ix, sort);
        }
        cx.notify();
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
                    Some(url) => Link::new((
                        "author-link",
                        author_id(&novel.author).unwrap_or_default() as u64,
                    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::types::{NovelCount, Title};

    #[test]
    fn loading_defaults_to_false_and_can_be_toggled() {
        let mut delegate = ResultsTableDelegate::new();

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
        let author = Author::Known(crate::store::types::Title {
            name: "作者".to_owned(),
            id: 538220,
        });

        assert_eq!(
            author_url(&author).as_deref(),
            Some("https://xn--pxtr7m.com/users/538220")
        );
    }

    #[test]
    fn anonymous_author_has_no_url() {
        assert_eq!(author_url(&Author::Anonymous("匿名".to_owned())), None);
    }

    #[test]
    fn optional_count_sorts_keep_missing_values_last() {
        let mut delegate = ResultsTableDelegate::new();
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

    fn novel_with_read_count(id: i32, read_count: Option<i32>) -> Novel {
        Novel {
            title: Title {
                name: format!("title {id}"),
                id,
            },
            author: Author::Anonymous("匿名".to_owned()),
            latest_chapter: Title {
                name: format!("chapter {id}"),
                id,
            },
            desc: String::new(),
            count: NovelCount {
                word_count: id * 1000,
                read_count,
                reply_count: None,
            },
            tags: Default::default(),
            is_limit: false,
        }
    }

    impl ResultsTableDelegate {
        fn novel_ids(&self) -> Vec<i32> {
            self.novels.iter().map(|novel| novel.title.id).collect()
        }
    }
}
