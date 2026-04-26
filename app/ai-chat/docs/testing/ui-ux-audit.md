# ai-chat UI/UX Audit Manual Testing

This checklist covers the UI/UX remediation tracked in issue #84. It is organized by interface category so each page can be verified independently.

## Setup

1. Start `ai-chat` from the issue branch.
2. Test once in light mode and once in dark mode.
3. Use at least one configured OpenAI provider, one Ollama provider, two templates, one conversation with multiple messages, and one temporary conversation.
4. Include Chinese and English text in at least one conversation or template.
5. Do not paste or record a real API key in issue comments, screenshots, logs, or test notes.

## Security & Privacy

1. Open Settings, then Provider.
2. Confirm the OpenAI API Key field is masked by default.
3. Confirm the user can intentionally reveal and hide the key.
4. Confirm copying, editing, saving, changing tabs, closing Settings, and reopening Settings preserve the stored key without exposing it by default.
5. Confirm notifications, logs, issue comments, and test output do not contain the real key.
6. Confirm screenshots used for review do not show an unmasked key.

## Message Preview

1. Open a normal conversation with user and assistant messages.
2. Open message preview from each message action.
3. Confirm the window title clearly identifies the message being inspected.
4. Confirm metadata is visible, compact, and readable at the default preview-window size.
5. Confirm Preview mode renders message text as readable content, not as disabled form fields.
6. Confirm citations and send content are readable when present and do not dominate the first viewport when empty.
7. Switch to Edit mode.
8. Confirm editable fields are visually distinct from read-only fields.
9. Edit message text and save.
10. Confirm success and error notifications are clear, localized, and anchored to the preview workflow.
11. Resize the preview window narrower and shorter, then confirm controls remain reachable and text does not overlap.
12. Confirm there is only one meaningful vertical scroll surface for the detail content.

## Template Settings

1. Open Settings > Templates.
2. Search by template name and description.
3. Open a template with one prompt and a template with multiple prompts.
4. Confirm the list row shows only icon, name, description, and prompt count.
5. Confirm the view dialog shows icon, name, description, ID, and prompt blocks without duplicate metadata.
6. Confirm each prompt clearly shows index, role, and content without relying only on avatar imagery.
7. Confirm long prompt content wraps cleanly and remains selectable.
8. Confirm add and edit use the same form dialog and keep prompt rows scrollable.
9. Confirm delete opens a destructive confirmation dialog with localized copy.
10. Confirm add, edit, and delete refresh the Settings > Templates list without a full app restart.

## Settings Search

1. Open Settings.
2. Confirm the settings search input is visibly discoverable in the sidebar or header.
3. Search for General, Appearance, Provider, Templates, Shortcuts, API Key, HTTP Proxy, and Theme.
4. Confirm matching pages or settings remain visible and non-matching areas are reduced or hidden consistently.
5. Confirm clearing search restores the default settings navigation.
6. Confirm keyboard focus starts in a predictable place and Tab navigation reaches search, sidebar items, and page controls.
7. Confirm the search layout does not collapse into an unlabeled icon-only control at the default settings-window size.

## Provider Settings

1. Open Settings, then Provider.
2. Confirm Ollama and OpenAI sections have clear grouping and spacing.
3. Confirm Base URL and HTTP Proxy fields align with their labels.
4. Confirm long Base URL values are readable, editable, and do not overflow neighboring controls.
5. Confirm API Key masking does not interfere with saving provider settings.
6. Switch away from Provider and back, then confirm values and masked state behave correctly.

## Shortcut Bindings

1. Open Settings, then Shortcuts.
2. Confirm the shortcut binding list is usable at the default settings-window size.
3. Confirm the template, model, mode, hotkey, enabled state, and actions are visible or reachable without hidden right-edge controls.
4. If details are collapsed into an expandable row or popover, confirm preset and send content can still be edited.
5. Add a shortcut binding.
6. Edit template, model, mode, preset, send content, hotkey, and enabled state.
7. Confirm invalid hotkeys show inline validation without changing row height unpredictably.
8. Confirm delete opens a destructive confirmation dialog.
9. Resize the settings window and confirm the table or list remains readable.
10. Confirm keyboard navigation through rows and controls is predictable.

## Temporary Conversation

1. Open temporary conversation from the app menu or configured global hotkey.
2. Confirm the empty state explains what can be done next without filling the page with blank space.
3. Confirm clear and save actions are hidden or disabled when there are no messages.
4. Confirm template selection is discoverable without knowing the slash shortcut.
5. Confirm typing `/` still opens the template picker.
6. Select a template, send a message, pause generation, and clear the temporary conversation.
7. Save a temporary conversation and confirm the add conversation dialog is prefilled with expected message content.
8. Confirm losing focus still follows the existing temporary-window hide behavior.
9. Confirm controls remain reachable in compact window sizes.

## Chat Role Visuals

1. Inspect normal conversation messages for Developer, User, and Assistant roles.
2. Inspect template detail prompts for the same roles.
3. Confirm role visuals use one coherent icon, badge, or avatar style across both places.
4. Confirm status indicators remain visible in light and dark mode.
5. Confirm role recognition does not depend on decorative or mismatched photo assets.
6. Confirm role labels or tooltips are available where icon meaning is not obvious.

## Localization

1. Set language to Simplified Chinese.
2. Open message preview, template detail, settings, shortcut bindings, and temporary conversation.
3. Confirm no raw localization keys such as `section-content` are visible.
4. Confirm button labels such as reload are translated or intentionally product-named.
5. Set language to English and repeat the same pages.
6. Confirm text fits within buttons, table headers, tabs, and narrow controls in both languages.

## Regression

1. Create, edit, move, and delete conversations and folders.
2. Open conversation search and confirm it still opens from sidebar and keyboard shortcut.
3. Send, pause, resend, copy, preview, edit, and delete normal conversation messages.
4. Open, add, edit, and delete templates.
5. Export a conversation as JSON, CSV, and TXT.
6. Copy a conversation to a new conversation.
7. Change appearance mode and theme, then confirm the audited pages still have sufficient contrast.
8. Restart the app and confirm settings, provider values, sidebar width, open tabs, and drafts persist as before.
9. Run the existing `app/ai-chat/MANUAL_TESTING.md` regression sections that overlap with changed behavior.
