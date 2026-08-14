struct Option<T, U>(std::marker::PhantomData<(T, U)>);

#[derive(gpui_form_macros::FormSchema)]
struct MalformedOptionalChild {
    #[form(child)]
    value: Option<String, bool>,
}

fn main() {}
