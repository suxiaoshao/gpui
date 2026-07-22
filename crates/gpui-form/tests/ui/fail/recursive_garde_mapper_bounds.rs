use gpui_form::typed::{
    FieldPath, FieldPathSegment, FieldSchema, FormModelSchema, FormSchemaPathError,
    StructuralValidate, ValidationIssue, ValidationScope, ValidationTrigger,
};

#[derive(Clone, PartialEq, garde::Validate)]
struct Child {
    #[garde(skip)]
    value: String,
}

impl StructuralValidate for Child {
    fn structural_issues(
        &self,
        _base: &FieldPath,
        _trigger: ValidationTrigger,
        _scope: &ValidationScope,
        _issues: &mut Vec<ValidationIssue>,
    ) {
    }
}

impl FormModelSchema for Child {
    fn schema_at_path(
        &self,
        _segments: &[FieldPathSegment],
    ) -> Result<&'static FieldSchema, FormSchemaPathError> {
        Err(FormSchemaPathError::UnknownField)
    }
}

#[derive(Clone, PartialEq, gpui_form::FormStore, garde::Validate)]
#[form(validation(adapter = "garde"))]
struct Root {
    #[form(group)]
    #[garde(dive)]
    child: Child,
}

fn main() {}
