use std::{borrow::Cow, fmt, future::Future, marker::PhantomData, sync::Arc};

use gpui::{App, Context, Entity, Subscription, Window};
use gpui_operation::Transition as _;

use crate::{
    control::ControlBinding,
    form::transition::{
        CancelAsyncValidation, CommitFieldValue, CompleteAsyncValidation,
        NextAsyncValidationAttempt, ReplaceSynchronousValidation, StartAsyncValidation,
    },
    form::{FormEvent, FormState, apply_form_effect, apply_validation_effect},
    schema::{FieldSchema, array::FormItemId, path::FieldPath},
    validation::report::{ValidationIssue, ValidationSource},
    validation::trigger::ValidationTrigger,
    validation::{AsyncValidationIssue, ValidationScope},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldAccessError {
    ValueUnavailable,
    MissingItem(FormItemId),
    DuplicateItem(FormItemId),
}

impl fmt::Display for FieldAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueUnavailable => f.write_str("the field value is unavailable"),
            Self::MissingItem(id) => write!(f, "form item #{id} is missing"),
            Self::DuplicateItem(id) => write!(f, "form item #{id} is duplicated"),
        }
    }
}

impl std::error::Error for FieldAccessError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldMutationError {
    Access(FieldAccessError),
    ItemIdentityChanged,
}

impl fmt::Display for FieldMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => error.fmt(f),
            Self::ItemIdentityChanged => {
                f.write_str("an identified array item cannot change its stable id")
            }
        }
    }
}

impl std::error::Error for FieldMutationError {}

impl From<FieldAccessError> for FieldMutationError {
    fn from(error: FieldAccessError) -> Self {
        Self::Access(error)
    }
}

#[derive(Clone)]
enum DescriptorPath {
    Static(&'static str),
    Located(FieldPath),
}

impl DescriptorPath {
    fn to_path(&self) -> FieldPath {
        match self {
            Self::Static(name) => FieldPath::field(name),
            Self::Located(path) => path.clone(),
        }
    }
}

type DynamicRead<Form, T> = dyn Fn(&<Form as FormState>::Model) -> Result<T, FieldAccessError>;
type DynamicWrite<Form, T> =
    dyn Fn(&mut <Form as FormState>::Model, T) -> Result<(), FieldMutationError>;

enum ReadLens<Form, T>
where
    Form: FormState,
{
    Static(fn(&Form::Model) -> &T),
    Dynamic(Arc<DynamicRead<Form, T>>),
}

impl<Form, T> Clone for ReadLens<Form, T>
where
    Form: FormState,
{
    fn clone(&self) -> Self {
        match self {
            Self::Static(read) => Self::Static(*read),
            Self::Dynamic(read) => Self::Dynamic(read.clone()),
        }
    }
}

enum WriteLens<Form, T>
where
    Form: FormState,
{
    Static(fn(&mut Form::Model, T)),
    Dynamic(Arc<DynamicWrite<Form, T>>),
}

impl<Form, T> Clone for WriteLens<Form, T>
where
    Form: FormState,
{
    fn clone(&self) -> Self {
        match self {
            Self::Static(write) => Self::Static(*write),
            Self::Dynamic(write) => Self::Dynamic(write.clone()),
        }
    }
}

pub struct FormField<Form, T>
where
    Form: FormState,
{
    event_path: DescriptorPath,
    validation_path: DescriptorPath,
    schema: FieldSchema,
    read: ReadLens<Form, T>,
    write: WriteLens<Form, T>,
    item_id_at: Option<fn(&T, usize) -> Option<FormItemId>>,
    marker: PhantomData<fn(T) -> T>,
}

impl<Form, T> Clone for FormField<Form, T>
where
    Form: FormState,
{
    fn clone(&self) -> Self {
        Self {
            event_path: self.event_path.clone(),
            validation_path: self.validation_path.clone(),
            schema: self.schema,
            read: self.read.clone(),
            write: self.write.clone(),
            item_id_at: self.item_id_at,
            marker: PhantomData,
        }
    }
}

pub struct PartialFormField<Form, T>
where
    Form: FormState,
{
    event_path: FieldPath,
    validation_path: FieldPath,
    read: Arc<DynamicRead<Form, T>>,
    write: Arc<DynamicWrite<Form, T>>,
    marker: PhantomData<fn(T) -> T>,
}

impl<Form, T> Clone for PartialFormField<Form, T>
where
    Form: FormState,
{
    fn clone(&self) -> Self {
        Self {
            event_path: self.event_path.clone(),
            validation_path: self.validation_path.clone(),
            read: self.read.clone(),
            write: self.write.clone(),
            marker: PhantomData,
        }
    }
}

impl<Form, T> FormField<Form, T>
where
    Form: FormState,
    T: Clone + PartialEq + 'static,
{
    #[doc(hidden)]
    pub const fn __new(
        name: &'static str,
        schema: FieldSchema,
        read: fn(&Form::Model) -> &T,
        write: fn(&mut Form::Model, T),
        item_id_at: Option<fn(&T, usize) -> Option<FormItemId>>,
    ) -> Self {
        Self {
            event_path: DescriptorPath::Static(name),
            validation_path: DescriptorPath::Static(name),
            schema,
            read: ReadLens::Static(read),
            write: WriteLens::Static(write),
            item_id_at,
            marker: PhantomData,
        }
    }

    fn from_dynamic(
        event_path: FieldPath,
        validation_path: FieldPath,
        schema: FieldSchema,
        read: impl Fn(&Form::Model) -> Result<T, FieldAccessError> + 'static,
        write: impl Fn(&mut Form::Model, T) -> Result<(), FieldMutationError> + 'static,
    ) -> Self {
        Self {
            event_path: DescriptorPath::Located(event_path),
            validation_path: DescriptorPath::Located(validation_path),
            schema,
            read: ReadLens::Dynamic(Arc::new(read)),
            write: WriteLens::Dynamic(Arc::new(write)),
            item_id_at: None,
            marker: PhantomData,
        }
    }

    fn read_model(&self, model: &Form::Model) -> Result<T, FieldAccessError> {
        match &self.read {
            ReadLens::Static(read) => Ok(read(model).clone()),
            ReadLens::Dynamic(read) => read(model),
        }
    }

    fn write_model(&self, model: &mut Form::Model, value: T) -> Result<(), FieldMutationError> {
        match &self.write {
            WriteLens::Static(write) => {
                write(model, value);
                Ok(())
            }
            WriteLens::Dynamic(write) => write(model, value),
        }
    }

    fn as_partial(&self) -> PartialFormField<Form, T> {
        let read_field = self.clone();
        let write_field = self.clone();
        PartialFormField::new_dynamic(
            self.path(),
            self.validation_path(),
            move |model| read_field.read_model(model),
            move |model, value| write_field.write_model(model, value),
        )
    }

    pub fn path(&self) -> FieldPath {
        self.event_path.to_path()
    }

    pub fn schema(&self) -> FieldSchema {
        self.schema
    }

    fn validation_path(&self) -> FieldPath {
        self.validation_path.to_path()
    }

    pub fn value(&self, form: &Entity<Form>, cx: &App) -> T {
        self.read_model(form.read(cx).value())
            .expect("total form descriptor must always resolve")
    }

    pub fn set(&self, form: &Entity<Form>, value: T, cx: &mut App) {
        let field = self.clone();
        form.update(cx, move |form, form_cx| {
            let mut candidate = form.value().clone();
            field
                .write_model(&mut candidate, value)
                .expect("total form descriptor must always resolve");
            commit_candidate(
                form,
                candidate,
                &field.path(),
                &field.validation_path(),
                form_cx,
            );
        });
    }

    pub fn validate(&self, form: &Entity<Form>, trigger: ValidationTrigger, cx: &mut App) {
        let validation_path = self.validation_path();
        form.update(cx, move |form, form_cx| {
            let snapshot = form.value().clone();
            let validation = form.__validation_snapshot(
                &snapshot,
                trigger,
                ValidationScope::Field(validation_path),
                form_cx,
            );
            let effect = form
                .__runtime_mut()
                .validation_mut()
                .transition(ReplaceSynchronousValidation(validation));
            apply_validation_effect(effect, form_cx);
        });
    }

    pub fn errors(&self, form: &Entity<Form>, cx: &App) -> Vec<ValidationIssue> {
        errors_for_paths(form.read(cx), &self.path(), &self.validation_path())
    }

    pub fn is_validating(&self, form: &Entity<Form>, cx: &App) -> bool {
        form.read(cx).is_validating_at(&self.path())
    }

    pub fn bind_control(&self, form: &Entity<Form>, cx: &mut App) -> ControlBinding<Form, T> {
        let _ = self.value(form, cx);
        ControlBinding::new(form, self.as_partial())
    }

    pub fn subscribe_in<Owner>(
        &self,
        form: &Entity<Form>,
        window: &Window,
        cx: &mut Context<Owner>,
        listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
    {
        subscribe_to_descriptor(self.path(), form, window, cx, listener)
    }

    pub fn start_async_validation<F, Fut>(
        &self,
        form: &Entity<Form>,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    ) where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        let value = self.value(form, cx);
        start_async_validation(
            form,
            value,
            self.path(),
            source.into(),
            trigger,
            validate,
            cx,
        );
    }

    pub fn cancel_async_validation(&self, form: &Entity<Form>, source: &str, cx: &mut App) {
        cancel_async_validation(form, self.path(), source, cx);
    }

    pub fn project_value<U>(
        self,
        name: &'static str,
        read: impl Fn(&T) -> Option<U> + 'static,
        write: impl Fn(&mut T, U) -> bool + 'static,
    ) -> PartialFormField<Form, U>
    where
        U: Clone + PartialEq + 'static,
    {
        let event_path = self.path().join_projection(name);
        let validation_path = self.validation_path();
        let read_parent = self.clone();
        let write_parent = self;
        let write_read_parent = write_parent.clone();
        PartialFormField::new_dynamic(
            event_path,
            validation_path,
            move |model| {
                let parent = read_parent.read_model(model)?;
                read(&parent).ok_or(FieldAccessError::ValueUnavailable)
            },
            move |model, value| {
                let mut parent = write_read_parent.read_model(model)?;
                if !write(&mut parent, value) {
                    return Err(FieldAccessError::ValueUnavailable.into());
                }
                write_parent.write_model(model, parent)
            },
        )
    }
}

impl<Form, T> PartialFormField<Form, T>
where
    Form: FormState,
    T: Clone + PartialEq + 'static,
{
    fn new_dynamic(
        event_path: FieldPath,
        validation_path: FieldPath,
        read: impl Fn(&Form::Model) -> Result<T, FieldAccessError> + 'static,
        write: impl Fn(&mut Form::Model, T) -> Result<(), FieldMutationError> + 'static,
    ) -> Self {
        Self {
            event_path,
            validation_path,
            read: Arc::new(read),
            write: Arc::new(write),
            marker: PhantomData,
        }
    }

    pub fn path(&self) -> &FieldPath {
        &self.event_path
    }

    pub fn try_value(&self, form: &Entity<Form>, cx: &App) -> Result<T, FieldAccessError> {
        (self.read)(form.read(cx).value())
    }

    pub fn try_set(
        &self,
        form: &Entity<Form>,
        value: T,
        cx: &mut App,
    ) -> Result<(), FieldMutationError> {
        let field = self.clone();
        form.update(cx, move |form, form_cx| {
            let mut candidate = form.value().clone();
            (field.write)(&mut candidate, value)?;
            commit_candidate(
                form,
                candidate,
                &field.event_path,
                &field.validation_path,
                form_cx,
            );
            Ok(())
        })
    }

    pub fn try_validate(
        &self,
        form: &Entity<Form>,
        trigger: ValidationTrigger,
        cx: &mut App,
    ) -> Result<(), FieldAccessError> {
        self.try_value(form, cx)?;
        let validation_path = self.validation_path.clone();
        form.update(cx, move |form, form_cx| {
            let snapshot = form.value().clone();
            let validation = form.__validation_snapshot(
                &snapshot,
                trigger,
                ValidationScope::Field(validation_path),
                form_cx,
            );
            let effect = form
                .__runtime_mut()
                .validation_mut()
                .transition(ReplaceSynchronousValidation(validation));
            apply_validation_effect(effect, form_cx);
        });
        Ok(())
    }

    pub fn try_errors(
        &self,
        form: &Entity<Form>,
        cx: &App,
    ) -> Result<Vec<ValidationIssue>, FieldAccessError> {
        self.try_value(form, cx)?;
        Ok(errors_for_paths(
            form.read(cx),
            &self.event_path,
            &self.validation_path,
        ))
    }

    pub fn try_is_validating(
        &self,
        form: &Entity<Form>,
        cx: &App,
    ) -> Result<bool, FieldAccessError> {
        self.try_value(form, cx)?;
        Ok(form.read(cx).is_validating_at(&self.event_path))
    }

    pub fn try_bind_control(
        &self,
        form: &Entity<Form>,
        cx: &mut App,
    ) -> Result<ControlBinding<Form, T>, FieldAccessError> {
        self.try_value(form, cx)?;
        Ok(ControlBinding::new(form, self.clone()))
    }

    pub fn try_subscribe_in<Owner>(
        &self,
        form: &Entity<Form>,
        window: &Window,
        cx: &mut Context<Owner>,
        listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Result<Subscription, FieldAccessError>
    where
        Owner: 'static,
    {
        self.try_value(form, cx)?;
        Ok(subscribe_to_descriptor(
            self.event_path.clone(),
            form,
            window,
            cx,
            listener,
        ))
    }

    pub fn try_start_async_validation<F, Fut>(
        &self,
        form: &Entity<Form>,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    ) -> Result<(), FieldAccessError>
    where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        let value = self.try_value(form, cx)?;
        start_async_validation(
            form,
            value,
            self.event_path.clone(),
            source.into(),
            trigger,
            validate,
            cx,
        );
        Ok(())
    }

    pub fn try_cancel_async_validation(
        &self,
        form: &Entity<Form>,
        source: &str,
        cx: &mut App,
    ) -> Result<(), FieldAccessError> {
        self.try_value(form, cx)?;
        cancel_async_validation(form, self.event_path.clone(), source, cx);
        Ok(())
    }
}

impl<Form, Item> FormField<Form, Vec<Item>>
where
    Form: FormState,
    Item: Clone + PartialEq + 'static,
{
    pub fn item(self, id: FormItemId) -> PartialFormField<Form, Item> {
        let item_id_at = self
            .item_id_at
            .expect("item() is only available for an identified-array descriptor");
        let event_path = self.path().join_item(id);
        let validation_path = event_path.clone();
        let read_parent = self.clone();
        let write_parent = self;
        let write_read_parent = write_parent.clone();
        PartialFormField::new_dynamic(
            event_path,
            validation_path,
            move |model| {
                let items = read_parent.read_model(model)?;
                locate_item(&items, id, item_id_at).map(|index| items[index].clone())
            },
            move |model, value| {
                let mut items = write_read_parent.read_model(model)?;
                if item_id_at(&vec![value.clone()], 0) != Some(id) {
                    return Err(FieldMutationError::ItemIdentityChanged);
                }
                let index = locate_item(&items, id, item_id_at)?;
                items[index] = value;
                write_parent.write_model(model, items)
            },
        )
    }
}

fn locate_item<Item>(
    items: &Vec<Item>,
    id: FormItemId,
    item_id_at: fn(&Vec<Item>, usize) -> Option<FormItemId>,
) -> Result<usize, FieldAccessError> {
    let matches = (0..items.len())
        .filter(|index| item_id_at(items, *index) == Some(id))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(FieldAccessError::MissingItem(id)),
        _ => Err(FieldAccessError::DuplicateItem(id)),
    }
}

#[doc(hidden)]
pub trait FormFieldParent<Child, T>
where
    Child: FormState,
{
    type Output;

    fn compose(self, child: FormField<Child, T>) -> Self::Output;
}

impl<Child, T> FormField<Child, T>
where
    Child: FormState,
    T: Clone + PartialEq + 'static,
{
    pub fn within<Parent>(self, parent: Parent) -> Parent::Output
    where
        Parent: FormFieldParent<Child, T>,
    {
        parent.compose(self)
    }
}

impl<Parent, Child, T> FormFieldParent<Child, T> for FormField<Parent, Child::Model>
where
    Parent: FormState,
    Child: FormState,
    Child::Model: Clone + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
{
    type Output = FormField<Parent, T>;

    fn compose(self, child: FormField<Child, T>) -> Self::Output {
        let event_path = self.path().join_path(&child.path());
        let validation_path = self.validation_path().join_path(&child.validation_path());
        let schema = child.schema();
        let read_parent = self.clone();
        let read_child = child.clone();
        let write_parent = self;
        let write_read_parent = write_parent.clone();
        let write_child = child;
        FormField::from_dynamic(
            event_path,
            validation_path,
            schema,
            move |model| {
                let parent = read_parent.read_model(model)?;
                read_child.read_model(&parent)
            },
            move |model, value| {
                let mut parent = write_read_parent.read_model(model)?;
                write_child.write_model(&mut parent, value)?;
                write_parent.write_model(model, parent)
            },
        )
    }
}

impl<Parent, Child, T> FormFieldParent<Child, T> for PartialFormField<Parent, Child::Model>
where
    Parent: FormState,
    Child: FormState,
    Child::Model: Clone + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
{
    type Output = PartialFormField<Parent, T>;

    fn compose(self, child: FormField<Child, T>) -> Self::Output {
        let event_path = self.event_path.join_path(&child.path());
        let validation_path = self.validation_path.join_path(&child.validation_path());
        let read_parent = self.clone();
        let read_child = child.clone();
        let write_parent = self;
        let write_read_parent = write_parent.clone();
        let write_child = child;
        PartialFormField::new_dynamic(
            event_path,
            validation_path,
            move |model| {
                let parent = (read_parent.read)(model)?;
                read_child.read_model(&parent)
            },
            move |model, value| {
                let mut parent = (write_read_parent.read)(model)?;
                write_child.write_model(&mut parent, value)?;
                (write_parent.write)(model, parent)
            },
        )
    }
}

fn commit_candidate<Form>(
    form: &mut Form,
    candidate: Form::Model,
    event_path: &FieldPath,
    validation_path: &FieldPath,
    cx: &mut Context<Form>,
) where
    Form: FormState,
{
    if form.value() == &candidate {
        return;
    }
    let validation = form.__validation_snapshot(
        &candidate,
        ValidationTrigger::Change,
        ValidationScope::Field(validation_path.clone()),
        cx,
    );
    let effect = form.__runtime_mut().transition(CommitFieldValue {
        candidate,
        event_path: event_path.clone(),
        validation_path: validation_path.clone(),
        validation,
    });
    apply_form_effect(effect, cx);
}

fn errors_for_paths<Form: FormState>(
    form: &Form,
    event_path: &FieldPath,
    validation_path: &FieldPath,
) -> Vec<ValidationIssue> {
    form.validation_report()
        .issues()
        .iter()
        .filter(|issue| {
            issue.path.as_ref() == Some(event_path)
                || (validation_path != event_path && issue.path.as_ref() == Some(validation_path))
        })
        .cloned()
        .collect()
}

fn paths_intersect(left: &FieldPath, right: &FieldPath) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn subscribe_to_descriptor<Form, Owner>(
    path: FieldPath,
    form: &Entity<Form>,
    window: &Window,
    cx: &mut Context<Owner>,
    mut listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
) -> Subscription
where
    Form: FormState,
    Owner: 'static,
{
    cx.subscribe_in(
        form,
        window,
        move |owner, _, event, window, cx| match event {
            FormEvent::ValueChanged {
                path: changed_path, ..
            } if paths_intersect(&path, changed_path) => listener(owner, window, cx),
            FormEvent::ModelReplaced { .. } => listener(owner, window, cx),
            FormEvent::ValueChanged { .. } | FormEvent::ValidationChanged { .. } => {}
        },
    )
}

fn start_async_validation<Form, T, F, Fut>(
    form: &Entity<Form>,
    value: T,
    path: FieldPath,
    source: Cow<'static, str>,
    trigger: ValidationTrigger,
    validate: F,
    cx: &mut App,
) where
    Form: FormState,
    T: Clone + PartialEq + 'static,
    F: FnOnce(T) -> Fut + 'static,
    Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
{
    form.update(cx, move |form, form_cx| {
        let attempt = form
            .__runtime_mut()
            .validation_mut()
            .transition(NextAsyncValidationAttempt);
        let completion_path = path.clone();
        let completion_source = source.clone();
        let task = form_cx.spawn(async move |weak_form, cx| {
            let result = validate(value).await;
            let Some(form) = weak_form.upgrade() else {
                return;
            };
            form.update(cx, move |form, form_cx| {
                let issue = result.err().map(|issue| {
                    ValidationIssue::field(
                        completion_path.clone(),
                        trigger,
                        ValidationSource::Async(completion_source.clone()),
                        issue.code,
                        issue.message,
                    )
                });
                let effect =
                    form.__runtime_mut()
                        .validation_mut()
                        .transition(CompleteAsyncValidation {
                            path: completion_path,
                            source: completion_source,
                            attempt,
                            issue,
                        });
                apply_validation_effect(effect, form_cx);
            });
        });
        let effect = form
            .__runtime_mut()
            .validation_mut()
            .transition(StartAsyncValidation {
                path,
                source,
                attempt,
                task,
            });
        apply_validation_effect(effect, form_cx);
    });
}

fn cancel_async_validation<Form: FormState>(
    form: &Entity<Form>,
    path: FieldPath,
    source: &str,
    cx: &mut App,
) {
    let source = Cow::Owned(source.to_owned());
    form.update(cx, move |form, form_cx| {
        let effect = form
            .__runtime_mut()
            .validation_mut()
            .transition(CancelAsyncValidation { path, source });
        apply_validation_effect(effect, form_cx);
    });
}
