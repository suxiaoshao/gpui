use gpui_form_macros::FormStore;

trait HasItem {
    type Item;
}

#[derive(FormStore)]
struct Example<T: HasItem> {
    #[form(array(id = "id"))]
    items: Vec<<T as HasItem>::Item>,
}

fn main() {}
