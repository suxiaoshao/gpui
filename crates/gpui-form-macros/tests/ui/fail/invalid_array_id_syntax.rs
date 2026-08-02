use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example {
    #[form(array(id = item_id))]
    items: Vec<Item>,
}

struct Item;

fn main() {}
