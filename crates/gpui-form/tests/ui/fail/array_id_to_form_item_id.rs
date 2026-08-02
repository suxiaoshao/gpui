#[derive(Clone, PartialEq, gpui_form::FormModel)]
struct Row {
    row_id: String,
}

#[derive(Clone, PartialEq, gpui_form::FormModel)]
struct Root {
    #[form(array(id = "row_id"))]
    rows: Vec<Row>,
}

fn main() {}
