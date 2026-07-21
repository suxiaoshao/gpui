use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example {
    #[form(component = InputState)]
    value: String,
}

fn main() {}
