//! Typed editing sessions for GPUI applications.

mod change;
mod control;
mod error;
mod form;
#[cfg(feature = "garde-adapter")]
mod garde;
mod path;
mod schema;
mod submit;
mod topology;
pub mod typed;
mod validation;

#[doc(hidden)]
pub mod __private {
    pub use crate::schema::SchemaVisitor;
    pub use gpui;
}

pub use change::{ChangeTarget, ModelChange, ModelChangeKind, PathImpact};
pub use control::{ControlBinding, ControlProjection, ControlWriter};
pub use error::{MutationError, PrepareError, ResolveError, TopologyError};
pub use form::{AsyncValidationIssue, Form, FormEvent, FormRevision};
#[cfg(feature = "garde-adapter")]
pub use garde::{
    DefaultGardeMessageProvider, GardeMessageProvider, GardeRule, GardeValidator, garde_error,
};
pub use gpui_form_macros::FormSchema;
pub use path::{
    CaseResolver, DynamicItemsPath, DynamicPath, IntoItemsPath, IntoTotalPath, ItemPath,
    OptionalResolver, PathEdge, Position, TotalItemsPath, TotalPath, ValidationCaseResolver,
    ValidationDynamicItemsPath, ValidationDynamicPath, ValidationItemPath,
    ValidationOptionalResolver, ValidationPathEdge,
};
pub use schema::{
    CaseDef, ChildDef, FieldDef, FieldSchema, FormSchema, ItemsDef, RequiredValue, RootDef,
    ValidationTriggers,
};
pub use submit::{FormVersion, Prepared};
pub use topology::PathKey;
pub use validation::{
    ErrorParamValue, ErrorParams, ValidationIssue, ValidationMessage, ValidationPath,
    ValidationReport, ValidationRequest, ValidationSink, ValidationSource, ValidationTrigger,
    Validator,
};

pub(crate) use path::PathCore;
pub(crate) use topology::ItemToken;
