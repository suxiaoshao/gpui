use std::borrow::Cow;

use crate::{
    ErrorParams, FieldError, FieldPath, FormError, FormValidationReport, ValidationSeverity,
    ValidationSource, ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationIssue {
    pub path: Option<FieldPath>,
    pub source: ValidationSource,
    pub trigger: ValidationTrigger,
    pub severity: ValidationSeverity,
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

impl ValidationIssue {
    pub fn field(
        path: FieldPath,
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path: Some(path),
            source,
            trigger,
            severity: ValidationSeverity::Error,
            code: code.into(),
            message_key: message_key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn form(
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path: None,
            source,
            trigger,
            severity: ValidationSeverity::Error,
            code: code.into(),
            message_key: message_key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<crate::ErrorParamValue>,
    ) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationAdapterReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationAdapterReport {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(|issue| !issue.severity.is_error())
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }

    pub fn into_form_report(self) -> FormValidationReport {
        let mut report = FormValidationReport::default();

        for issue in self.issues {
            if let Some(path) = issue.path {
                report.push_field_error(FieldError {
                    path,
                    trigger: issue.trigger,
                    severity: issue.severity,
                    source: issue.source,
                    code: issue.code,
                    message_key: issue.message_key,
                    params: issue.params,
                });
            } else {
                report.push_form_error(FormError {
                    path: None,
                    trigger: issue.trigger,
                    severity: issue.severity,
                    source: issue.source,
                    code: issue.code,
                    message_key: issue.message_key,
                    params: issue.params,
                });
            }
        }

        report
    }
}
