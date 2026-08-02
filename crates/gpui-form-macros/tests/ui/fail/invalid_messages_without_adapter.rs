use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation(messages = AppMessageProvider))]
struct Example {
    value: String,
}

fn main() {}
