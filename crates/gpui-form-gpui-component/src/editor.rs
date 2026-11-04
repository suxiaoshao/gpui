use gpui::App;
use gpui_form::{
    ControlBinding, ControlWriter, EditorStatus, FormSchema, LifecycleError, LifecycleReport,
};

trait EditorSource {
    fn discard_draft(&self, cx: &mut App) -> Result<LifecycleReport, LifecycleError>;
    fn editor_status(&self, cx: &App) -> Result<EditorStatus, LifecycleError>;
}

struct BoundEditor<M: FormSchema, T: 'static> {
    _binding: ControlBinding,
    writer: ControlWriter<M, T>,
}

impl<M: FormSchema, T: Clone + PartialEq + 'static> EditorSource for BoundEditor<M, T> {
    fn discard_draft(&self, cx: &mut App) -> Result<LifecycleReport, LifecycleError> {
        self.writer.discard_draft(cx)
    }

    fn editor_status(&self, cx: &App) -> Result<EditorStatus, LifecycleError> {
        self.writer.editor_status(cx)
    }
}

/// Keep the binding and one weak writer together behind a compact erased handle.
pub(crate) struct EditorAccess(Box<dyn EditorSource>);

impl EditorAccess {
    pub(crate) fn new<M: FormSchema, T: Clone + PartialEq + 'static>(
        binding: ControlBinding,
        writer: &ControlWriter<M, T>,
    ) -> Self {
        Self(Box::new(BoundEditor {
            _binding: binding,
            writer: writer.clone(),
        }))
    }
    pub(crate) fn discard_draft(&self, cx: &mut App) -> Result<LifecycleReport, LifecycleError> {
        self.0.discard_draft(cx)
    }
    pub(crate) fn editor_status(&self, cx: &App) -> Result<EditorStatus, LifecycleError> {
        self.0.editor_status(cx)
    }
}
