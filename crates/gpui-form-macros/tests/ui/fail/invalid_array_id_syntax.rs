use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example {
    #[form(array(id = item_id))]
    items: Vec<Item>,
}

struct Item;

fn main() {}
