use gpui_form::{FieldMerge, FormSchema};

fn merge_value(_: &String, _: &String, _: &String) -> FieldMerge<u64> {
    FieldMerge::merged(42)
}

#[derive(Clone, PartialEq, FormSchema)]
struct Draft {
    #[form(merge = merge_value)]
    value: String,
}

fn main() {}
