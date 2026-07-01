//! Shared form state, component binding, validation, and submit transform
//! primitives for GPUI applications.
//!
//! Unsupported field options fail at macro expansion time instead of being
//! ignored.
//!
//! ```compile_fail
//! #[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
//! struct TypoFormInput {
//!     #[form(requierd)]
//!     name: String,
//! }
//! ```
//!
//! The removed app-specific `state = "..."` attribute is also rejected; use
//! `binding = "TypeName"` instead.
//!
//! ```compile_fail
//! #[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
//! struct OldCustomStateFormInput {
//!     #[form(state = "CustomState")]
//!     secret: String,
//! }
//! ```

pub mod component;
pub mod core;
pub mod macro_support;
pub mod pipeline;
pub mod view;

#[doc(hidden)]
pub mod __private {
    pub use gpui;
}

#[cfg(test)]
mod test_support;

pub use component::binding::{ComponentStateOptions, FormComponentBinding, FormComponentEvent};
pub use component::fields::{
    BoolBinding, BoolComponentState, BoolFieldStore, ComboboxBinding, ComboboxFieldStore,
    ComboboxFieldValue, ComponentFieldStore, NumberFieldStore, NumberFieldValue,
    NumberInputBinding, NumberInputSync, SelectBinding, SelectFieldStore, SelectFieldValue,
    TextFieldStore, TextFieldValue, TextInputBinding,
};
pub use core::array::{
    ArrayIndexError, FieldArrayItem, FieldArrayStore, FormItemId, FormItemIdGenerator, FormRowValue,
};
pub use core::error::{
    ErrorParamValue, ErrorParams, FieldError, FieldValidationReport, FormError,
    FormValidationReport, ValidationSeverity, ValidationSource,
};
pub use core::field::{
    AnyFormField, FieldCore, FormField, NoComponentState, ValidationTriggers, ValueFieldStore,
};
pub use core::form::{FormState, FormStore};
pub use core::group::{FieldGroupStore, FormFragment};
pub use core::meta::{FieldMeta, FormMeta, SubmitOutcome};
pub use core::options::{OptionMismatch, OptionsSnapshot};
pub use core::path::{FieldPath, FieldPathSegment};
pub use core::subscriptions::SubscriptionSet;
pub use core::trigger::{ErrorVisibility, FieldChangeCause, ValidationTrigger};
pub use gpui_form_macros::FormStore;
pub use pipeline::transform::{
    IdentityTransform, SubmitTransform, TransformContext, TransformReport, ValidifyTransform,
};
pub use pipeline::validation::{
    GardeAdapter, NoopValidationAdapter, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationScope,
};
pub use view::render::{
    FieldErrorViewState, FieldText, FormIconKind, FormText, FormTextResolver, resolve_form_text,
};
