use crate::{FieldMeta, FormMeta};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValidationTrigger {
    Mount,
    Change,
    Blur,
    Submit,
    Dynamic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldChangeCause {
    UserInput,
    Blur,
    Reset,
    NormalizeOnSubmit,
    External,
}

impl FieldChangeCause {
    pub fn marks_dirty(self) -> bool {
        matches!(self, Self::UserInput | Self::Blur | Self::External)
    }

    pub fn triggers_change_validation(self) -> bool {
        matches!(self, Self::UserInput | Self::External)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ErrorVisibility {
    Always,
    AfterTouched,
    AfterBlurred,
    AfterSubmit,
    #[default]
    AfterInteractionOrSubmit,
    Never,
}

impl ErrorVisibility {
    pub fn is_visible(self, field_meta: &FieldMeta, form_meta: &FormMeta) -> bool {
        match self {
            Self::Always => true,
            Self::AfterTouched => field_meta.is_touched,
            Self::AfterBlurred => field_meta.is_blurred,
            Self::AfterSubmit => form_meta.submission_attempts > 0,
            Self::AfterInteractionOrSubmit => {
                field_meta.is_touched || field_meta.is_blurred || form_meta.submission_attempts > 0
            }
            Self::Never => false,
        }
    }
}
