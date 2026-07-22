use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(transform())]
struct Example {
    value: String,
}

fn main() {}
