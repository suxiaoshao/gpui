#[derive(Clone, PartialEq)]
struct Child {
    value: String,
}

impl gpui_form::typed::GardePathMapper for Child {
    fn map_garde_path(
        &self,
        path: &str,
    ) -> Result<gpui_form::typed::FieldPath, gpui_form::typed::GardePathError> {
        Err(gpui_form::typed::GardePathError::UnknownField {
            path: path.to_owned(),
        })
    }
}

#[derive(Clone, PartialEq, gpui_form::FormStore)]
struct Root {
    #[form(group)]
    child: Child,
}

fn main() {}
