use std::{borrow::Cow, fmt, future::Future, marker::PhantomData, sync::Arc};

use gpui::{App, Context, EventEmitter, Subscription, WeakEntity, Window};

use crate::{
    array::{FormItemId, ToFormItemId},
    control::ControlAttachment,
    error::{ValidationIssue, ValidationSource},
    form::{FormEvent, FormStore},
    path::FieldPath,
    trigger::ValidationTrigger,
    validation::{AsyncValidationIssue, ValidationScope},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormFieldError {
    FormReleased,
    ValueUnavailable,
}

impl fmt::Display for FormFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormReleased => f.write_str("the form owning this field has been released"),
            Self::ValueUnavailable => f.write_str("the field value is no longer available"),
        }
    }
}

impl std::error::Error for FormFieldError {}

type ReadField<Form, T> = dyn Fn(&Form) -> Option<T>;
type WriteField<Form, T> = dyn Fn(&mut Form, T, &mut Context<Form>) -> Result<bool, FormFieldError>;

pub struct FormField<Form, T>
where
    Form: FormStore,
{
    form: WeakEntity<Form>,
    field: Form::Field,
    path: FieldPath,
    validation_path: FieldPath,
    read: Arc<ReadField<Form, T>>,
    write: Arc<WriteField<Form, T>>,
    marker: PhantomData<fn(T) -> T>,
}

impl<Form, T> Clone for FormField<Form, T>
where
    Form: FormStore,
{
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            field: self.field,
            path: self.path.clone(),
            validation_path: self.validation_path.clone(),
            read: self.read.clone(),
            write: self.write.clone(),
            marker: PhantomData,
        }
    }
}

impl<Form, T> FormField<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn new(
        form: WeakEntity<Form>,
        field: Form::Field,
        path: FieldPath,
        read: fn(&Form) -> &T,
        write: fn(&mut Form, T, &mut Context<Form>) -> Result<bool, FormFieldError>,
    ) -> Self {
        Self::new_dynamic(
            form,
            field,
            path.clone(),
            path,
            move |form| Some(read(form).clone()),
            write,
        )
    }

    pub fn new_dynamic(
        form: WeakEntity<Form>,
        field: Form::Field,
        path: FieldPath,
        validation_path: FieldPath,
        read: impl Fn(&Form) -> Option<T> + 'static,
        write: impl Fn(&mut Form, T, &mut Context<Form>) -> Result<bool, FormFieldError> + 'static,
    ) -> Self {
        Self {
            form,
            field,
            path,
            validation_path,
            read: Arc::new(read),
            write: Arc::new(write),
            marker: PhantomData,
        }
    }

    pub fn field(&self) -> Form::Field {
        self.field
    }
    pub fn path(&self) -> &FieldPath {
        &self.path
    }
    pub fn validation_path(&self) -> &FieldPath {
        &self.validation_path
    }

    pub fn value(&self, cx: &App) -> Result<T, FormFieldError> {
        let read = self.read.clone();
        self.form
            .read_with(cx, move |form, _| read(form))
            .map_err(|_| FormFieldError::FormReleased)?
            .ok_or(FormFieldError::ValueUnavailable)
    }

    pub fn set(&self, value: T, cx: &mut App) -> Result<(), FormFieldError> {
        self.write_value(value, cx)
    }
    pub fn set_user_value(&self, value: T, cx: &mut App) -> Result<(), FormFieldError> {
        self.write_value(value, cx)
    }

    fn write_value(&self, value: T, cx: &mut App) -> Result<(), FormFieldError> {
        let write = self.write.clone();
        let changed = self
            .form
            .update(cx, move |form, form_cx| write(form, value, form_cx))
            .map_err(|_| FormFieldError::FormReleased)??;
        let _ = changed;
        Ok(())
    }

    pub fn project<U>(
        &self,
        name: &'static str,
        read_child: fn(&T) -> &U,
        write_child: fn(&mut T, U),
    ) -> FormField<Form, U>
    where
        U: Clone + PartialEq + 'static,
    {
        let parent_read = self.read.clone();
        let write_read = self.read.clone();
        let parent_write = self.write.clone();
        let path = self.path.join_field(name);
        FormField::new_dynamic(
            self.form.clone(),
            self.field,
            path.clone(),
            path,
            move |form| parent_read(form).map(|parent| read_child(&parent).clone()),
            move |form, value, cx| {
                let Some(mut parent) = write_read(form) else {
                    return Err(FormFieldError::ValueUnavailable);
                };
                write_child(&mut parent, value);
                parent_write(form, parent, cx)
            },
        )
    }

    pub fn project_value<U>(
        &self,
        name: &'static str,
        read_child: impl Fn(&T) -> Option<U> + 'static,
        write_child: impl Fn(&mut T, U) -> bool + 'static,
    ) -> FormField<Form, U>
    where
        U: Clone + PartialEq + 'static,
    {
        let parent_read = self.read.clone();
        let write_read = self.read.clone();
        let parent_write = self.write.clone();
        FormField::new_dynamic(
            self.form.clone(),
            self.field,
            self.path.join_projection(name),
            self.validation_path.clone(),
            move |form| parent_read(form).and_then(|parent| read_child(&parent)),
            move |form, value, cx| {
                let Some(mut parent) = write_read(form) else {
                    return Err(FormFieldError::ValueUnavailable);
                };
                if !write_child(&mut parent, value) {
                    return Err(FormFieldError::ValueUnavailable);
                }
                parent_write(form, parent, cx)
            },
        )
    }

    pub fn validate(&self, trigger: ValidationTrigger, cx: &mut App) -> Result<(), FormFieldError> {
        let scope = ValidationScope::Field(self.validation_path.clone());
        self.form
            .update(cx, |form, form_cx| form.validate(trigger, scope, form_cx))
            .map(|_| ())
            .map_err(|_| FormFieldError::FormReleased)
    }

    pub fn attach_control(
        &self,
        cx: &mut App,
    ) -> Result<ControlAttachment<Form, T>, FormFieldError> {
        self.value(cx)?;
        Ok(ControlAttachment::new(self.clone()))
    }

    pub fn errors(&self, cx: &App) -> Result<Vec<ValidationIssue>, FormFieldError> {
        self.value(cx)?;
        let path = self.path.clone();
        self.form
            .read_with(cx, move |form, _| form.errors_at(&path))
            .map_err(|_| FormFieldError::FormReleased)
    }

    pub fn is_validating(&self, cx: &App) -> Result<bool, FormFieldError> {
        self.value(cx)?;
        let path = self.path.clone();
        self.form
            .read_with(cx, move |form, _| form.is_validating_at(&path))
            .map_err(|_| FormFieldError::FormReleased)
    }

    pub fn start_async_validation<F, Fut>(
        &self,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    ) -> Result<(), FormFieldError>
    where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        let read = self.read.clone();
        let path = self.path.clone();
        let source = source.into();
        self.form
            .update(cx, move |form, form_cx| {
                let value = read(form).ok_or(FormFieldError::ValueUnavailable)?;
                let generation = form
                    .__runtime_mut()
                    .validation_mut()
                    .next_async_generation();
                let completion_path = path.clone();
                let completion_source = source.clone();
                let task = form_cx.spawn(async move |weak_form, cx| {
                    let result = validate(value).await;
                    if let Some(form) = weak_form.upgrade() {
                        form.update(cx, |form, form_cx| {
                            let issue = result.err().map(|issue| {
                                ValidationIssue::field(
                                    completion_path.clone(),
                                    trigger,
                                    ValidationSource::Async(completion_source.clone()),
                                    issue.code,
                                    issue.message,
                                )
                            });
                            if form.__runtime_mut().validation_mut().finish_async(
                                &completion_path,
                                &completion_source,
                                generation,
                                issue,
                            ) {
                                form_cx.emit(FormEvent::RuntimeChanged);
                                form_cx.notify();
                            }
                        });
                    }
                });
                form.__runtime_mut()
                    .validation_mut()
                    .set_async_task(path, source, generation, task);
                form_cx.emit(FormEvent::RuntimeChanged);
                form_cx.notify();
                Ok(())
            })
            .map_err(|_| FormFieldError::FormReleased)??;
        Ok(())
    }

    pub fn cancel_async_validation(
        &self,
        source: &str,
        cx: &mut App,
    ) -> Result<(), FormFieldError> {
        self.value(cx)?;
        let path = self.path.clone();
        let source = source.to_owned();
        self.form
            .update(cx, move |form, form_cx| {
                if form
                    .__runtime_mut()
                    .validation_mut()
                    .cancel_async(&path, &source)
                {
                    form_cx.emit(FormEvent::RuntimeChanged);
                    form_cx.notify();
                }
            })
            .map_err(|_| FormFieldError::FormReleased)
    }

    pub(crate) fn update_form(
        &self,
        cx: &mut App,
        update: impl FnOnce(&mut Form, &mut Context<Form>),
    ) -> Result<(), FormFieldError> {
        let read = self.read.clone();
        self.form
            .update(cx, move |form, form_cx| {
                if read(form).is_none() {
                    return Err(FormFieldError::ValueUnavailable);
                }
                update(form, form_cx);
                Ok(())
            })
            .map_err(|_| FormFieldError::FormReleased)?
    }

    pub(crate) fn update_form_if_alive(
        &self,
        cx: &mut App,
        update: impl FnOnce(&mut Form, &mut Context<Form>),
    ) -> Result<(), FormFieldError> {
        self.form
            .update(cx, update)
            .map_err(|_| FormFieldError::FormReleased)
    }

    pub fn subscribe_in<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
        mut listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Result<Subscription, FormFieldError>
    where
        Owner: 'static,
        Form: EventEmitter<FormEvent<Form::Field>>,
    {
        let form = self.form.upgrade().ok_or(FormFieldError::FormReleased)?;
        Ok(
            cx.subscribe_in(&form, window, move |owner, _, event, window, cx| {
                if matches!(
                    event,
                    FormEvent::FieldChanged { .. } | FormEvent::ModelReplaced { .. }
                ) {
                    listener(owner, window, cx);
                }
            }),
        )
    }
}

impl<Form, Item> FormField<Form, Vec<Item>>
where
    Form: FormStore,
    Item: Clone + PartialEq + 'static,
{
    pub fn identified_item<Id>(
        &self,
        id: FormItemId,
        item_id: fn(&Item) -> &Id,
    ) -> FormField<Form, Item>
    where
        Id: ToFormItemId + 'static,
    {
        let parent_read = self.read.clone();
        let write_read = self.read.clone();
        let parent_write = self.write.clone();
        let path = self.path.join_item(id);
        FormField::new_dynamic(
            self.form.clone(),
            self.field,
            path.clone(),
            path,
            move |form| {
                let items = parent_read(form)?;
                let mut matches = items
                    .iter()
                    .filter(|item| item_id(item).to_form_item_id() == Some(id));
                let value = matches.next()?.clone();
                matches.next().is_none().then_some(value)
            },
            move |form, value, cx| {
                let Some(mut items) = write_read(form) else {
                    return Err(FormFieldError::ValueUnavailable);
                };
                let matches = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| item_id(item).to_form_item_id() == Some(id))
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                let [index] = matches.as_slice() else {
                    return Err(FormFieldError::ValueUnavailable);
                };
                items[*index] = value;
                parent_write(form, items, cx)
            },
        )
    }
}
