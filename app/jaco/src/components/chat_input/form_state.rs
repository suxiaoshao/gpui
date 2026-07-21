use super::ComposerSnapshot;
use crate::{components::run_settings::RunSettingsInput, state::attachments::ComposerAttachment};
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ChatInputFormStore)]
pub(crate) struct ChatInputInput {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) attachments: Vec<ComposerAttachment>,
    #[form(group)]
    pub(crate) run_settings: RunSettingsInput,
}

impl ChatInputInput {
    pub(crate) fn new(
        composer: ComposerSnapshot,
        attachments: Vec<ComposerAttachment>,
        run_settings: RunSettingsInput,
    ) -> Self {
        Self {
            composer,
            attachments,
            run_settings,
        }
    }
}
