# ai-chat Manual Testing

This checklist covers the ChatGPT workflow migration in issue #60. Theme changes and WASM extensions are intentionally out of scope.

## Setup

1. Start `ai-chat` from the issue branch.
2. Create at least two folders and three conversations.
3. Put one conversation inside a nested folder.
4. Use one Chinese title or description, for example `命名助手` with `生成更好的名字`.
5. Add at least one user/assistant message to a conversation.

## Conversation Search

1. Open search from the sidebar action.
2. Open search with `Cmd+F` on macOS or `Ctrl+F` on Windows/Linux.
3. Press Escape to close search, then open it again with the same keyboard shortcut.
4. Confirm the search dialog has no title or close button, and the input and selected rows span the dialog width.
5. Open Settings, About, Temporary Conversation, or Message Preview and confirm `Cmd+F`/`Ctrl+F` does not open Home search there.
6. Search by literal title, description, folder name, pinyin, and pinyin initials.
7. Use Up/Down and Enter to select a result.
8. Confirm the selected conversation opens as the active tab.
9. Search for a missing term and confirm the empty state is shown.

## Home Shortcuts

1. Focus different Home regions: sidebar, conversation tab, and an empty area.
2. Press `Cmd+N` on macOS or `Ctrl+N` on Windows/Linux and confirm the add conversation dialog opens.
3. Press `Cmd+Shift+N` on macOS or `Ctrl+Shift+N` on Windows/Linux and confirm the add folder dialog opens.
4. Open the template list and press `Cmd+N` or `Ctrl+N`; confirm the add template dialog opens instead of the add conversation dialog.
5. Open Settings, About, Temporary Conversation, or Message Preview and confirm the Home new conversation/folder shortcuts do not fire there.

## Conversation Export

1. Open a conversation with messages.
2. Use the header export button and export JSON, CSV, and TXT.
3. Repeat the same export name and confirm a suffixed file is created instead of overwriting.
4. Cancel the save dialog and confirm no error notification is shown.
5. Check JSON contains the conversation and messages.
6. Check CSV contains headers and escaped content.
7. Check TXT is readable and includes message role, provider, status, and text.

## Copy To New Conversation

1. Use the conversation header copy button or conversation context menu.
2. Confirm the add conversation dialog is prefilled with name, icon, info, and the same folder.
3. Submit the dialog.
4. Confirm a separate conversation is created.
5. Confirm old messages are not copied.

## Settings

1. Open Settings.
2. Change Language to English and confirm visible settings/sidebar/menu text refreshes.
3. Change Language to 简体中文 and confirm text refreshes again.
4. Change Language to System and restart the app; confirm the setting is preserved.
5. Click Open next to Config File and confirm the system opens `config.toml`.

## Temporary Conversation Window

1. Open the temporary conversation with the configured global hotkey.
2. Confirm the same temporary popup is reused when the hotkey is triggered again.
3. Confirm no detach button or separate normal temporary window is available.
4. Test send, pause, resend, clear, message preview, and save conversation in the temporary popup.
5. Let the popup lose focus and confirm it follows the existing hide behavior.

## Screenshot Overlay

1. Trigger a screenshot shortcut that opens the selection overlay.
2. Press Escape and confirm the overlay closes without creating a shortcut request.
3. Press Escape in a normal Home, Settings, or Temporary window and confirm it does not dispatch screenshot cancel behavior.

## Regression

1. Edit, delete, and move conversations and folders.
2. Clear a normal conversation.
3. Preview, edit, delete, pause, and resend normal conversation messages.
4. Open the template list and template details.
5. Confirm the global temporary hotkey still opens and hides the popup as before.
