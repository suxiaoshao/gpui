use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(adapter = AppValidator, context = "AppContext"))]
struct Example {
    value: String,
}

fn main() {}
