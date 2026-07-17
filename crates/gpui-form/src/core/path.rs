use std::{borrow::Cow, fmt};

use super::array::FormItemId;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath {
    segments: Vec<FieldPathSegment>,
}

impl FieldPath {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn from_static(field: &'static str) -> Self {
        Self {
            segments: vec![FieldPathSegment::field(field)],
        }
    }

    pub fn field(field: &'static str) -> Self {
        Self::from_static(field)
    }

    pub fn from_segments(segments: impl IntoIterator<Item = FieldPathSegment>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
        }
    }

    pub fn parse_lossy(path: impl AsRef<str>) -> Self {
        let path = path.as_ref();
        if path.is_empty() {
            return Self::root();
        }

        let mut segments = Vec::new();
        let mut field = String::new();
        let mut index = String::new();
        let mut in_index = false;

        for ch in path.chars() {
            match (ch, in_index) {
                ('.', false) => {
                    if !field.is_empty() {
                        segments.push(FieldPathSegment::Field(Cow::Owned(std::mem::take(
                            &mut field,
                        ))));
                    }
                }
                ('[', false) => {
                    if !field.is_empty() {
                        segments.push(FieldPathSegment::Field(Cow::Owned(std::mem::take(
                            &mut field,
                        ))));
                    }
                    in_index = true;
                }
                (']', true) => {
                    if let Ok(index) = index.parse::<usize>() {
                        segments.push(FieldPathSegment::Index(index));
                    } else if !index.is_empty() {
                        segments.push(FieldPathSegment::Field(Cow::Owned(std::mem::take(
                            &mut index,
                        ))));
                    }
                    index.clear();
                    in_index = false;
                }
                (_, true) => index.push(ch),
                _ => field.push(ch),
            }
        }

        if !field.is_empty() {
            segments.push(FieldPathSegment::Field(Cow::Owned(field)));
        }
        if !index.is_empty() {
            segments.push(FieldPathSegment::Field(Cow::Owned(index)));
        }

        Self { segments }
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn segments(&self) -> &[FieldPathSegment] {
        &self.segments
    }

    pub fn into_segments(self) -> Vec<FieldPathSegment> {
        self.segments
    }

    pub fn join_field(&self, field: &'static str) -> Self {
        self.join_segment(FieldPathSegment::field(field))
    }

    pub fn join_owned_field(&self, field: impl Into<String>) -> Self {
        self.join_segment(FieldPathSegment::Field(Cow::Owned(field.into())))
    }

    pub fn join_index(&self, index: usize) -> Self {
        self.join_segment(FieldPathSegment::Index(index))
    }

    pub fn join_item(&self, id: FormItemId) -> Self {
        self.join_segment(FieldPathSegment::Item(id))
    }

    pub fn join_segment(&self, segment: FieldPathSegment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self { segments }
    }

    pub fn join_path(&self, path: &FieldPath) -> Self {
        let mut segments = self.segments.clone();
        segments.extend(path.segments().iter().cloned());
        Self { segments }
    }

    pub fn parent(&self) -> Option<Self> {
        let mut segments = self.segments.clone();
        segments.pop()?;
        Some(Self { segments })
    }

    pub fn starts_with(&self, parent: &FieldPath) -> bool {
        self.segments.starts_with(parent.segments())
    }

    pub fn strip_prefix(&self, prefix: &FieldPath) -> Option<Self> {
        self.starts_with(prefix).then(|| Self {
            segments: self.segments[prefix.segments().len()..].to_vec(),
        })
    }
}

impl From<&'static str> for FieldPath {
    fn from(value: &'static str) -> Self {
        Self::from_static(value)
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return write!(f, "$");
        }

        for (ix, segment) in self.segments.iter().enumerate() {
            match segment {
                FieldPathSegment::Field(name) => {
                    if ix > 0 {
                        write!(f, ".")?;
                    }
                    write!(f, "{name}")?;
                }
                FieldPathSegment::Index(index) => write!(f, "[{index}]")?,
                FieldPathSegment::Item(id) => write!(f, "[#{}]", id.get())?,
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldPathSegment {
    Field(Cow<'static, str>),
    Index(usize),
    Item(FormItemId),
}

impl FieldPathSegment {
    pub fn field(name: &'static str) -> Self {
        Self::Field(Cow::Borrowed(name))
    }

    pub fn owned_field(name: impl Into<String>) -> Self {
        Self::Field(Cow::Owned(name.into()))
    }
}
