use gpui::App;
use gpui_form::{ControlWriter, EditorStatus, FormSchema, LifecycleError, LifecycleReport};

type Discard = dyn Fn(&mut App) -> Result<LifecycleReport, LifecycleError>;
type Query = dyn Fn(&App) -> Result<EditorStatus, LifecycleError>;

/// Erased weak source capability; adapters never need to expose generated editor identities.
pub(crate) struct EditorAccess {
    discard: Box<Discard>,
    query: Box<Query>,
}

impl EditorAccess {
    pub(crate) fn new<M: FormSchema, T: Clone + PartialEq + 'static>(
        writer: &ControlWriter<M, T>,
    ) -> Self {
        let discard = writer.clone();
        let query = writer.clone();
        Self {
            discard: Box::new(move |cx| discard.discard_draft(cx)),
            query: Box::new(move |cx| query.editor_status(cx)),
        }
    }
    pub(crate) fn discard_draft(&self, cx: &mut App) -> Result<LifecycleReport, LifecycleError> {
        (self.discard)(cx)
    }
    pub(crate) fn editor_status(&self, cx: &App) -> Result<EditorStatus, LifecycleError> {
        (self.query)(cx)
    }
}
