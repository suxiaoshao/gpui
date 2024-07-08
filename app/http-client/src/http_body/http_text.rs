pub enum TextType {
    Plaintext,
    Json,
    Html,
    Xml,
    Javascript,
    Css,
}

pub struct HttpText {
    text: String,
    text_type: TextType,
}
