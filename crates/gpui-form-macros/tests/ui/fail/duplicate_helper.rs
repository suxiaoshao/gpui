use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(store = ExampleForm)]
#[form(transform(adapter = "validify"))]
struct Example {
    value: String,
}

fn main() {}
