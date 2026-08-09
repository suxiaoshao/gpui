#[derive(gpui_form_macros::FormSchema)]
struct InvalidTrigger {
    #[form(validate(on_dynamic))]
    value: String,
}

fn main() {}
