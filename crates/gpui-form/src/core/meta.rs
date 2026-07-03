use crate::{SubmitOutcome, SubmitRuntime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMeta {
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_dirty: bool,
    pub is_default_value: bool,
    pub is_validating: bool,
}

impl Default for FieldMeta {
    fn default() -> Self {
        Self {
            is_touched: false,
            is_blurred: false,
            is_dirty: false,
            is_default_value: true,
            is_validating: false,
        }
    }
}

impl FieldMeta {
    pub fn mark_touched(&mut self) {
        self.is_touched = true;
    }

    pub fn mark_blurred(&mut self) {
        self.is_blurred = true;
    }

    pub fn mark_dirty(&mut self, is_default_value: bool) {
        self.is_default_value = is_default_value;
        self.is_dirty = !is_default_value;
    }

    pub fn set_validating(&mut self, validating: bool) {
        self.is_validating = validating;
    }

    pub fn is_pristine(&self) -> bool {
        !self.is_dirty
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormMeta {
    pub is_dirty: bool,
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_validating: bool,
    pub last_submit_outcome: Option<SubmitOutcome>,
    pub submission_attempts: u32,
}

impl FormMeta {
    pub fn aggregate<'a>(fields: impl IntoIterator<Item = &'a FieldMeta>) -> Self {
        let mut meta = Self::default();

        for field in fields {
            meta.is_dirty |= field.is_dirty;
            meta.is_touched |= field.is_touched;
            meta.is_blurred |= field.is_blurred;
            meta.is_validating |= field.is_validating;
        }

        meta
    }

    pub fn with_submit_runtime(mut self, runtime: &SubmitRuntime) -> Self {
        self.last_submit_outcome = runtime.last_outcome();
        self.submission_attempts = runtime.submission_attempts();
        self
    }

    pub fn is_pristine(&self) -> bool {
        !self.is_dirty
    }

    pub fn is_submit_successful(&self) -> bool {
        self.last_submit_outcome == Some(SubmitOutcome::Success)
    }
}
