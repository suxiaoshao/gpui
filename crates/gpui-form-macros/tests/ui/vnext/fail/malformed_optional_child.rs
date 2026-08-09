#[derive(gpui_form_macros::FormSchema)]
struct MalformedOptionalChild {
    #[form(child)]
    value: Option<String, bool>,
}

fn main() {}
