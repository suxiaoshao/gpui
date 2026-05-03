mod choices;
mod dialogs;
mod form;
mod list;
mod segmented;
mod validation;

pub(crate) use list::ShortcutSettingsPage;

const SHORTCUT_DIALOG_WIDTH: f32 = 760.;
const SHORTCUT_DIALOG_MAX_HEIGHT: f32 = 720.;
const SHORTCUT_DIALOG_MARGIN_TOP: f32 = 36.;
