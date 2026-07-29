//! A single authoritative, type-safe, in-memory state container for GPUI
//! applications.
//!
//! [`Store<S>`] owns the single source of truth for a state type `S`. It
//! exposes explicit [`read`], [`set`], [`update`], and [`update_if`] for
//! controlled mutation and notification. Derived read-only views are provided
//! by [`select`], [`observe`], [`observe_select`], and
//! [`observe_select_in`].
//!
//! [`Store<S>`]: Store
//! [`read`]: Store::read
//! [`set`]: Store::set
//! [`update`]: Store::update
//! [`update_if`]: Store::update_if
//! [`select`]: Store::select
//! [`observe`]: Store::observe
//! [`observe_select`]: Store::observe_select
//! [`observe_select_in`]: Store::observe_select_in
//!
//! # Example
//!
//! ```no_run
//! use gpui::App;
//! use gpui_store::{Store, StoreChange};
//!
//! struct Counter {
//!     value: u64,
//! }
//!
//! fn update_counter(cx: &mut App) {
//!     let counter = Store::new(cx, Counter { value: 0 });
//!
//!     counter.update(cx, |state| {
//!         state.value += 1;
//!     });
//!
//!     let outcome = counter.update_if(cx, |state| {
//!         if state.value == 1 {
//!             StoreChange::unchanged(state.value)
//!         } else {
//!             state.value = 1;
//!             StoreChange::changed(state.value)
//!         }
//!     });
//!
//!     assert_eq!(outcome.into_result(), 1);
//!     assert_eq!(counter.read(cx, |state| state.value), 1);
//! }
//! ```

mod change;
mod projection;
mod store;

#[cfg(test)]
mod tests;

pub use change::StoreChange;
pub use projection::{Select, StoreSelection};
pub use store::Store;
