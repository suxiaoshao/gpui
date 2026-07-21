use gpui::{Entity, SharedString};

use super::project_control::ProjectControlState;

use crate::components::run_settings::{
    ApprovalControlState, ModelControlState, ReasoningControlState,
};

/// Availability is part of the ChatForm composition contract.  A hidden
/// control contributes no layout, while a disabled control keeps its state so
/// the same visual control can still show its value and placeholder.
#[derive(Clone)]
pub(crate) enum ControlSlot<T> {
    Hidden,
    Disabled(T),
    Enabled(T),
}

impl<T> ControlSlot<T> {
    pub(crate) fn as_ref(&self) -> ControlSlot<&T> {
        match self {
            Self::Hidden => ControlSlot::Hidden,
            Self::Disabled(value) => ControlSlot::Disabled(value),
            Self::Enabled(value) => ControlSlot::Enabled(value),
        }
    }

    pub(crate) fn value(&self) -> Option<&T> {
        match self {
            Self::Hidden => None,
            Self::Disabled(value) | Self::Enabled(value) => Some(value),
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    pub(crate) fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }
}

#[derive(Clone, Default)]
pub(crate) struct AttachmentControlState {
    pub(crate) form: Option<Entity<crate::components::chat_input::ChatInputFormStore>>,
}

#[derive(Clone, Default)]
pub(crate) struct AddAttachmentControl;

#[derive(Clone, Default)]
pub(crate) struct PrimaryActionControlState {
    pub(crate) agent_running: bool,
    pub(crate) can_submit: bool,
}

#[derive(Clone)]
pub(crate) struct RunSettingsControls {
    pub(crate) model: ControlSlot<Entity<ModelControlState>>,
    pub(crate) reasoning: ControlSlot<Entity<ReasoningControlState>>,
    pub(crate) approval: ControlSlot<Entity<ApprovalControlState>>,
}

#[derive(Clone)]
pub(crate) struct ChatFormControls {
    pub(crate) project: ControlSlot<Entity<ProjectControlState>>,
    pub(crate) composer: ControlSlot<Entity<crate::components::chat_input::ComposerEditor>>,
    pub(crate) attachments: ControlSlot<Entity<AttachmentControlState>>,
    pub(crate) add_attachment: ControlSlot<AddAttachmentControl>,
    pub(crate) run_settings: RunSettingsControls,
    pub(crate) primary_action: ControlSlot<Entity<PrimaryActionControlState>>,
}

#[cfg(test)]
mod tests {
    use super::ControlSlot;

    #[test]
    fn control_slot_tracks_visibility_and_interactivity() {
        let hidden: ControlSlot<u8> = ControlSlot::Hidden;
        let disabled = ControlSlot::Disabled(1_u8);
        let enabled = ControlSlot::Enabled(2_u8);

        assert!(!hidden.is_visible());
        assert!(!hidden.is_enabled());
        assert!(hidden.value().is_none());

        assert!(disabled.is_visible());
        assert!(!disabled.is_enabled());
        assert_eq!(disabled.value(), Some(&1));

        assert!(enabled.is_visible());
        assert!(enabled.is_enabled());
        assert_eq!(enabled.value(), Some(&2));
    }
}
