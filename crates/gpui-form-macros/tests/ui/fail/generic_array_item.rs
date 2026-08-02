use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example<T> {
    #[form(array(id = "id"))]
    items: Vec<T>,
}

fn main() {}
