use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(store = ExampleForm, store = OtherForm)]
struct Example {
    value: String,
}

fn main() {}
