use gpui::{App, AppContext as _, Entity, Window};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FieldPath, FormComponentBinding,
    ValidationTrigger,
};

#[derive(Debug)]
pub struct BoolComponentState {
    value: bool,
    disabled: bool,
    required: bool,
}

impl BoolComponentState {
    pub fn value(&self) -> bool {
        self.value
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_required(&self) -> bool {
        self.required
    }
}

pub struct BoolBinding;

impl FormComponentBinding<bool> for BoolBinding {
    type State = BoolComponentState;
    type Draft = bool;

    fn new_state(
        initial: &bool,
        options: ComponentStateOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let value = *initial;
        cx.new(|_| BoolComponentState {
            value,
            disabled: options.disabled,
            required: options.required,
        })
    }

    fn draft_from_value(value: &bool) -> Self::Draft {
        *value
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: FieldPath,
        _trigger: ValidationTrigger,
        _cx: &App,
    ) -> Result<bool, Box<FieldError>> {
        Ok(*draft)
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &bool,
        _cause: FieldChangeCause,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.value = *value;
        });
    }

    fn set_disabled(
        state: &Entity<Self::State>,
        disabled: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.disabled = disabled;
        });
    }

    fn set_required(
        state: &Entity<Self::State>,
        required: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.required = required;
        });
    }

    fn focus(_state: &Entity<Self::State>, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}
