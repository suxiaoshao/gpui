use std::{fmt, marker::PhantomData};

use gpui::{AppContext, Context, EventEmitter, Subscription, WeakEntity, Window};

use crate::{FieldChangeCause, FieldDraftEvent, FieldPath, FormDraftEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormFieldHandleError {
    FormReleased,
}

impl fmt::Display for FormFieldHandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormReleased => f.write_str("form entity has been released"),
        }
    }
}

impl std::error::Error for FormFieldHandleError {}

pub struct FormFieldHandle<Form, Draft> {
    form: WeakEntity<Form>,
    path: FieldPath,
    read: fn(&Form) -> Draft,
    write: fn(&mut Form, Draft, FieldChangeCause, &mut Context<Form>),
    _draft: PhantomData<fn() -> Draft>,
}

impl<Form, Draft> Clone for FormFieldHandle<Form, Draft> {
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            path: self.path.clone(),
            read: self.read,
            write: self.write,
            _draft: PhantomData,
        }
    }
}

impl<Form, Draft> fmt::Debug for FormFieldHandle<Form, Draft> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormFieldHandle")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl<Form, Draft> FormFieldHandle<Form, Draft>
where
    Form: 'static,
    Draft: Clone + 'static,
{
    pub fn new(
        form: WeakEntity<Form>,
        path: FieldPath,
        read: fn(&Form) -> Draft,
        write: fn(&mut Form, Draft, FieldChangeCause, &mut Context<Form>),
    ) -> Self {
        Self {
            form,
            path,
            read,
            write,
            _draft: PhantomData,
        }
    }

    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    pub fn draft<C: AppContext>(&self, cx: &C) -> Result<Draft, FormFieldHandleError> {
        self.form
            .read_with(cx, |form, _| (self.read)(form))
            .map_err(|_| FormFieldHandleError::FormReleased)
    }

    pub fn set_user_draft<C: AppContext>(
        &self,
        draft: Draft,
        cx: &mut C,
    ) -> Result<(), FormFieldHandleError> {
        self.set_draft(draft, FieldChangeCause::UserInput, cx)
    }

    pub fn set_draft<C: AppContext>(
        &self,
        draft: Draft,
        cause: FieldChangeCause,
        cx: &mut C,
    ) -> Result<(), FormFieldHandleError> {
        self.form
            .update(cx, |form, form_cx| {
                (self.write)(form, draft, cause, form_cx)
            })
            .map(|_| ())
            .map_err(|_| FormFieldHandleError::FormReleased)
    }

    pub fn subscribe_in<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
        mut listener: impl FnMut(&mut Owner, &FieldDraftEvent<Draft>, &mut Window, &mut Context<Owner>)
        + 'static,
    ) -> Result<Subscription, FormFieldHandleError>
    where
        Owner: 'static,
        Form: EventEmitter<FormDraftEvent>,
    {
        let form = self
            .form
            .upgrade()
            .ok_or(FormFieldHandleError::FormReleased)?;
        let path = self.path.clone();
        Ok(
            cx.subscribe_in(&form, window, move |owner, _, event, window, cx| {
                if event.path() != &path {
                    return;
                }
                let Some(draft) = event.draft::<Draft>() else {
                    return;
                };
                let event = FieldDraftEvent {
                    draft: draft.clone(),
                    cause: event.cause(),
                };
                listener(owner, &event, window, cx);
            }),
        )
    }
}
