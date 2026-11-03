#[derive(gpui_form_macros::FormSchema)]
struct DuplicateMerge {
    #[form(merge = merge_value)]
    #[form(merge = merge_value)]
    value: String,
}

fn main() {}
