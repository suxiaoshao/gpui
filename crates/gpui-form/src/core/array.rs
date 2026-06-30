use std::{fmt, num::NonZeroU64};

use crate::{FieldError, FieldMeta, FieldPath, SubscriptionSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormItemId(u64);

impl FormItemId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn non_zero(self) -> Option<NonZeroU64> {
        NonZeroU64::new(self.0)
    }
}

impl fmt::Display for FormItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormItemIdGenerator {
    next: u64,
}

impl Default for FormItemIdGenerator {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl FormItemIdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_next(next: u64) -> Self {
        Self { next }
    }

    pub fn peek(&self) -> FormItemId {
        FormItemId(self.next)
    }

    pub fn generate(&mut self) -> FormItemId {
        let id = FormItemId(self.next);
        self.next = self.next.checked_add(1).expect("form item id overflowed");
        id
    }
}

#[derive(Debug)]
pub struct FieldArrayItem<Item> {
    pub id: FormItemId,
    pub index: usize,
    pub item: Item,
    subscriptions: SubscriptionSet,
}

impl<Item> FieldArrayItem<Item> {
    pub fn new(id: FormItemId, index: usize, item: Item) -> Self {
        Self {
            id,
            index,
            item,
            subscriptions: SubscriptionSet::default(),
        }
    }

    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut SubscriptionSet {
        &mut self.subscriptions
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormRowValue<T> {
    pub id: FormItemId,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayIndexError {
    pub index: usize,
    pub len: usize,
}

impl fmt::Display for ArrayIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "array index {} is out of bounds for length {}",
            self.index, self.len
        )
    }
}

impl std::error::Error for ArrayIndexError {}

#[derive(Debug)]
pub struct FieldArrayStore<Item> {
    path: FieldPath,
    items: Vec<FieldArrayItem<Item>>,
    id_generator: FormItemIdGenerator,
    array_revision: u64,
    meta: FieldMeta,
    required: bool,
    errors: Vec<FieldError>,
    subscriptions: SubscriptionSet,
}

impl<Item> FieldArrayStore<Item> {
    pub fn new(path: impl Into<FieldPath>, items: impl IntoIterator<Item = Item>) -> Self {
        let mut generator = FormItemIdGenerator::default();
        Self::from_items_with_generator(path, items, &mut generator)
    }

    pub fn from_items_with_generator(
        path: impl Into<FieldPath>,
        items: impl IntoIterator<Item = Item>,
        generator: &mut FormItemIdGenerator,
    ) -> Self {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| FieldArrayItem::new(generator.generate(), index, item))
            .collect();

        Self {
            path: path.into(),
            items,
            id_generator: generator.clone(),
            array_revision: 0,
            meta: FieldMeta::default(),
            required: false,
            errors: Vec::new(),
            subscriptions: SubscriptionSet::default(),
        }
    }

    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    pub fn items(&self) -> &[FieldArrayItem<Item>] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [FieldArrayItem<Item>] {
        &mut self.items
    }

    pub fn item(&self, id: FormItemId) -> Option<&FieldArrayItem<Item>> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_mut(&mut self, id: FormItemId) -> Option<&mut FieldArrayItem<Item>> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn index_of(&self, id: FormItemId) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }

    pub fn ids(&self) -> Vec<FormItemId> {
        self.items.iter().map(|item| item.id).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn meta(&self) -> &FieldMeta {
        &self.meta
    }

    pub fn meta_mut(&mut self) -> &mut FieldMeta {
        &mut self.meta
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn set_required(&mut self, required: bool) {
        self.required = required;
    }

    pub fn set_meta(&mut self, meta: FieldMeta) {
        self.meta = meta;
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    pub fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.meta.set_valid(errors.is_empty());
        self.errors = errors;
    }

    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut SubscriptionSet {
        &mut self.subscriptions
    }

    pub fn array_revision(&self) -> u64 {
        self.array_revision
    }

    pub fn append(&mut self, item: Item) -> FormItemId {
        let id = self.id_generator.generate();
        let index = self.items.len();
        self.items.push(FieldArrayItem::new(id, index, item));
        self.bump_revision();
        id
    }

    pub fn insert(&mut self, index: usize, item: Item) -> Result<FormItemId, ArrayIndexError> {
        if index > self.items.len() {
            return Err(ArrayIndexError {
                index,
                len: self.items.len(),
            });
        }

        let id = self.id_generator.generate();
        self.items
            .insert(index, FieldArrayItem::new(id, index, item));
        self.reindex();
        self.bump_revision();
        Ok(id)
    }

    pub fn remove(&mut self, index: usize) -> Result<FieldArrayItem<Item>, ArrayIndexError> {
        if index >= self.items.len() {
            return Err(ArrayIndexError {
                index,
                len: self.items.len(),
            });
        }

        let removed = self.items.remove(index);
        self.reindex();
        self.bump_revision();
        Ok(removed)
    }

    pub fn remove_id(&mut self, id: FormItemId) -> Option<FieldArrayItem<Item>> {
        let index = self.index_of(id)?;
        self.remove(index).ok()
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> Result<(), ArrayIndexError> {
        let len = self.items.len();
        if from >= len {
            return Err(ArrayIndexError { index: from, len });
        }
        if to >= len {
            return Err(ArrayIndexError { index: to, len });
        }
        if from == to {
            return Ok(());
        }

        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.reindex();
        self.bump_revision();
        Ok(())
    }

    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), ArrayIndexError> {
        let len = self.items.len();
        if a >= len {
            return Err(ArrayIndexError { index: a, len });
        }
        if b >= len {
            return Err(ArrayIndexError { index: b, len });
        }
        if a == b {
            return Ok(());
        }

        self.items.swap(a, b);
        self.reindex();
        self.bump_revision();
        Ok(())
    }

    pub fn replace_item(
        &mut self,
        index: usize,
        item: Item,
    ) -> Result<FormItemId, ArrayIndexError> {
        if index >= self.items.len() {
            return Err(ArrayIndexError {
                index,
                len: self.items.len(),
            });
        }

        let id = self.id_generator.generate();
        self.items[index] = FieldArrayItem::new(id, index, item);
        self.bump_revision();
        Ok(id)
    }

    pub fn replace(&mut self, items: impl IntoIterator<Item = Item>) {
        self.items.clear();
        self.id_generator = FormItemIdGenerator::default();
        let mut next_items = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            next_items.push(FieldArrayItem::new(
                self.id_generator.generate(),
                index,
                item,
            ));
        }
        self.items = next_items;
        self.meta = FieldMeta::default();
        self.errors.clear();
        self.bump_revision();
    }

    pub fn reset(&mut self, items: impl IntoIterator<Item = Item>) {
        self.replace(items);
    }

    pub fn clear_errors(&mut self) {
        self.errors.clear();
        self.meta.set_valid(true);
    }

    fn reindex(&mut self) {
        for (index, item) in self.items.iter_mut().enumerate() {
            item.index = index;
        }
    }

    fn bump_revision(&mut self) {
        self.array_revision = self.array_revision.saturating_add(1);
        self.meta.mark_touched();
        self.meta.mark_dirty(false);
    }
}
