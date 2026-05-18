use crate::{
    errors::FeiwenResult,
    foundation::field_matches_query,
    store::{
        query::{AuthorRef, BoolField, NumberField, SortExpr, TextField},
        service::Tag,
    },
};
use duckdb::Connection;
use gpui::{AnyElement, IntoElement, SharedString};
use gpui_component::select::{SearchableVec, SelectItem};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FieldKind {
    Title,
    Description,
    LatestChapterTitle,
    Author,
    Tags,
    WordCount,
    ReadCount,
    ReplyCount,
    IsLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextRelation {
    Contains,
    StartsWith,
    EndsWith,
    Equals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NumberRelation {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Between,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BoolRelation {
    Is,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TagsRelation {
    Intersects,
    ContainsAll,
    ContainedBy,
    Equals,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AuthorRelation {
    NameContains,
    NameStartsWith,
    NameEndsWith,
    NameEquals,
    Is,
    IsNot,
    In,
    NotIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroupRelation {
    All,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SortField {
    Title,
    AuthorName,
    NovelId,
    LatestChapterId,
    LatestChapterTitle,
    WordCount,
    ReadCount,
    ReplyCount,
    AuthorId,
    IsLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SortDirectionChoice {
    Asc,
    Desc,
}

#[derive(Clone)]
pub(super) struct SelectChoice<T: Copy + Eq + 'static> {
    label: &'static str,
    value: T,
}

pub(super) type FieldSelectItems = SearchableVec<SelectChoice<FieldKind>>;

impl<T: Copy + Eq + 'static> SelectChoice<T> {
    pub(super) fn new(label: &'static str, value: T) -> Self {
        Self { label, value }
    }
}

impl<T: Copy + Eq + 'static> SelectItem for SelectChoice<T> {
    type Value = T;

    fn title(&self) -> SharedString {
        self.label.into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(self.label.into_any_element())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn matches(&self, query: &str) -> bool {
        field_matches_query(self.label, query)
    }
}

impl FieldKind {
    pub(super) fn text_field(self) -> Option<TextField> {
        match self {
            Self::Title => Some(TextField::Title),
            Self::Description => Some(TextField::Description),
            Self::LatestChapterTitle => Some(TextField::LatestChapter),
            _ => None,
        }
    }

    pub(super) fn number_field(self) -> Option<NumberField> {
        match self {
            Self::WordCount => Some(NumberField::WordCount),
            Self::ReadCount => Some(NumberField::ReadCount),
            Self::ReplyCount => Some(NumberField::ReplyCount),
            _ => None,
        }
    }
}

impl TagsRelation {
    pub(super) fn needs_value(self) -> bool {
        !matches!(self, Self::IsEmpty | Self::IsNotEmpty)
    }
}

impl SortField {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Title => "标题",
            Self::AuthorName => "作者名称",
            Self::NovelId => "作品 ID",
            Self::LatestChapterId => "最新章节 ID",
            Self::LatestChapterTitle => "最新章节标题",
            Self::WordCount => "字数",
            Self::ReadCount => "阅读数",
            Self::ReplyCount => "回复数",
            Self::AuthorId => "作者 ID",
            Self::IsLimit => "是否受限",
        }
    }

    pub(super) fn sort_expr(self) -> SortExpr {
        match self {
            Self::Title => SortExpr::Text(TextField::Title),
            Self::AuthorName => SortExpr::Text(TextField::AuthorName),
            Self::NovelId => SortExpr::Number(NumberField::NovelId),
            Self::LatestChapterId => SortExpr::Number(NumberField::LatestChapterId),
            Self::LatestChapterTitle => SortExpr::Text(TextField::LatestChapter),
            Self::WordCount => SortExpr::Number(NumberField::WordCount),
            Self::ReadCount => SortExpr::Number(NumberField::ReadCount),
            Self::ReplyCount => SortExpr::Number(NumberField::ReplyCount),
            Self::AuthorId => SortExpr::Number(NumberField::AuthorId),
            Self::IsLimit => SortExpr::Bool(BoolField::IsLimit),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct QueryOptions {
    pub(super) tags: Vec<TagOption>,
    pub(super) authors: Vec<AuthorOption>,
}

struct AuthorOptionRow {
    author_id: Option<i32>,
    author_name: String,
}

impl QueryOptions {
    pub(crate) fn load(conn: &Connection) -> FeiwenResult<Self> {
        let tags = Tag::tags_with_id(conn)?
            .into_iter()
            .map(|tag| TagOption {
                id: tag.id,
                name: tag.name,
            })
            .collect();
        let mut authors = HashMap::new();
        for row in load_author_rows(conn)? {
            let author_ref = match row.author_id {
                Some(id) => AuthorRef::Id(id),
                None => AuthorRef::Name(row.author_name.clone()),
            };
            authors
                .entry(author_ref.clone())
                .or_insert_with(|| AuthorOption {
                    author: author_ref,
                    name: row.author_name,
                });
        }
        let mut authors = authors.into_values().collect::<Vec<_>>();
        sort_author_options(&mut authors);
        Ok(Self { tags, authors })
    }
}

fn load_author_rows(conn: &Connection) -> FeiwenResult<Vec<AuthorOptionRow>> {
    let mut stmt = conn.prepare(
        "\
        SELECT author_id, author_name \
        FROM novel \
        GROUP BY author_id, author_name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AuthorOptionRow {
            author_id: row.get(0)?,
            author_name: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[derive(Clone)]
pub(super) struct TagOption {
    pub(super) id: i32,
    pub(super) name: String,
}

#[derive(Clone)]
pub(super) struct AuthorOption {
    pub(super) author: AuthorRef,
    pub(super) name: String,
}

impl SelectItem for TagOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }

    fn matches(&self, query: &str) -> bool {
        option_matches(&self.name, &format!("标签 ID {}", self.id), query)
    }
}

impl SelectItem for AuthorOption {
    type Value = AuthorRef;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.author
    }

    fn matches(&self, query: &str) -> bool {
        field_matches_query(&self.name, query)
    }
}

fn sort_author_options(authors: &mut [AuthorOption]) {
    authors.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            author_ref_order_key(&left.author).cmp(&author_ref_order_key(&right.author))
        })
    });
}

fn author_ref_order_key(author: &AuthorRef) -> (u8, i32, &str) {
    match author {
        AuthorRef::Id(id) => (0, *id, ""),
        AuthorRef::Name(name) => (1, 0, name.as_str()),
    }
}

fn option_matches(title: &str, description: &str, query: &str) -> bool {
    field_matches_query(title, query) || field_matches_query(description, query)
}

pub(super) fn field_items() -> FieldSelectItems {
    SearchableVec::new(vec![
        SelectChoice::new("标题", FieldKind::Title),
        SelectChoice::new("简介", FieldKind::Description),
        SelectChoice::new("最新章节标题", FieldKind::LatestChapterTitle),
        SelectChoice::new("作者", FieldKind::Author),
        SelectChoice::new("标签", FieldKind::Tags),
        SelectChoice::new("字数", FieldKind::WordCount),
        SelectChoice::new("阅读数", FieldKind::ReadCount),
        SelectChoice::new("回复数", FieldKind::ReplyCount),
        SelectChoice::new("是否受限", FieldKind::IsLimit),
    ])
}

pub(super) fn text_relation_items() -> Vec<SelectChoice<TextRelation>> {
    vec![
        SelectChoice::new("包含", TextRelation::Contains),
        SelectChoice::new("开头是", TextRelation::StartsWith),
        SelectChoice::new("结尾是", TextRelation::EndsWith),
        SelectChoice::new("等于", TextRelation::Equals),
    ]
}

pub(super) fn number_relation_items() -> Vec<SelectChoice<NumberRelation>> {
    vec![
        SelectChoice::new("等于", NumberRelation::Eq),
        SelectChoice::new("不等于", NumberRelation::Ne),
        SelectChoice::new("小于", NumberRelation::Lt),
        SelectChoice::new("小于等于", NumberRelation::Lte),
        SelectChoice::new("大于", NumberRelation::Gt),
        SelectChoice::new("大于等于", NumberRelation::Gte),
        SelectChoice::new("介于范围", NumberRelation::Between),
    ]
}

pub(super) fn bool_relation_items() -> Vec<SelectChoice<BoolRelation>> {
    vec![SelectChoice::new("是否", BoolRelation::Is)]
}

pub(super) fn bool_value_items() -> Vec<SelectChoice<bool>> {
    vec![
        SelectChoice::new("是", true),
        SelectChoice::new("否", false),
    ]
}

pub(super) fn tags_relation_items() -> Vec<SelectChoice<TagsRelation>> {
    vec![
        SelectChoice::new("有交集", TagsRelation::Intersects),
        SelectChoice::new("包含全部", TagsRelation::ContainsAll),
        SelectChoice::new("被集合包含", TagsRelation::ContainedBy),
        SelectChoice::new("集合相等", TagsRelation::Equals),
        SelectChoice::new("为空", TagsRelation::IsEmpty),
        SelectChoice::new("不为空", TagsRelation::IsNotEmpty),
    ]
}

pub(super) fn author_relation_items() -> Vec<SelectChoice<AuthorRelation>> {
    vec![
        SelectChoice::new("名称包含", AuthorRelation::NameContains),
        SelectChoice::new("名称开头是", AuthorRelation::NameStartsWith),
        SelectChoice::new("名称结尾是", AuthorRelation::NameEndsWith),
        SelectChoice::new("名称等于", AuthorRelation::NameEquals),
        SelectChoice::new("是", AuthorRelation::Is),
        SelectChoice::new("不是", AuthorRelation::IsNot),
        SelectChoice::new("在集合中", AuthorRelation::In),
        SelectChoice::new("不在集合中", AuthorRelation::NotIn),
    ]
}

pub(super) fn sort_field_items() -> Vec<SelectChoice<SortField>> {
    vec![
        SelectChoice::new("标题", SortField::Title),
        SelectChoice::new("作者名称", SortField::AuthorName),
        SelectChoice::new("作品 ID", SortField::NovelId),
        SelectChoice::new("最新章节 ID", SortField::LatestChapterId),
        SelectChoice::new("最新章节标题", SortField::LatestChapterTitle),
        SelectChoice::new("字数", SortField::WordCount),
        SelectChoice::new("阅读数", SortField::ReadCount),
        SelectChoice::new("回复数", SortField::ReplyCount),
        SelectChoice::new("作者 ID", SortField::AuthorId),
        SelectChoice::new("是否受限", SortField::IsLimit),
    ]
}

pub(super) fn sort_direction_items() -> Vec<SelectChoice<SortDirectionChoice>> {
    vec![
        SelectChoice::new("升序", SortDirectionChoice::Asc),
        SelectChoice::new("降序", SortDirectionChoice::Desc),
    ]
}

#[cfg(test)]
mod query_options_load_tests {
    use super::*;
    use crate::store::{
        initialize_schema,
        service::{Novel, Tag},
        types::{Author, NovelCount, Title},
    };
    use duckdb::Connection;
    use std::collections::HashSet;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        conn
    }

    fn novel(id: i32, author: Author, tags: &[&str]) -> Novel {
        Novel {
            title: Title {
                name: format!("novel-{id}"),
                id,
            },
            author,
            latest_chapter: Title {
                name: "chapter".to_owned(),
                id: id * 10,
            },
            desc: "desc".to_owned(),
            count: NovelCount {
                word_count: 1000,
                read_count: Some(100),
                reply_count: Some(10),
            },
            tags: tags
                .iter()
                .map(|name| Tag {
                    name: (*name).to_owned(),
                    id: Some(id),
                })
                .collect::<HashSet<_>>(),
            is_limit: false,
        }
    }

    #[test]
    fn query_options_loads_author_choices_without_full_novel_rows() {
        let mut conn = connection();
        novel(
            1,
            Author::Known(Title {
                name: "张三".to_owned(),
                id: 10,
            }),
            &["rust", "gpui"],
        )
        .save(&mut conn)
        .unwrap();
        novel(
            2,
            Author::Known(Title {
                name: "张三".to_owned(),
                id: 10,
            }),
            &["rust"],
        )
        .save(&mut conn)
        .unwrap();
        novel(
            3,
            Author::Anonymous("匿名".to_owned()),
            &["rust", "匿名标签"],
        )
        .save(&mut conn)
        .unwrap();

        let options = QueryOptions::load(&conn).unwrap();

        assert_eq!(
            options
                .authors
                .iter()
                .map(|author| (author.name.clone(), author.author.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("匿名".to_owned(), AuthorRef::Name("匿名".to_owned())),
                ("张三".to_owned(), AuthorRef::Id(10)),
            ]
        );
        let tag_names = options
            .tags
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(tag_names.first(), Some(&"rust"));
        assert!(tag_names.contains(&"gpui"));
        assert!(tag_names.contains(&"匿名标签"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_component::{IndexPath, select::SelectDelegate};

    #[test]
    fn tag_option_matches_pinyin_and_initials() {
        let option = TagOption {
            id: 1,
            name: "中篇".to_owned(),
        };

        assert!(option.matches("zhongpian"));
        assert!(option.matches("zp"));
        assert!(option.matches("标签 ID 1"));
        assert!(!option.matches("changpian"));
    }

    #[test]
    fn select_choice_matches_pinyin_and_initials() {
        let option = SelectChoice::new("包含全部", TagsRelation::ContainsAll);

        assert!(option.matches("baohan"));
        assert!(option.matches("bhqb"));
        assert!(!option.matches("dengyu"));
    }

    #[test]
    fn field_items_are_flat_and_match_prd_order() {
        let items = field_items();
        let labels = (0..items.items_count(0))
            .map(|row| {
                items
                    .item(IndexPath::default().row(row))
                    .expect("field item should exist")
                    .title()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "标题",
                "简介",
                "最新章节标题",
                "作者",
                "标签",
                "字数",
                "阅读数",
                "回复数",
                "是否受限",
            ]
        );
        assert!(!labels.iter().any(|label| matches!(
            label.as_str(),
            "作品 ID" | "最新章节 ID" | "作者 ID" | "作者名称"
        )));
    }

    #[test]
    fn author_relation_items_cover_text_and_entity_conditions() {
        let items = author_relation_items();
        let labels = items
            .iter()
            .map(|item| item.title().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "名称包含",
                "名称开头是",
                "名称结尾是",
                "名称等于",
                "是",
                "不是",
                "在集合中",
                "不在集合中",
            ]
        );
        assert!(items[0].matches("mingcheng"));
        assert!(items[0].matches("mcbh"));
        assert!(items[5].matches("bushi"));
        assert!(items[5].matches("bs"));
    }

    #[test]
    fn author_options_sort_by_name_then_stable_author_ref() {
        let mut authors = vec![
            AuthorOption {
                author: AuthorRef::Name("anonymous-b".to_owned()),
                name: "bravo".to_owned(),
            },
            AuthorOption {
                author: AuthorRef::Id(20),
                name: "alpha".to_owned(),
            },
            AuthorOption {
                author: AuthorRef::Id(10),
                name: "alpha".to_owned(),
            },
            AuthorOption {
                author: AuthorRef::Name("anonymous-a".to_owned()),
                name: "alpha".to_owned(),
            },
        ];

        sort_author_options(&mut authors);

        assert_eq!(
            authors
                .into_iter()
                .map(|author| author.author)
                .collect::<Vec<_>>(),
            vec![
                AuthorRef::Id(10),
                AuthorRef::Id(20),
                AuthorRef::Name("anonymous-a".to_owned()),
                AuthorRef::Name("anonymous-b".to_owned()),
            ]
        );
    }
}
