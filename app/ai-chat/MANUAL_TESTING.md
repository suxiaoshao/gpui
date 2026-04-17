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
3. Search by literal title, description, folder name, pinyin, and pinyin initials.
4. Use Up/Down and Enter to select a result.
5. Confirm the selected conversation opens as the active tab.
6. Search for a missing term and confirm the empty state is shown.

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

## Temporary Conversation Windows

1. Open the temporary conversation with the configured global hotkey.
2. Send or type temporary content.
3. Click the detach button.
4. Confirm a normal temporary conversation window opens and does not hide when it loses focus.
5. Reopen the global-hotkey temporary popup and confirm it is independent from the detached window.
6. In the detached window, test send, pause, resend, clear, message preview, and save conversation.
7. Close the detached window and confirm the global-hotkey popup still works.

## Regression

1. Edit, delete, and move conversations and folders.
2. Clear a normal conversation.
3. Preview, edit, delete, pause, and resend normal conversation messages.
4. Open the template list and template details.
5. Confirm the global temporary hotkey still opens and hides the popup as before.
