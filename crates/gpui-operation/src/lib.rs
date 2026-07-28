//! Caller-controlled, fallible operation lifecycles via message-driven
//! transitions.
//!
//! The crate provides two families of type-safe state machines:
//!
//! - [`refresh`] — repeatable fetches and retries with no explicit repair step.
//! - [`repair`] — fetches that require a caller-selected repair after a
//!   problem.
//!
//! Each family provides a complete runtime `Operation` enum for long-term
//! storage, plus concrete named states. Both layers receive owned messages
//! through [`Transition<Message>`].
//! Synchronous work can settle directly from `Idle` or `Ready` with [`Settle`];
//! [`Complete`] is reserved for an active task.
//!
//! # Quick start
//!
//! ```rust
//! use gpui_operation::{
//!     Complete, Load, Transition,
//!     refresh::{Operation, Phase},
//! };
//!
//! // A running task is just an owned opaque handle the caller constructs.
//! struct MyTask;
//!
//! let mut operation =
//!     Operation::<i32, std::io::Error, MyTask>::new();
//! operation.transition(Load(MyTask));
//! assert_eq!(operation.phase(), Phase::Loading);
//! operation.transition(Complete(Ok(42)));
//! assert_eq!(operation.data(), Some(&42));
//! ```
//!
//! The caller owns Task construction, task runtime, completion routing, and
//! notification. The runtime enum owns state matching and payload movement.

#![forbid(unsafe_code)]

mod message;
mod transition;

pub use message::{Cancel, Complete, Load, Refresh, Repair, Retry, Settle};
pub use transition::Transition;

pub mod refresh;
pub mod repair;
