use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example<T> {
    #[form(array(id = "id"))]
    items: Vec<T>,
}

fn main() {}
