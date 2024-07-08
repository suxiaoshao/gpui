pub use http_text::HttpText;
pub use x_form::XForm;

mod form_data;
mod http_text;
mod x_form;

pub enum BodyType {
    None,
    Text,
    XForm,
    FormData,
}

pub struct HttpBodyForm {
    body_type: BodyType,
    text: HttpText,
    x_form: Vec<XForm>,
}

pub struct HttpBodyView {}
