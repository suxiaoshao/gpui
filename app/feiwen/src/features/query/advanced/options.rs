use crate::{
    errors::FeiwenResult,
    foundation::field_matches_query,
    store::{
        model::NovelModel,
        query::{AuthorRef, BoolField, NumberField, SortExpr, TextField},
        service::Tag,
    },
};
use diesel::SqliteConnection;
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui_component::{
    label::Label,
    select::{SearchableVec, SelectGroup, SelectItem},
    v_flex,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FieldKind {
    Title,
    Description,
    LatestChapterTitle,
    AuthorName,
    NovelId,
    LatestChapterId,
    AuthorId,
    WordCount,
    ReadCount,
    ReplyCount,
    IsLimit,
    Tags,
    Author,
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
pub(super) enum IdRelation {
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
    Is,
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

pub(super) type FieldSelectItems = SearchableVec<SelectGroup<SelectChoice<FieldKind>>>;

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
            Self::AuthorName => Some(TextField::AuthorName),
            _ => None,
        }
    }

    pub(super) fn number_field(self) -> Option<NumberField> {
        match self {
            Self::NovelId => Some(NumberField::NovelId),
            Self::LatestChapterId => Some(NumberField::LatestChapterId),
            Self::AuthorId => Some(NumberField::AuthorId),
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
    pub(super) novels: Vec<IdOption>,
    pub(super) chapters: Vec<IdOption>,
    pub(super) author_ids: Vec<IdOption>,
}

impl QueryOptions {
    pub(crate) fn load(conn: &mut SqliteConnection) -> FeiwenResult<Self> {
        let tags = Tag::tags_with_id(conn)?
            .into_iter()
            .map(|tag| TagOption {
                id: tag.id,
                name: tag.name,
            })
            .collect();
        let novels = NovelModel::query(conn)?;
        let mut authors = HashMap::new();
        let mut novel_options = Vec::new();
        let mut chapter_options = Vec::new();
        let mut author_id_options = HashMap::new();
        for novel in novels {
            novel_options.push(IdOption {
                id: novel.id,
                label: novel.name.clone(),
                description: format!("作品 ID {}", novel.id),
            });
            chapter_options.push(IdOption {
                id: novel.latest_chapter_id,
                label: novel.latest_chapter_name.clone(),
                description: format!("最新章节 ID {}", novel.latest_chapter_id),
            });
            let author_ref = match novel.author_id {
                Some(id) => {
                    author_id_options.entry(id).or_insert_with(|| IdOption {
                        id,
                        label: novel.author_name.clone(),
                        description: format!("作者 ID {id}"),
                    });
                    AuthorRef::Id(id)
                }
                None => AuthorRef::Name(novel.author_name.clone()),
            };
            authors
                .entry(author_ref.clone())
                .or_insert_with(|| AuthorOption {
                    author: author_ref,
                    name: novel.author_name,
                });
        }
        Ok(Self {
            tags,
            authors: authors.into_values().collect(),
            novels: novel_options,
            chapters: chapter_options,
            author_ids: author_id_options.into_values().collect(),
        })
    }
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

#[derive(Clone)]
pub(super) struct IdOption {
    pub(super) id: i32,
    pub(super) label: String,
    pub(super) description: String,
}

impl SelectItem for TagOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        option_content(self.title(), format!("标签 ID {}", self.id).into())
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

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        option_content(self.title(), self.description().into())
    }

    fn value(&self) -> &Self::Value {
        &self.author
    }

    fn matches(&self, query: &str) -> bool {
        option_matches(&self.name, &self.description(), query)
    }
}

impl AuthorOption {
    fn description(&self) -> String {
        match &self.author {
            AuthorRef::Id(id) => format!("作者 ID {id}"),
            AuthorRef::Name(_) => "匿名作者".to_owned(),
        }
    }
}

impl SelectItem for IdOption {
    type Value = i32;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        option_content(self.title(), self.description.clone().into())
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }

    fn matches(&self, query: &str) -> bool {
        option_matches(&self.label, &self.description, query)
    }
}

fn option_content(title: SharedString, description: SharedString) -> impl IntoElement {
    v_flex()
        .min_w_0()
        .gap_0p5()
        .child(Label::new(title).text_sm())
        .child(Label::new(description).text_xs())
}

fn option_matches(title: &str, description: &str, query: &str) -> bool {
    field_matches_query(title, query) || field_matches_query(description, query)
}

pub(super) fn field_items() -> FieldSelectItems {
    SearchableVec::new(vec![
        SelectGroup::new("文本字段").items([
            SelectChoice::new("标题", FieldKind::Title),
            SelectChoice::new("简介", FieldKind::Description),
            SelectChoice::new("最新章节标题", FieldKind::LatestChapterTitle),
            SelectChoice::new("作者名称", FieldKind::AuthorName),
        ]),
        SelectGroup::new("ID 字段").items([
            SelectChoice::new("作品 ID", FieldKind::NovelId),
            SelectChoice::new("最新章节 ID", FieldKind::LatestChapterId),
            SelectChoice::new("作者 ID", FieldKind::AuthorId),
        ]),
        SelectGroup::new("数字字段").items([
            SelectChoice::new("字数", FieldKind::WordCount),
            SelectChoice::new("阅读数", FieldKind::ReadCount),
            SelectChoice::new("回复数", FieldKind::ReplyCount),
        ]),
        SelectGroup::new("布尔字段").item(SelectChoice::new("是否受限", FieldKind::IsLimit)),
        SelectGroup::new("集合字段").item(SelectChoice::new("标签", FieldKind::Tags)),
        SelectGroup::new("作者字段").item(SelectChoice::new("作者", FieldKind::Author)),
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

pub(super) fn id_relation_items() -> Vec<SelectChoice<IdRelation>> {
    vec![
        SelectChoice::new("等于", IdRelation::Eq),
        SelectChoice::new("不等于", IdRelation::Ne),
        SelectChoice::new("小于", IdRelation::Lt),
        SelectChoice::new("小于等于", IdRelation::Lte),
        SelectChoice::new("大于", IdRelation::Gt),
        SelectChoice::new("大于等于", IdRelation::Gte),
        SelectChoice::new("介于范围", IdRelation::Between),
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
        SelectChoice::new("是", AuthorRelation::Is),
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
mod tests {
    use super::*;

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
}
