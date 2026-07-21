mod error;
mod parse;
mod policy;

use std::{fmt::Display, ops::Deref, str::FromStr};

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Refineable, RenderOnce, SharedString, StyleRefinement, Styled, Subscription,
    Window,
};
use gpui_component::input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction};
use gpui_component::{Disableable, Sizable, Size};
use gpui_form::typed::{FormControl, FormField, FormStore, ValidationMessage};

use crate::{FormControlError, IntegerInputPolicyError};

pub use error::IntegerInputError;
pub use policy::IntegerInputPolicy;

mod sealed {
    pub trait Sealed {}
}

pub trait IntegerValue: sealed::Sealed + Copy + Eq + Ord + Display + FromStr + 'static {
    const ZERO: Self;
    const ONE: Self;

    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
}

macro_rules! integer_value {
    ($($type:ty),* $(,)?) => {$ (
        impl sealed::Sealed for $type {}
        impl IntegerValue for $type {
            const ZERO: Self = 0;
            const ONE: Self = 1;
            fn checked_add(self, rhs: Self) -> Option<Self> { <$type>::checked_add(self, rhs) }
            fn checked_sub(self, rhs: Self) -> Option<Self> { <$type>::checked_sub(self, rhs) }
        }
    )*};
}

integer_value!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegerInputEvent<N> {
    Change(Result<N, IntegerInputError<N>>),
    Blur,
}

pub struct IntegerInputState<N>
where
    N: IntegerValue,
{
    editor_subscriptions: Vec<Subscription>,
    editor: Entity<InputState>,
    value: N,
    policy: IntegerInputPolicy<N>,
}

impl<N> IntegerInputState<N>
where
    N: IntegerValue,
{
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| InputState::new(window, cx));
        editor.update(cx, |editor, cx| editor.set_step(None, window, cx));

        let input_subscription = cx.subscribe_in(
            &editor,
            window,
            |this, editor, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let result =
                        parse::parse_integer(editor.read(cx).value().as_ref(), this.policy);
                    if let Ok(value) = result {
                        this.value = value;
                    }
                    cx.emit(IntegerInputEvent::Change(result));
                }
                InputEvent::Blur => cx.emit(IntegerInputEvent::Blur),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );

        let step_subscription = cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(action) = event;
                let next = match action {
                    StepAction::Increment => this.value.checked_add(this.policy.step),
                    StepAction::Decrement => this.value.checked_sub(this.policy.step),
                };
                if let Some(next) = next.filter(|value| this.policy.contains(*value)) {
                    let state = cx.entity().downgrade();
                    cx.defer_in(window, move |_, window, cx| {
                        let Some(state) = state.upgrade() else { return };
                        state.update(cx, |state, cx| {
                            state.set_value(next, window, cx);
                            cx.emit(IntegerInputEvent::Change(Ok(next)));
                        });
                    });
                }
            },
        );

        Self {
            editor_subscriptions: vec![input_subscription, step_subscription],
            editor,
            value: N::ZERO,
            policy: IntegerInputPolicy::new(),
        }
    }

    pub fn min(mut self, min: N) -> Self {
        self.policy = self.policy.min(min);
        self
    }
    pub fn max(mut self, max: N) -> Self {
        self.policy = self.policy.max(max);
        self
    }
    pub fn step(mut self, step: N) -> Self {
        self.policy = self.policy.step(step);
        self
    }
    pub fn value(&self) -> N {
        self.value
    }
    pub fn policy(&self) -> IntegerInputPolicy<N> {
        self.policy
    }
    pub fn editor(&self) -> &Entity<InputState> {
        &self.editor
    }

    pub fn set_policy(
        &mut self,
        policy: IntegerInputPolicy<N>,
    ) -> Result<(), IntegerInputPolicyError> {
        policy.validate()?;
        self.policy = policy;
        Ok(())
    }

    pub fn set_value(&mut self, value: N, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value;
        self.editor.update(cx, |editor, cx| {
            editor.set_value(value.to_string(), window, cx)
        });
    }

    pub fn validate_policy(&self) -> Result<(), IntegerInputPolicyError> {
        self.policy.validate().map(|_| ())
    }
}

impl<N> EventEmitter<IntegerInputEvent<N>> for IntegerInputState<N> where N: IntegerValue {}

impl<N> Drop for IntegerInputState<N>
where
    N: IntegerValue,
{
    fn drop(&mut self) {
        self.editor_subscriptions.clear();
    }
}

impl<N> Focusable for IntegerInputState<N>
where
    N: IntegerValue,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

pub struct FormIntegerInput<N>
where
    N: IntegerValue,
{
    subscriptions: Vec<Subscription>,
    input: Entity<IntegerInputState<N>>,
}

impl<N> FormIntegerInput<N>
where
    N: IntegerValue,
{
    pub fn new<Form, Owner, Build>(
        field: FormField<Form, N>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, FormControlError>
    where
        Form: FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<IntegerInputState<N>>) -> IntegerInputState<N>,
    {
        <Self as FormControl<N>>::new(field, build, window, cx)
    }
}

impl<N> Deref for FormIntegerInput<N>
where
    N: IntegerValue,
{
    type Target = Entity<IntegerInputState<N>>;
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<N> Drop for FormIntegerInput<N>
where
    N: IntegerValue,
{
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

impl<N> FormControl<N> for FormIntegerInput<N>
where
    N: IntegerValue,
{
    type State = IntegerInputState<N>;
    type Error = FormControlError;

    fn new<Form, Owner, Build>(
        field: FormField<Form, N>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State,
    {
        let value = field.value(cx)?;
        let attachment = field.attach_control(cx)?;
        let input = cx.new(|cx| build(window, cx));
        input.read(cx).validate_policy()?;
        input.update(cx, |input, cx| input.set_value(value, window, cx));

        let weak_input = input.downgrade();
        let projection = field.clone();
        let projection_attachment = attachment.clone();
        let form_subscription = field.subscribe_in(window, cx, move |_, window, cx| {
            let weak_input = weak_input.clone();
            let projection = projection.clone();
            let attachment = projection_attachment.clone();
            cx.defer_in(window, move |_, window, cx| {
                let Some(input) = weak_input.upgrade() else {
                    return;
                };
                let Ok(value) = projection.value(cx) else {
                    return;
                };
                input.update(cx, |input, cx| input.set_value(value, window, cx));
                attachment.defer_clear_issue(window, cx);
            });
        })?;

        let event_attachment = attachment.clone();
        let event_subscription = cx.subscribe_in(
            &input,
            window,
            move |_, _, event: &IntegerInputEvent<N>, window, cx| match event {
                IntegerInputEvent::Change(Ok(value)) => {
                    event_attachment.defer_clear_issue(window, cx);
                    event_attachment.defer_set_user_value(*value, window, cx);
                }
                IntegerInputEvent::Change(Err(error)) => {
                    let (code, message) = integer_issue(*error);
                    event_attachment.defer_set_issue(code, message, window, cx);
                }
                IntegerInputEvent::Blur => event_attachment.defer_blur(window, cx),
            },
        );

        Ok(Self {
            subscriptions: vec![form_subscription, event_subscription],
            input,
        })
    }
}

fn integer_issue<N: IntegerValue>(
    error: IntegerInputError<N>,
) -> (&'static str, ValidationMessage) {
    match error {
        IntegerInputError::Incomplete => (
            "integer_input_incomplete",
            ValidationMessage::key("gpui-form-error-integer-incomplete"),
        ),
        IntegerInputError::InvalidSyntax => (
            "integer_input_invalid",
            ValidationMessage::key("gpui-form-error-integer-invalid"),
        ),
        IntegerInputError::Overflow => (
            "integer_input_overflow",
            ValidationMessage::key("gpui-form-error-integer-overflow"),
        ),
        IntegerInputError::OutOfRange {
            min: Some(min),
            max: Some(max),
        } => (
            "integer_input_out_of_range",
            ValidationMessage::key("gpui-form-error-integer-range")
                .with_param("min", min.to_string())
                .with_param("max", max.to_string()),
        ),
        IntegerInputError::OutOfRange {
            min: Some(min),
            max: None,
        } => (
            "integer_input_out_of_range",
            ValidationMessage::key("gpui-form-error-integer-min")
                .with_param("min", min.to_string()),
        ),
        IntegerInputError::OutOfRange {
            min: None,
            max: Some(max),
        } => (
            "integer_input_out_of_range",
            ValidationMessage::key("gpui-form-error-integer-max")
                .with_param("max", max.to_string()),
        ),
        IntegerInputError::OutOfRange {
            min: None,
            max: None,
        } => (
            "integer_input_out_of_range",
            ValidationMessage::key("gpui-form-error-integer-range"),
        ),
    }
}

#[derive(IntoElement)]
pub struct IntegerInput<N>
where
    N: IntegerValue,
{
    state: Entity<IntegerInputState<N>>,
    placeholder: SharedString,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    appearance: bool,
    size: Size,
    disabled: bool,
    style: StyleRefinement,
}

impl<N> IntegerInput<N>
where
    N: IntegerValue,
{
    pub fn new(state: &Entity<IntegerInputState<N>>) -> Self {
        Self {
            state: state.clone(),
            placeholder: SharedString::default(),
            prefix: None,
            suffix: None,
            appearance: true,
            size: Size::default(),
            disabled: false,
            style: StyleRefinement::default(),
        }
    }
    pub fn placeholder(mut self, value: impl Into<SharedString>) -> Self {
        self.placeholder = value.into();
        self
    }
    pub fn prefix(mut self, value: impl IntoElement) -> Self {
        self.prefix = Some(value.into_any_element());
        self
    }
    pub fn suffix(mut self, value: impl IntoElement) -> Self {
        self.suffix = Some(value.into_any_element());
        self
    }
    pub fn appearance(mut self, value: bool) -> Self {
        self.appearance = value;
        self
    }
}

impl<N> Focusable for IntegerInput<N>
where
    N: IntegerValue,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}
impl<N> Sizable for IntegerInput<N>
where
    N: IntegerValue,
{
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}
impl<N> Disableable for IntegerInput<N>
where
    N: IntegerValue,
{
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl<N> Styled for IntegerInput<N>
where
    N: IntegerValue,
{
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl<N> RenderOnce for IntegerInput<N>
where
    N: IntegerValue,
{
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let editor = self.state.read(cx).editor().clone();
        let mut input = NumberInput::new(&editor)
            .placeholder(self.placeholder)
            .appearance(self.appearance)
            .with_size(self.size)
            .disabled(self.disabled);
        if let Some(prefix) = self.prefix {
            input = input.prefix(prefix);
        }
        if let Some(suffix) = self.suffix {
            input = input.suffix(suffix);
        }
        input.style().refine(&self.style);
        input
    }
}

#[cfg(test)]
mod tests {
    use super::{IntegerInputError, IntegerInputPolicy, parse::parse_integer};

    #[test]
    fn parses_exact_u64_values_above_two_pow_53() {
        let value = 9_007_199_254_740_993u64;
        assert_eq!(
            parse_integer(&value.to_string(), IntegerInputPolicy::new()),
            Ok(value)
        );
    }

    #[test]
    fn distinguishes_incomplete_overflow_and_range() {
        assert_eq!(
            parse_integer::<i32>("-", IntegerInputPolicy::new()),
            Err(IntegerInputError::Incomplete)
        );
        assert_eq!(
            parse_integer::<u8>("999", IntegerInputPolicy::new()),
            Err(IntegerInputError::Overflow)
        );
        assert!(matches!(
            parse_integer::<u32>("11", IntegerInputPolicy::new().max(10)),
            Err(IntegerInputError::OutOfRange { .. })
        ));
    }
}
