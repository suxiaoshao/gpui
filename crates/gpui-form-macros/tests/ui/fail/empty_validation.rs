use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation())]
struct Example {
    value: String,
}

fn main() {}
