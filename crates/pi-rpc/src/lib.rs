#![doc = include_str!("../README.md")]
mod client;
mod error;
mod jsonl;
pub mod probe;
pub mod protocol;
pub use client::{Client, CloseReport, ConnectionState, EventStream, LaunchOptions, Limits};
pub use error::Error;
