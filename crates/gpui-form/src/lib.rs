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

mod control;
mod field;
mod form;
mod schema;
mod submit;
pub mod typed;
mod validation;

#[doc(hidden)]
pub mod __private {
    pub use crate::form::FormRuntime;
    pub use gpui;
}

pub use gpui_form_macros::FormStore;
pub use typed::*;
