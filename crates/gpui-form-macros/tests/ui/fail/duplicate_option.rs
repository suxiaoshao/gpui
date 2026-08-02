use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(state = ExampleForm, state = OtherForm)]
struct Example {
    value: String,
}

fn main() {}
