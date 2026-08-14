use gpui::{App, Entity};
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
    #[form(child)]
    fallback: Option<Condition>,
}

fn consume_all_path_kinds(form: &Entity<Form<Query>>, cx: &mut App) {
    let roots = Query::ROOT.then(Query::ROOTS).items(form, cx);
    let Some(root) = roots.into_iter().next() else {
        return;
    };

    let kind = root.then(Node::KIND);
    if let Some(condition) = kind.case(Kind::CONDITION).resolve(form, cx).unwrap() {
        let value = condition.then(Condition::VALUE);
        let _: String = value.try_get(form, cx).unwrap();
        let _: bool = value.try_set(form, String::new(), cx).unwrap();
    }

    if let Some(condition) = Query::ROOT
        .then(Query::FALLBACK)
        .some()
        .resolve(form, cx)
        .unwrap()
    {
        let value = condition.then(Condition::VALUE);
        let _: String = value.try_get(form, cx).unwrap();
        let _: bool = value.try_set(form, String::new(), cx).unwrap();
    }
}

fn main() {
    let _ = Form::new(Query {
        roots: Vec::new(),
        fallback: None,
    });
}
