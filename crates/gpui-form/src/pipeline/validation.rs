pub mod adapter;

#[cfg(feature = "garde-adapter")]
pub mod garde;
pub mod report;
pub mod required;

#[cfg(not(feature = "garde-adapter"))]
pub use adapter::GardeAdapter;
pub use adapter::{
    NoValidationContext, NoopValidationAdapter, ValidationAdapter, ValidationContext,
    ValidationContextValue, ValidationScope,
};
#[cfg(feature = "garde-adapter")]
pub use garde::GardeAdapter;
pub use report::{ValidationAdapterReport, ValidationIssue};
pub use required::{RequiredRule, RequiredValue};
