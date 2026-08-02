use gpui_form_macros::FormModel;

trait HasItem {
    type Item;
}

#[derive(FormModel)]
struct Example<T: HasItem> {
    #[form(array(id = "id"))]
    items: Vec<<T as HasItem>::Item>,
}

fn main() {}
