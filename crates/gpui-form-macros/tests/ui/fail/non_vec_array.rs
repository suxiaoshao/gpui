use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example {
    #[form(array(id = "id"))]
    items: Item,
}

struct Item;

fn main() {}
