use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(adapter = "garde", messages = FirstProvider, messages = SecondProvider))]
struct Example {
    value: String,
}

fn main() {}
