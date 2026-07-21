use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example {
    #[form(validate())]
    value: String,
}

fn main() {}
