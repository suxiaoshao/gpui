#[derive(Clone, PartialEq)]
struct Row {
    row_id: u64,
}

impl gpui_form::typed::GardePathMapper for Row {
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
    #[form(array(id = "row_id"))]
    rows: Vec<Row>,
}

fn main() {}
