use gpui_form_macros::FormModel;

trait HasItem {
    type Item;
}

#[derive(FormModel)]
struct Example<T: HasItem> {
    #[form(array(id = "id"))]
    items: Vec<T::Item>,
}

fn main() {}
