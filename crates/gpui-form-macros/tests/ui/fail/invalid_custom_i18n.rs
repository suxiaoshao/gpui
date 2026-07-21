use gpui_form_macros::FormStore;

#[derive(FormStore)]
#[form(validation(adapter = AppValidator, i18n = AppI18nProvider))]
struct Example {
    value: String,
}

fn main() {}
