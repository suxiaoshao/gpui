use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation(adapter = "garde", messages = FirstProvider, messages = SecondProvider))]
struct Example {
    value: String,
}

fn main() {}
