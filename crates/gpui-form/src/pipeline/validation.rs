pub mod adapter;

#[cfg(feature = "garde-adapter")]
pub mod garde;
pub mod report;

#[cfg(not(feature = "garde-adapter"))]
pub use adapter::GardeAdapter;
pub use adapter::{NoopValidationAdapter, ValidationAdapter, ValidationContext, ValidationScope};
#[cfg(feature = "garde-adapter")]
pub use garde::GardeAdapter;
pub use report::{ValidationAdapterReport, ValidationIssue};
