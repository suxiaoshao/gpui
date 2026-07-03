use gpui::{App, Global, SharedString};

use crate::{ErrorParams, FieldError, ValidationSeverity};

#[derive(Clone, Copy)]
pub struct FormTextResolver {
    resolve: fn(&str, &App) -> SharedString,
}

impl FormTextResolver {
    pub fn new(resolve: fn(&str, &App) -> SharedString) -> Self {
        Self { resolve }
    }

    pub fn resolve(&self, key: &str, cx: &App) -> SharedString {
        (self.resolve)(key, cx)
    }
}

impl Global for FormTextResolver {}

pub fn resolve_form_text(key: &'static str, cx: &App) -> SharedString {
    if cx.has_global::<FormTextResolver>() {
        let resolve = cx.global::<FormTextResolver>().resolve;
        resolve(key, cx)
    } else {
        key.into()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormText {
    pub key: &'static str,
    pub params: ErrorParams,
}

impl FormText {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            params: ErrorParams::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldText {
    pub label: Option<FormText>,
    pub description: Option<FormText>,
    pub help: Option<FormText>,
    pub placeholder: Option<FormText>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormIconKind {
    Error,
    Warning,
    Success,
    Info,
    Loading,
    Required,
    AddItem,
    RemoveItem,
    ReorderItem,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldErrorViewState {
    pub severity: ValidationSeverity,
    pub icon: FormIconKind,
    pub text: FormText,
}

impl FieldErrorViewState {
    pub fn from_error(error: &FieldError) -> Self {
        let icon = match error.severity {
            ValidationSeverity::Error => FormIconKind::Error,
            ValidationSeverity::Warning => FormIconKind::Warning,
            ValidationSeverity::Info => FormIconKind::Info,
        };

        Self {
            severity: error.severity,
            icon,
            text: FormText {
                key: match error.message_key.as_ref() {
                    "gpui-form-error-garde" => "gpui-form-error-garde",
                    "gpui-form-error-internal" => "gpui-form-error-internal",
                    _ => "gpui-form-error",
                },
                params: error.params.clone(),
            },
        }
    }
}
