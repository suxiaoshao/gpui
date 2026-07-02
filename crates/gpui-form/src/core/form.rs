use gpui::{App, Context, Window};

use crate::{FormMeta, FormValidationReport, SubmitError, SubmitStart, ValidationTrigger};

pub trait FormStore: Sized + 'static {
    type Output;

    fn meta(&self) -> &FormMeta;

    fn is_submitting(&self) -> bool;

    fn is_submitted(&self) -> bool {
        self.meta().submission_attempts > 0 && !self.is_submitting()
    }

    fn can_attempt_submit(&self) -> bool {
        !self.is_submitting() && !self.meta().is_validating
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>);

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        window: &mut Window,
        cx: &mut App,
    ) -> FormValidationReport;

    fn prepare_submit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Self::Output, FormValidationReport>;

    fn submit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Self::Output, FormValidationReport> {
        self.prepare_submit(window, cx)
    }

    fn submit_sync<H, Success, Error>(
        &mut self,
        handler: H,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Success, SubmitError<Error>>
    where
        H: FnOnce(Self::Output, &mut Window, &mut App) -> Result<Success, Error>;

    fn submit_async<H, Success, TaskError, StartError>(
        &mut self,
        handler: H,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<SubmitStart, SubmitError<StartError>>
    where
        Success: 'static,
        TaskError: 'static,
        H: FnOnce(
            Self::Output,
            &mut Window,
            &mut App,
        ) -> Result<gpui::Task<Result<Success, TaskError>>, StartError>;

    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App) -> bool;
}
