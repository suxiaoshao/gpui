---
name: gpui-computer-use-debugging
description: Use when validating or debugging local GPUI desktop app UI behavior with Computer Use across gpui workspace apps, including runtime layout issues, dialogs, scrolling, focus, window state, and manual verification after code changes or bundling.
---

# GPUI Computer Use Debugging

Use this skill to verify actual desktop behavior, not just compile-time correctness. It applies to all GPUI apps in this workspace and is especially useful after UI/layout changes where screenshots, scrolling, focus, dialog sizing, or hit targets matter.

## App Targets

Use the app under test from the user's request or the changed files:

- `ai-chat`: bundle command `cargo run -p xtask -- bundle ai-chat`, macOS bundle `target/release/bundle/macos/AI Chat.app`, bundle id `top.sushao.ai-chat`.
- `feiwen`: bundle command `cargo run -p xtask -- bundle feiwen`, macOS bundle `target/release/bundle/macos/Feiwen.app`, bundle id `top.sushao.feiwen`.
- `http-client`: bundle command `cargo run -p xtask -- bundle http-client`, macOS bundle `target/release/bundle/macos/HTTP Client.app`, bundle id `top.sushao.http-client`.
- `novel-download`: bundle command `cargo run -p xtask -- bundle novel-download`, macOS bundle `target/release/bundle/macos/Novel Download.app`, bundle id `top.sushao.novel-download`.

## Workflow

1. Build the app variant that matches the user-visible target.
   - Prefer the app-specific `cargo run -p xtask -- bundle <app>` command from the App Targets table when validating packaged behavior.
   - For fast development checks where packaging is irrelevant, run the app package directly, for example `cargo run -p feiwen`.
   - Treat `actool` / CoreSimulator icon warnings as non-fatal when the command still emits the expected app bundle.
   - Do not replace an installed `/Applications/*.app` unless the user explicitly asks or confirms the overwrite.

2. Launch the exact artifact under test.
   - Prefer the target bundle after local changes:
     `open "/Users/sushao/Documents/code/gpui/target/release/bundle/macos/<Product Name>.app"`.
   - Quit any previous instance of the same app first if stale UI is likely:
     `osascript -e 'tell application id "<bundle-id>" to quit' || true`.
   - Commands that open GUI apps generally need sandbox escalation.

3. Attach Computer Use to the app.
   - Call `mcp__computer_use__.get_app_state({"app":"<bundle-id>"})` before any click, key, or scroll in each assistant turn.
   - Use the returned screenshot plus accessibility tree together. Prefer element indices when stable; use coordinates when GPUI controls are not exposed as useful AX elements.
   - If a separate window, dialog, popover, or settings surface is opened, call `get_app_state` again before interacting.

4. Navigate with visible evidence.
   - Use the app's actual navigation model rather than assuming ai-chat's sidebar/settings layout.
   - For table/list workflows, verify row selection, sorting, scrolling, and action controls.
   - For settings or form workflows, click the page nav item first, then use row action icons for view/edit/delete when that app has such a structure.
   - For dialogs, verify the actual states the user mentioned: default view, scroll position after wheel, focused input/editor, confirmation state, save/cancel footer, and long-content behavior.

5. Test interactions, not only screenshots.
   - Use `mcp__computer_use__.scroll` on the window element to confirm dialog body scrolling.
   - Click inside editors before testing editor-specific keyboard behavior such as `Page_Down`.
   - Re-query `get_app_state` after Computer Use reports the user or app changed state.
   - When visual validation is limited by accessibility or coordinates, report exactly what was verified and what remains inferred.

6. Keep validation tied to the current build.
   - If code changes after a launch, rebuild and relaunch before judging the UI.
   - Say whether validation used the target bundle or the installed `/Applications` app.
   - For layout fixes, record the command checks run alongside Computer Use evidence: typically `cargo fmt`, app-focused tests such as `cargo test -p <app>`, app-focused clippy such as `cargo clippy -p <app> --all-targets --all-features -- -D warnings`, and the bundle command when packaging was part of the validation.

## Practical Checks

- Dialog width: confirm controls are not clipped at the right edge.
- Dialog height: confirm body scroll works and footer remains reachable.
- Nested scrolling: test both dialog body scroll and editor/content scroll when code editors or text views are involved.
- Footer buttons: confirm natural button width and right alignment rather than full-row stretching.
- Content density: compare labels, select controls, dividers, and repeated metadata against the user screenshot.
- Tables/lists: verify visible row counts, horizontal/vertical scrolling, row actions, and whether fixed columns remain reachable.
- Popovers/menus: verify open position, clipping, keyboard dismissal, and focus return.
- Destructive flows: open confirmation dialogs but do not confirm deletion unless the user explicitly asks for the destructive action.

## Reporting

Summarize validation in concrete terms:

- artifact launched, for example `target/release/bundle/macos/AI Chat.app`
- window or dialog inspected
- interactions performed, for example click edit, scroll down, focus editor, press `Page_Down`
- commands run and whether they passed
- warnings that did not block the bundle, especially `actool` icon injection warnings
