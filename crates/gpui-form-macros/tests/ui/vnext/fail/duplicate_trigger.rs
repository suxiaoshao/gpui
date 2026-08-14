#[derive(gpui_form_macros::FormSchema)]
struct DuplicateTrigger {
    #[form(validate(on_external, on_external))]
    value: String,
}

fn main() {}
