//! `gpui-component` bindings for `gpui-form`.
//!
//! `gpui-form` owns form state, validation, and submit behavior. This crate
//! owns the optional adapter layer for applications that use `gpui-component`
//! controls.

pub mod bool;
pub mod combobox;
pub mod input;
pub mod number;
pub mod select;

pub use bool::{BoolBinding, BoolComponentState};
pub use combobox::{ComboboxBinding, ComboboxFieldValue};
pub use input::{TextFieldValue, TextInputBinding};
pub use number::{NumberFieldValue, NumberInputBinding, number_input};
pub use select::{SelectBinding, SelectFieldValue};
