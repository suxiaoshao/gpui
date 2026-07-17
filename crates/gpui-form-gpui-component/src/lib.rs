//! `gpui-component` adapters for `gpui-form`.
//!
//! `gpui-form` owns form state, validation, and submit behavior. This crate
//! owns the optional adapter layer for applications that use `gpui-component`
//! controls.

pub mod binding;
pub mod bool;
pub mod combobox;
pub mod input;
pub mod number;
pub mod select;

pub use binding::{ComponentBindError, bind_bool, bind_input, bind_number, subscribe_form_changes};
pub use bool::{BoolComponentEvent, BoolComponentState};
pub use combobox::{
    ComboboxFieldValue, bind_combobox, focus_combobox, new_combobox_state, set_combobox_items,
};
pub use input::{OptionalTextCodec, TextFieldValue};
pub use number::{NumberCodec, NumberFieldValue, NumberInputKind, NumberInputPolicy, number_input};
pub use select::{SelectFieldValue, bind_select, focus_select, new_select_state, set_select_items};
