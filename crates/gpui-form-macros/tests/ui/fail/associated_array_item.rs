use gpui_form_macros::FormStore;

trait HasItem {
    type Item;
}

#[derive(FormStore)]
struct Example<T: HasItem> {
    #[form(array(id = "id"))]
    items: Vec<T::Item>,
}

fn main() {}
