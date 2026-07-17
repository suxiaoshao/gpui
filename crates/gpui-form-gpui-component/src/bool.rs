use gpui::{Context, EventEmitter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolComponentEvent {
    Changed(bool),
}

impl BoolComponentEvent {
    pub fn value(self) -> bool {
        match self {
            Self::Changed(value) => value,
        }
    }
}
#[derive(Debug)]
pub struct BoolComponentState {
    value: bool,
    disabled: bool,
    required: bool,
}

impl EventEmitter<BoolComponentEvent> for BoolComponentState {}

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

    pub fn set_value(&mut self, value: bool, cx: &mut Context<Self>) {
        if self.value == value {
            return;
        }
        self.value = value;
        cx.emit(BoolComponentEvent::Changed(value));
        cx.notify();
    }
}
