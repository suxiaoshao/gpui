use std::borrow::Cow;

use super::{ValidationIssue, ValidationSource, ValidationTrigger};
use crate::topology::CanonicalAddress;

pub(super) fn replacement_keys(
    issues: &[ValidationIssue],
) -> Vec<(
    CanonicalAddress,
    ValidationSource,
    Cow<'static, str>,
    ValidationTrigger,
)> {
    issues
        .iter()
        .map(|issue| {
            (
                issue.address.clone(),
                issue.source.clone(),
                issue.code.clone(),
                issue.trigger,
            )
        })
        .collect()
}
