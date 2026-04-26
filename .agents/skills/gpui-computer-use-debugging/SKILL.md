---
name: gpui-computer-use-debugging
description: Use when validating or debugging local GPUI desktop app UI behavior with Computer Use, especially ai-chat runtime layout issues, dialogs, scrolling, focus, window state, and manual verification after code changes or bundling.
---

# GPUI Computer Use Debugging

Use this skill to verify actual desktop behavior, not just compile-time correctness. It is especially useful after UI/layout changes where screenshots, scrolling, focus, dialog sizing, or hit targets matter.

## Workflow

1. Build the app variant that matches the user-visible target.
   - For ai-chat release bundle, run `cargo run -p xtask -- bundle-ai-chat`.
   - Treat `actool` / CoreSimulator icon warnings as non-fatal when the command still emits `target/release/bundle/macos/AI Chat.app`.
   - Do not replace `/Applications/AI Chat.app` unless the user explicitly asks or confirms the overwrite.

2. Launch the exact artifact under test.
   - Prefer the target bundle after local changes:
     `open "/Users/sushao/Documents/code/gpui/target/release/bundle/macos/AI Chat.app"`.
   - Quit any previous ai-chat process first if stale UI is likely:
     `osascript -e 'tell application id "top.sushao.ai-chat" to quit' || true`.
   - Commands that open GUI apps generally need sandbox escalation.

3. Attach Computer Use to the app.
   - Call `mcp__computer_use__.get_app_state({"app":"top.sushao.ai-chat"})` before any click, key, or scroll in each assistant turn.
   - Use the returned screenshot plus accessibility tree together. Prefer element indices when stable; use coordinates when GPUI controls are not exposed as useful AX elements.
   - If a separate Settings window is opened, call `get_app_state` again before interacting.

4. Navigate with visible evidence.
   - In ai-chat, Settings is usually in the left sidebar action area.
   - In Settings, click the page nav item first, then use row action icons for view/edit/delete.
   - For dialogs, verify the actual states the user mentioned: default view, scroll position after wheel, focused editor, delete confirmation, save/cancel footer, and long-content behavior.

5. Test interactions, not only screenshots.
   - Use `mcp__computer_use__.scroll` on the window element to confirm dialog body scrolling.
   - Click inside editors before testing editor-specific keyboard behavior such as `Page_Down`.
   - Re-query `get_app_state` after Computer Use reports the user or app changed state.
   - When visual validation is limited by accessibility or coordinates, report exactly what was verified and what remains inferred.

6. Keep validation tied to the current build.
   - If code changes after a launch, rebuild and relaunch before judging the UI.
   - Say whether validation used the target bundle or the installed `/Applications` app.
   - For layout fixes, record the command checks run alongside Computer Use evidence: typically `cargo fmt`, `cargo test -p ai-chat`, `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`, and the bundle command.

## Practical Checks

- Dialog width: confirm controls are not clipped at the right edge.
- Dialog height: confirm body scroll works and footer remains reachable.
- Nested scrolling: test both dialog body scroll and editor/content scroll when code editors or text views are involved.
- Footer buttons: confirm natural button width and right alignment rather than full-row stretching.
- Content density: compare labels, select controls, dividers, and repeated metadata against the user screenshot.
- Destructive flows: open confirmation dialogs but do not confirm deletion unless the user explicitly asks for the destructive action.

## Reporting

Summarize validation in concrete terms:

- artifact launched, for example `target/release/bundle/macos/AI Chat.app`
- window or dialog inspected
- interactions performed, for example click edit, scroll down, focus editor, press `Page_Down`
- commands run and whether they passed
- warnings that did not block the bundle, especially `actool` icon injection warnings
