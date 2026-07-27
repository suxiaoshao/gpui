use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormItemId(u64);

impl FormItemId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for FormItemId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<FormItemId> for u64 {
    fn from(value: FormItemId) -> Self {
        value.get()
    }
}

impl fmt::Display for FormItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub trait ToFormItemId {
    fn to_form_item_id(&self) -> Option<FormItemId>;
}

impl ToFormItemId for FormItemId {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        Some(*self)
    }
}

impl ToFormItemId for u64 {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        Some(FormItemId::new(*self))
    }
}

impl ToFormItemId for u32 {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        Some(FormItemId::new(u64::from(*self)))
    }
}

impl ToFormItemId for usize {
    fn to_form_item_id(&self) -> Option<FormItemId> {
        u64::try_from(*self).ok().map(FormItemId::new)
    }
}
