#[derive(gpui_form_macros::FormSchema)]
struct ConflictingFieldKind {
    #[form(child, items)]
    values: Vec<String>,
}

fn main() {}
