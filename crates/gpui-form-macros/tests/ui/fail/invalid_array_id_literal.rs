use gpui_form_macros::FormStore;

#[derive(FormStore)]
struct Example {
    #[form(array(id = "not-a-field"))]
    items: Vec<Item>,
}

struct Item;

fn main() {}
