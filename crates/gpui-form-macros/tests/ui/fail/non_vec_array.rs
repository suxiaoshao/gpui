use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example {
    #[form(array(id = "id"))]
    items: Item,
}

struct Item;

fn main() {}
