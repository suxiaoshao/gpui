//! Typed, form-owned values, validation, and GPUI control bindings.
//!
//! Unsupported field options fail at macro expansion time instead of being
//! ignored.
//!
//! ```compile_fail
//! #[derive(Clone, Debug, PartialEq, gpui_form::FormModel)]
//! struct TypoFormInput {
//!     #[form(requierd)]
//!     name: String,
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
    pub use crate::{form::FormRuntime, validation::ValidationSnapshot};
    pub use gpui;
}

pub use gpui_form_macros::FormModel;
pub use typed::*;
