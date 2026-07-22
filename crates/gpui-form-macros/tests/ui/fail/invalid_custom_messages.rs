use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(adapter = AppValidator, messages = AppMessageProvider))]
struct Example {
    value: String,
}

fn main() {}
