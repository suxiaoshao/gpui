#[derive(gpui_form_macros::FormSchema)]
struct RemovedArray {
    #[form(array(id = "id"))]
    rows: Vec<String>,
}

fn main() {}
