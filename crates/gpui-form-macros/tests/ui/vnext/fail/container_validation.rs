#[derive(gpui_form_macros::FormSchema)]
struct ContainerValidation {
    #[form(child, required)]
    value: String,
}

fn main() {}
