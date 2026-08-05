use gpui_form::{Form, FormSchema};

#[derive(Clone, PartialEq, FormSchema)]
struct Condition {
    value: String,
}

#[derive(Clone, PartialEq, FormSchema)]
struct Group {
    #[form(items)]
    children: Vec<Node>,
}

#[derive(Clone, PartialEq, FormSchema)]
enum Kind {
    Group(Group),
    Condition(Condition),
    Empty,
}

#[derive(Clone, PartialEq, FormSchema)]
struct Node {
    #[form(child)]
    kind: Kind,
}

#[derive(Clone, PartialEq, FormSchema)]
struct Query {
    #[form(items)]
    roots: Vec<Node>,
}

fn main() {
    let _ = Form::try_new(Query { roots: Vec::new() }).unwrap();
    let _value = Query::ROOT
        .then(Query::ROOTS);
}
