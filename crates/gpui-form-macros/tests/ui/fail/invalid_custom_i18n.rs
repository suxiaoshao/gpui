use gpui_form_macros::FormModel;

#[derive(FormModel)]
#[form(validation(adapter = AppValidator, i18n = AppI18nProvider))]
struct Example {
    value: String,
}

fn main() {}
