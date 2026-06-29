use std::marker::PhantomData;

use crate::{
    ErrorParamValue, FieldPath, ValidationAdapter, ValidationAdapterReport, ValidationContext,
    ValidationIssue, ValidationScope, ValidationSource, ValidationTrigger,
};

#[derive(Clone, Debug, Default)]
pub struct GardeAdapter<T> {
    _marker: PhantomData<T>,
}

impl<T> GardeAdapter<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> ValidationAdapter<T> for GardeAdapter<T>
where
    T: garde::Validate + 'static,
    T::Context: Default,
{
    fn validate(
        &self,
        draft: &T,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        _context: &ValidationContext,
    ) -> ValidationAdapterReport {
        match draft.validate() {
            Ok(()) => ValidationAdapterReport::default(),
            Err(report) => {
                let mut issues = Vec::new();

                for (path, error) in report.into_inner() {
                    let path_text = path.to_string();
                    let error_message = error.message().to_string();
                    let path = FieldPath::parse_lossy(&path_text);
                    if !scope_includes_path(&scope, &path) {
                        continue;
                    }

                    let issue = if path_text.is_empty() {
                        ValidationIssue::form(
                            trigger,
                            ValidationSource::Garde,
                            "garde",
                            "gpui-form-error-garde",
                        )
                    } else {
                        ValidationIssue::field(
                            path,
                            trigger,
                            ValidationSource::Garde,
                            "garde",
                            "gpui-form-error-garde",
                        )
                    }
                    .with_param("path", path_text)
                    .with_param("message", ErrorParamValue::String(error_message.into()));
                    issues.push(issue);
                }

                ValidationAdapterReport::new(issues)
            }
        }
    }
}

fn scope_includes_path(scope: &ValidationScope, path: &FieldPath) -> bool {
    match scope {
        ValidationScope::Form => true,
        ValidationScope::Field(field_path) => path == field_path,
        ValidationScope::Group(group_path) => path.starts_with(group_path),
        ValidationScope::ArrayItem {
            path: array_path,
            id,
        } => path.starts_with(&array_path.join_item(*id)),
    }
}
