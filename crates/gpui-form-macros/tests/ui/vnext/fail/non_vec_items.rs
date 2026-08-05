#[derive(gpui_form_macros::FormSchema)]
struct NonVecItems {
    #[form(items)]
    rows: String,
}

fn main() {}
