use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(messages = AppMessageProvider))]
struct Example {
    value: String,
}

fn main() {}
