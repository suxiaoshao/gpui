use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example {
    #[form(validate())]
    value: String,
}

fn main() {}
