use gpui_form_macros::FormModel;

#[derive(FormModel)]
struct Example {
    #[form(array(id = "not-a-field"))]
    items: Vec<Item>,
}

struct Item;

fn main() {}
