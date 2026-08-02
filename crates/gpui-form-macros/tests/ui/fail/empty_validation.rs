use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation())]
struct Example {
    value: String,
}

fn main() {}
