#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FieldKind {
    #[default]
    Value,
    Input,
    Number,
    Select,
    Combobox,
    Bool,
    Group,
    Array,
    Binding,
}

impl FieldKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "value" => Some(Self::Value),
            "input" | "text" | "textarea" | "secret" | "url" => Some(Self::Input),
            "number" => Some(Self::Number),
            "select" => Some(Self::Select),
            "combobox" => Some(Self::Combobox),
            "checkbox" | "switch" | "bool" => Some(Self::Bool),
            "group" | "nested" => Some(Self::Group),
            "array" | "dynamic_array" => Some(Self::Array),
            _ => None,
        }
    }
}
