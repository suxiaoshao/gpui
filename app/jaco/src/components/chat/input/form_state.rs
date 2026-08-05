use super::ComposerSnapshot;
use crate::{
    components::chat::run_settings::RunSettingsInput,
    features::conversation::attachments::ComposerAttachment,
};
#[derive(Clone, Debug, PartialEq, gpui_form::FormSchema)]
pub(crate) struct ChatInputInput {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) attachments: Vec<ComposerAttachment>,
    #[form(child)]
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
