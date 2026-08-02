use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation(adapter = "garde", context = AppContext))]
struct Example {
    value: String,
}

fn main() {}
