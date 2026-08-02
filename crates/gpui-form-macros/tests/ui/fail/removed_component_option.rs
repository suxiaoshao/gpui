use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example {
    #[form(component = InputState)]
    value: String,
}

fn main() {}
