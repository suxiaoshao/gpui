#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FieldKind {
    #[default]
    Value,
    Group,
    Array,
}

impl FieldKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "value" => Some(Self::Value),
            "group" | "nested" => Some(Self::Group),
            "array" | "dynamic_array" => Some(Self::Array),
            _ => None,
        }
    }

    pub(crate) fn is_removed_alias(value: &str) -> bool {
        matches!(
            value,
            "input"
                | "text"
                | "textarea"
                | "secret"
                | "url"
                | "number"
                | "select"
                | "combobox"
                | "checkbox"
                | "switch"
                | "bool"
        )
    }
}
