use std::cell::Cell;
use std::rc::Rc;

use gpui::{Context, Entity, EventEmitter, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::{
    FieldChangeCause, FieldCodec, FieldCodecError, FieldDraftEvent, FormDraftEvent,
    FormFieldHandle, RequiredValue, SubscriptionSet,
};

use super::ProviderFormField;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ProviderSecretValue {
    pub(in crate::features::settings::provider) field: ProviderFormField,
    pub(in crate::features::settings::provider) value: String,
    pub(in crate::features::settings::provider) changed: bool,
}

impl ProviderSecretValue {
    pub(in crate::features::settings::provider) fn new(
        field: ProviderFormField,
        value: String,
        changed: bool,
    ) -> Self {
        Self {
            field,
            value,
            changed,
        }
    }

    pub(in crate::features::settings::provider) fn key(&self) -> &'static str {
        self.field.key()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ProviderSecretDraft {
    pub(in crate::features::settings::provider) field: ProviderFormField,
    pub(in crate::features::settings::provider) value: String,
    pub(in crate::features::settings::provider) changed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ProviderSecretCodec;

impl FieldCodec<ProviderSecretValue> for ProviderSecretCodec {
    type Draft = ProviderSecretDraft;

    fn draft_from_value(value: &ProviderSecretValue) -> Self::Draft {
        ProviderSecretDraft {
            field: value.field,
            value: value.value.clone(),
            changed: value.changed,
        }
    }

    fn parse(draft: &Self::Draft) -> Result<ProviderSecretValue, FieldCodecError> {
        Ok(ProviderSecretValue::new(
            draft.field,
            draft.value.clone(),
            draft.changed,
        ))
    }
}

impl RequiredValue for ProviderSecretValue {
    fn is_empty_value(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// Adapter for a provider secret input. The form owns the `ProviderSecretDraft`;
/// this component only mirrors its text and marks the draft as changed after a
/// user edit.
pub(in crate::features::settings::provider) fn bind_provider_secret<Form, Owner>(
    field: FormFieldHandle<Form, ProviderSecretDraft>,
    state: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, gpui_form_gpui_component::ComponentBindError>
where
    Form: EventEmitter<FormDraftEvent> + 'static,
    Owner: 'static,
{
    let draft = field
        .draft(cx)
        .map_err(gpui_form_gpui_component::ComponentBindError::from)?;
    state.update(cx, |input, cx| input.set_value(draft.value, window, cx));

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum SyncState {
        #[default]
        Idle,
        FromForm,
        FromComponent,
    }

    let sync = Rc::new(Cell::new(SyncState::Idle));
    let mut subscriptions = SubscriptionSet::new();

    let form_sync = sync.clone();
    let form_state = state.clone();
    subscriptions.push(field.subscribe_in(
        window,
        cx,
        move |_owner, event: &FieldDraftEvent<ProviderSecretDraft>, window, cx| {
            if form_sync.get() == SyncState::FromComponent {
                return;
            }
            form_sync.set(SyncState::FromForm);
            form_state.update(cx, |input, cx| {
                input.set_value(event.draft.value.clone(), window, cx);
            });
            form_sync.set(SyncState::Idle);
        },
    )?);

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, state, event: &InputEvent, window, cx| {
            if !matches!(event, InputEvent::Change) || component_sync.get() == SyncState::FromForm {
                return;
            }
            let value = state.read(cx).value().to_string();
            let Ok(mut draft) = component_field.draft(cx) else {
                return;
            };
            draft.value = value;
            draft.changed = true;
            let sync = component_sync.clone();
            let field = component_field.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                if sync.get() == SyncState::FromForm {
                    return;
                }
                sync.set(SyncState::FromComponent);
                let _ = field.set_draft(draft, FieldChangeCause::UserInput, cx);
                sync.set(SyncState::Idle);
            });
        },
    ));

    Ok(subscriptions)
}
