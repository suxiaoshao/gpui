use std::{borrow::Cow, fmt};

use crate::array::FormItemId;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath {
    segments: Vec<FieldPathSegment>,
}

impl FieldPath {
    pub const fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn field(name: &'static str) -> Self {
        Self::from_segments([FieldPathSegment::Field(Cow::Borrowed(name))])
    }

    pub fn from_segments(segments: impl IntoIterator<Item = FieldPathSegment>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
        }
    }

    pub fn segments(&self) -> &[FieldPathSegment] {
        &self.segments
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn join_field(&self, name: &'static str) -> Self {
        self.join_segment(FieldPathSegment::Field(Cow::Borrowed(name)))
    }

    pub fn join_owned_field(&self, name: impl Into<String>) -> Self {
        self.join_segment(FieldPathSegment::Field(Cow::Owned(name.into())))
    }

    pub fn join_item(&self, id: FormItemId) -> Self {
        self.join_segment(FieldPathSegment::Item(id))
    }

    pub fn join_projection(&self, name: &'static str) -> Self {
        self.join_segment(FieldPathSegment::Projection(Cow::Borrowed(name)))
    }

    pub fn join_path(&self, suffix: &Self) -> Self {
        let mut segments = self.segments.clone();
        segments.extend(suffix.segments.iter().cloned());
        Self { segments }
    }

    pub fn join_segment(&self, segment: FieldPathSegment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self { segments }
    }

    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.segments.starts_with(&prefix.segments)
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            return f.write_str("$");
        }

        for (index, segment) in self.segments.iter().enumerate() {
            match segment {
                FieldPathSegment::Field(name) => {
                    if index > 0 {
                        f.write_str(".")?;
                    }
                    f.write_str(name)?;
                }
                FieldPathSegment::Item(id) => write!(f, "[#{}]", id.get())?,
                FieldPathSegment::Projection(name) => write!(f, "::<{name}>")?,
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldPathSegment {
    Field(Cow<'static, str>),
    Item(FormItemId),
    Projection(Cow<'static, str>),
}
