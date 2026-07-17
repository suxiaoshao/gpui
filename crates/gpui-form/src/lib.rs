//! Shared form state, validation, and submit transform primitives for GPUI
//! applications.
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
//! Component state and subscriptions are owned by the caller and connected via
//! adapter crates.
//!
//! ```compile_fail
//! #[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
//! struct OldCustomStateFormInput {
//!     #[form(binding = "CustomBinding")]
//!     secret: String,
//! }
//! ```

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

pub use core::array::{
    ArrayIndexError, FieldArrayItem, FieldArrayStore, FormItemId, FormItemIdGenerator, FormRowValue,
};
pub use core::codec::{DraftFieldStore, DraftUpdate, FieldCodec, FieldCodecError, IdentityCodec};
pub use core::error::{
    ErrorParamValue, ErrorParams, FieldError, FieldValidationReport, FormError,
    FormValidationReport, ValidationSeverity, ValidationSource,
};
pub use core::events::{FieldDraftEvent, FormDraftEvent, FormStoreEvent};
pub use core::field::{FieldCore, FormField, ValidationTriggers, ValueFieldStore};
pub use core::field_handle::{FormFieldHandle, FormFieldHandleError};
pub use core::form::FormStore;
pub use core::group::FieldGroupStore;
pub use core::meta::{FieldMeta, FormMeta};
pub use core::options::{OptionMismatch, OptionsSnapshot};
pub use core::path::{FieldPath, FieldPathSegment};
pub use core::submit::{SubmitError, SubmitOutcome, SubmitRuntime};
pub use core::subscriptions::SubscriptionSet;
pub use core::trigger::{ErrorVisibility, FieldChangeCause, ValidationTrigger};
pub use gpui_form_macros::FormStore;
pub use pipeline::transform::{
    IdentityTransform, SubmitTransform, TransformContext, TransformReport, ValidifyTransform,
};
pub use pipeline::validation::{
    GardeAdapter, NoValidationContext, NoopValidationAdapter, RequiredRule, RequiredValue,
    ValidationAdapter, ValidationAdapterReport, ValidationContext, ValidationContextValue,
    ValidationIssue, ValidationScope,
};
pub use view::render::{
    FieldErrorViewState, FieldText, FormIconKind, FormText, FormTextResolver, resolve_form_text,
};
pub use view::state::FieldViewState;
