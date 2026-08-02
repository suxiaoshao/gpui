use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation(adapter = AppValidator, context = "AppContext"))]
struct Example {
    value: String,
}

fn main() {}
