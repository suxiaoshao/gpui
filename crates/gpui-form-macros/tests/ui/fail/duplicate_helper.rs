use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(state = ExampleForm)]
#[form(transform(adapter = "validify"))]
struct Example {
    value: String,
}

fn main() {}
