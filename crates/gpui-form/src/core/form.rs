use gpui::{App, Context, Window};

use crate::{
    AnyFormField, FieldPath, FormMeta, FormValidationReport, SubmitTransform, ValidationAdapter,
    ValidationTrigger,
};

pub trait FormStore: Sized + 'static {
    type Output;

    fn meta(&self) -> &FormMeta;

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>);

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        window: &mut Window,
        cx: &mut App,
    ) -> FormValidationReport;

    fn submit(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Self::Output, FormValidationReport>;

    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App) -> bool;
}

pub trait FormState: 'static {
    type Draft;
    type Output;
    type Validation: ValidationAdapter<Self::Draft>;
    type Transform: SubmitTransform<Self::Draft, Self::Output>;

    fn meta(&self) -> &FormMeta;
    fn draft(&self) -> Self::Draft;
    fn validation(&self) -> &Self::Validation;
    fn transform(&self) -> &Self::Transform;
    fn write_normalized_output(&mut self, output: Self::Output, window: &mut Window, cx: &mut App);
    fn field_paths(&self) -> &[FieldPath];
    fn field(&self, path: &FieldPath) -> Option<&dyn AnyFormField>;
    fn field_mut(&mut self, path: &FieldPath) -> Option<&mut dyn AnyFormField>;

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        window: &mut Window,
        cx: &mut App,
    ) -> FormValidationReport;

    fn submit(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Self::Output, FormValidationReport>;

    fn reset(&mut self, window: &mut Window, cx: &mut App);

    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App) -> bool;
}
