use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(adapter = "garde", context = AppContext))]
struct Example {
    value: String,
}

fn main() {}
