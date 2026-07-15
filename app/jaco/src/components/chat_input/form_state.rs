use super::ComposerSnapshot;
use crate::{
    components::run_settings::{RunSettingsFormStore, RunSettingsInput},
    state::attachments::ComposerAttachment,
};
use gpui::AppContext as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ChatInputFormStore)]
pub(crate) struct ChatInputInput {
    #[form(component = "value")]
    pub(crate) composer: ComposerSnapshot,
    #[form(component = "value")]
    pub(crate) attachments: Vec<ComposerAttachment>,
    #[form(component = "group", store = "RunSettingsFormStore")]
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
