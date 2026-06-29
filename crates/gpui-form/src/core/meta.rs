#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMeta {
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_dirty: bool,
    pub is_pristine: bool,
    pub is_default_value: bool,
    pub is_validating: bool,
    pub is_valid: bool,
}

impl Default for FieldMeta {
    fn default() -> Self {
        Self {
            is_touched: false,
            is_blurred: false,
            is_dirty: false,
            is_pristine: true,
            is_default_value: true,
            is_validating: false,
            is_valid: true,
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
        self.is_pristine = is_default_value;
    }

    pub fn set_validating(&mut self, validating: bool) {
        self.is_validating = validating;
    }

    pub fn set_valid(&mut self, valid: bool) {
        self.is_valid = valid;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormMeta {
    pub is_dirty: bool,
    pub is_pristine: bool,
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_validating: bool,
    pub is_valid: bool,
    pub can_submit: bool,
    pub is_submitting: bool,
    pub is_submitted: bool,
    pub is_submit_successful: bool,
    pub submission_attempts: u32,
}

impl Default for FormMeta {
    fn default() -> Self {
        Self {
            is_dirty: false,
            is_pristine: true,
            is_touched: false,
            is_blurred: false,
            is_validating: false,
            is_valid: true,
            can_submit: true,
            is_submitting: false,
            is_submitted: false,
            is_submit_successful: false,
            submission_attempts: 0,
        }
    }
}

impl FormMeta {
    pub fn aggregate<'a>(fields: impl IntoIterator<Item = &'a FieldMeta>) -> Self {
        let mut meta = Self::default();
        let mut saw_field = false;

        for field in fields {
            saw_field = true;
            meta.is_dirty |= field.is_dirty;
            meta.is_touched |= field.is_touched;
            meta.is_blurred |= field.is_blurred;
            meta.is_validating |= field.is_validating;
            meta.is_valid &= field.is_valid;
        }

        meta.is_pristine = !meta.is_dirty;
        meta.can_submit = saw_field && !meta.is_submitting && !meta.is_validating;
        meta
    }

    pub fn begin_submit(&mut self) {
        self.is_submitting = true;
        self.is_submitted = false;
        self.is_submit_successful = false;
        self.submission_attempts = self.submission_attempts.saturating_add(1);
        self.can_submit = false;
    }

    pub fn finish_submit_success(&mut self) {
        self.is_submitting = false;
        self.is_submitted = true;
        self.is_submit_successful = true;
        self.can_submit = true;
    }

    pub fn finish_submit_failure(&mut self) {
        self.is_submitting = false;
        self.is_submitted = true;
        self.is_submit_successful = false;
        self.is_valid = false;
        self.can_submit = true;
    }
}
