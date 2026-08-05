#[derive(gpui_form_macros::FormSchema)]
struct InvalidTrigger {
    #[form(validate(on_focus))]
    value: String,
}

fn main() {}
