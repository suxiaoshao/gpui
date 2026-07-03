use gpui::{App, KeyBinding, actions};

use super::shell::KEY_CONTEXT;

actions!(ai_chat2_home, [OpenNewConversation, OpenConversationSearch]);

pub(crate) const OPEN_NEW_CONVERSATION_KEY: &str = "secondary-n";
pub(crate) const OPEN_CONVERSATION_SEARCH_KEY: &str = "secondary-f";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            OPEN_NEW_CONVERSATION_KEY,
            OpenNewConversation,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            OPEN_CONVERSATION_SEARCH_KEY,
            OpenConversationSearch,
            Some(KEY_CONTEXT),
        ),
    ]);
}

#[cfg(test)]
mod tests {
    use super::{OPEN_CONVERSATION_SEARCH_KEY, OPEN_NEW_CONVERSATION_KEY};
    use gpui::Keystroke;
    use gpui_component::kbd::Kbd;

    #[test]
    fn home_shortcut_keys_use_secondary_modifier() {
        assert_eq!(OPEN_NEW_CONVERSATION_KEY, "secondary-n");
        assert_eq!(OPEN_CONVERSATION_SEARCH_KEY, "secondary-f");
    }

    #[test]
    fn home_shortcut_labels_match_platform() {
        let new_conversation = Kbd::format(&Keystroke::parse(OPEN_NEW_CONVERSATION_KEY).unwrap());
        let search = Kbd::format(&Keystroke::parse(OPEN_CONVERSATION_SEARCH_KEY).unwrap());

        assert_eq!(
            new_conversation,
            if cfg!(target_os = "macos") {
                "\u{2318}N"
            } else {
                "Ctrl+N"
            }
        );
        assert_eq!(
            search,
            if cfg!(target_os = "macos") {
                "\u{2318}F"
            } else {
                "Ctrl+F"
            }
        );
    }
}
