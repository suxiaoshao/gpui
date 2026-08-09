use gpui::{App, Entity};
use gpui_form::{Form, FormSchema};

#[derive(Clone, PartialEq, FormSchema)]
struct Draft {
    name: String,
}

fn wrong_value_type(form: &Entity<Form<Draft>>, cx: &mut App) {
    Draft::NAME.set(form, 42_u64, cx);
}

fn main() {}
