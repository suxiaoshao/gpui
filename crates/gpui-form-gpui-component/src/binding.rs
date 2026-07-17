use std::{cell::Cell, fmt, rc::Rc};

use crate::bool::{BoolComponentEvent, BoolComponentState};
use gpui::{Context, Entity, EventEmitter, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    FieldChangeCause, FormFieldHandle, FormFieldHandleError, FormStoreEvent, SubscriptionSet,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ComponentSyncState {
    #[default]
    Idle,
    FromForm,
    FromComponent,
}

#[derive(Debug)]
pub enum ComponentBindError {
    Form(FormFieldHandleError),
}

impl fmt::Display for ComponentBindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Form(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ComponentBindError {}

impl From<FormFieldHandleError> for ComponentBindError {
    fn from(value: FormFieldHandleError) -> Self {
        Self::Form(value)
    }
}

fn project_input<Form, Owner>(
    field: &FormFieldHandle<Form, String>,
    state: &Entity<InputState>,
    cx: &mut Context<Owner>,
    window: &mut Window,
) -> Result<(), ComponentBindError>
where
    Form: 'static,
    Owner: 'static,
{
    let draft = field.draft(cx)?;
    state.update(cx, |input, cx| input.set_value(draft, window, cx));
    Ok(())
}

pub fn bind_input<Form, Owner>(
    field: FormFieldHandle<Form, String>,
    state: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>
where
    Form: EventEmitter<gpui_form::FormDraftEvent> + 'static,
    Owner: 'static,
{
    let sync = Rc::new(Cell::new(ComponentSyncState::Idle));
    project_input(&field, state, cx, window)?;

    let form_sync = sync.clone();
    let form_state = state.clone();
    let mut subscriptions = SubscriptionSet::new();
    subscriptions.push(
        field.subscribe_in(window, cx, move |_owner, event, window, cx| {
            if form_sync.get() == ComponentSyncState::FromComponent {
                return;
            }
            form_sync.set(ComponentSyncState::FromForm);
            form_state.update(cx, |input, cx| {
                input.set_value(event.draft.clone(), window, cx);
            });
            form_sync.set(ComponentSyncState::Idle);
        })?,
    );

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, state, event: &InputEvent, window, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            if component_sync.get() == ComponentSyncState::FromForm {
                return;
            }
            let draft = state.read(cx).value().to_string();
            let sync = component_sync.clone();
            let field = component_field.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                if sync.get() == ComponentSyncState::FromForm {
                    return;
                }
                sync.set(ComponentSyncState::FromComponent);
                let _ = field.set_user_draft(draft, cx);
                sync.set(ComponentSyncState::Idle);
            });
        },
    ));

    Ok(subscriptions)
}

pub fn bind_number<Form, Owner>(
    field: FormFieldHandle<Form, String>,
    state: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>
where
    Form: EventEmitter<gpui_form::FormDraftEvent> + 'static,
    Owner: 'static,
{
    bind_input(field, state, window, cx)
}

pub fn bind_bool<Form, Owner>(
    field: FormFieldHandle<Form, bool>,
    state: &Entity<BoolComponentState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>
where
    Form: EventEmitter<gpui_form::FormDraftEvent> + 'static,
    Owner: 'static,
{
    let initial = field.draft(cx)?;
    state.update(cx, |state, cx| state.set_value(initial, cx));
    let sync = Rc::new(Cell::new(ComponentSyncState::Idle));
    let mut subscriptions = SubscriptionSet::new();

    let form_sync = sync.clone();
    let form_state = state.clone();
    subscriptions.push(
        field.subscribe_in(window, cx, move |_owner, event, _window, cx| {
            if form_sync.get() == ComponentSyncState::FromComponent {
                return;
            }
            form_sync.set(ComponentSyncState::FromForm);
            form_state.update(cx, |state, cx| state.set_value(event.draft, cx));
            form_sync.set(ComponentSyncState::Idle);
        })?,
    );

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, _state, event: &BoolComponentEvent, window, cx| {
            if component_sync.get() == ComponentSyncState::FromForm {
                return;
            }
            let value = event.value();
            let sync = component_sync.clone();
            let field = component_field.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                if sync.get() == ComponentSyncState::FromForm {
                    return;
                }
                sync.set(ComponentSyncState::FromComponent);
                let _ = field.set_user_draft(value, cx);
                sync.set(ComponentSyncState::Idle);
            });
        },
    ));
    Ok(subscriptions)
}

pub fn subscribe_form_changes<Owner, Form, Field>(
    form: &Entity<Form>,
    window: &Window,
    cx: &mut Context<Owner>,
    mut listener: impl FnMut(Field, FieldChangeCause) + 'static,
) -> Subscription
where
    Owner: 'static,
    Form: EventEmitter<FormStoreEvent<Field>> + 'static,
    Field: Clone + 'static,
{
    cx.subscribe_in(form, window, move |_owner, _, event, _, _| {
        let FormStoreEvent::FieldChanged { field, cause } = event;
        listener(field.clone(), *cause);
    })
}
