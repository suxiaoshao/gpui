use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(transform())]
struct Example {
    value: String,
}

fn main() {}
