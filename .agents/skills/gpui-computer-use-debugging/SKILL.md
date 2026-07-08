---
name: gpui-computer-use-debugging
description: Use when validating or debugging local GPUI desktop app UI behavior with Computer Use across gpui workspace apps, including runtime layout issues, dialogs, scrolling, focus, window state, and manual verification after code changes or bundling.
---

# GPUI Computer Use Debugging

Use this skill to verify actual desktop behavior, not just compile-time correctness. It applies to all GPUI apps in this workspace and is especially useful after UI/layout changes where screenshots, scrolling, focus, dialog sizing, or hit targets matter.

## App Targets

Use the app under test from the user's request or the changed files:

- `jaco`: bundle command `cargo run -p xtask -- bundle jaco`, macOS bundle `target/release/bundle/macos/Jaco.app`, bundle id `top.sushao.jaco`.
- `feiwen`: bundle command `cargo run -p xtask -- bundle feiwen`, macOS bundle `target/release/bundle/macos/Feiwen.app`, bundle id `top.sushao.feiwen`.
- `http-client`: bundle command `cargo run -p xtask -- bundle http-client`, macOS bundle `target/release/bundle/macos/HTTP Client.app`, bundle id `top.sushao.http-client`.
- `novel-download`: bundle command `cargo run -p xtask -- bundle novel-download`, macOS bundle `target/release/bundle/macos/Novel Download.app`, bundle id `top.sushao.novel-download`.

## Product Test Docs

Before doing manual UI validation for an app that has product QA docs, open the app-local README and the matching `docs/tests` page. These docs describe user-facing flows, required test data, expected results, edge cases, and cleanup steps.

- `jaco`: no app-local product QA docs tree is currently present under `app/jaco/docs`; use focused code tests and scenario-specific notes until docs are added.
- `feiwen`: app README `app/feiwen/README.md`, feature docs `app/feiwen/docs/features/README.md`, test docs `app/feiwen/docs/tests/README.md`.

Use the test docs as the source of truth for manual/Computer Use validation scope:

- Pick the test document that matches the changed feature, for example `app/feiwen/docs/tests/fetch/run-states.md` when validating Feiwen.
- Follow the documented test case structure: `测试目标`, `数据隔离`, `测试前提`, `测试数据`, `测试步骤`, `预期结果`, `边缘情况`, `清理`.
- Execute the exact field values and button paths in the test steps instead of inventing ad hoc data while debugging.
- If the docs require pre-existing folders, conversations, messages, novels, authors, tags, templates, shortcuts, or fetch pages and the test is not about creating those records, pre-seed a test database before launching the app.

## Workflow

1. Build the app variant that matches the user-visible target.
   - For Computer Use validation, always build a temporary local `.app` bundle first with the app-specific `cargo run -p xtask -- bundle <app>` command from the App Targets table.
   - Do not use `cargo run -p <app>` for Computer Use validation. Computer Use attaches to macOS apps, and using a bundle id after `cargo run` can attach to an older installed `/Applications/*.app` instead of the current build.
   - `cargo run -p <app>` is only acceptable for non-Computer-Use checks such as logs, startup smoke tests, or compile-time behavior.
   - Treat `actool` / CoreSimulator icon warnings as non-fatal when the command still emits the expected app bundle.
   - Do not replace an installed `/Applications/*.app` unless the user explicitly asks or confirms the overwrite.

2. Prepare isolated test data before launch.
   - Do not use the user's real app data as the target of a debug run unless the user explicitly asks for that.
   - Prefer an isolated temporary data directory or test SQLite database such as `/tmp/jaco-qa-data` or `/tmp/feiwen-qa-data`.
   - For `jaco`, use isolated config/data directories, test projects, conversations, prompts, shortcuts, MCP server fixtures, and API key placeholders.
   - For `feiwen`, use a test SQLite database plus local mock HTTP service data for fetch tests; do not use a real Cookie or real production crawl target. Use the fixtures and values described in `app/feiwen/docs/tests/README.md`.
   - If the app does not yet expose a documented way to override the data directory, stop and identify a safe test-data launch method before interacting with user data.

3. Launch the exact artifact under test.
   - Launch the target bundle after local changes by full path:
     `open "/Users/sushao/Documents/code/gpui/target/release/bundle/macos/<Product Name>.app"`.
   - Quit any previous instance of the same app first if stale UI is likely:
     `osascript -e 'tell application id "<bundle-id>" to quit' || true`.
   - When isolation requires environment variables such as `HOME`, `CARGO_HOME`, or `RUSTUP_HOME`, launch the bundled executable inside the `.app` with those variables instead of opening an installed app.
   - Commands that open GUI apps generally need sandbox escalation.

4. Attach Computer Use to the app.
   - Call `mcp__computer_use__.get_app_state({"app":"<full path to target .app>"})` before any click, key, or scroll in each assistant turn.
   - Do not attach by bundle id when validating local changes; bundle ids are ambiguous when an older installed app also exists.
   - Verify the returned app state `App=...` path is the target bundle under the current workspace, for example `target/release/bundle/macos/Jaco.app`. If it shows `/Applications/*.app`, stop and relaunch the correct local bundle.
   - Use the returned screenshot plus accessibility tree together. Prefer element indices when stable; use coordinates when GPUI controls are not exposed as useful AX elements.
   - If a separate window, dialog, popover, or settings surface is opened, call `get_app_state` again before interacting.

5. Navigate with visible evidence.
   - Use the app's actual navigation model rather than assuming another app's sidebar/settings layout.
   - When a relevant QA test doc exists, follow its documented navigation path and test steps first.
   - For table/list workflows, verify row selection, sorting, scrolling, and action controls.
   - For settings or form workflows, click the page nav item first, then use row action icons for view/edit/delete when that app has such a structure.
   - For dialogs, verify the actual states the user mentioned: default view, scroll position after wheel, focused input/editor, confirmation state, save/cancel footer, and long-content behavior.

6. Test interactions, not only screenshots.
   - Use `mcp__computer_use__.scroll` on the window element to confirm dialog body scrolling.
   - Click inside editors before testing editor-specific keyboard behavior such as `Page_Down`.
   - Re-query `get_app_state` after Computer Use reports the user or app changed state.
   - When visual validation is limited by accessibility or coordinates, report exactly what was verified and what remains inferred.

7. Keep validation tied to the current build.
   - If code changes after a launch, rebuild and relaunch before judging the UI.
   - Say whether validation used the target bundle or the installed `/Applications` app.
   - For layout fixes, record the command checks run alongside Computer Use evidence: typically `cargo fmt`, app-focused tests such as `cargo test -p <app>`, app-focused clippy such as `cargo clippy -p <app> --all-targets --all-features -- -D warnings`, and the bundle command when packaging was part of the validation.

8. Clean up test data.
   - Follow the `清理` section in the relevant test case.
   - Delete temporary databases, mock service data, generated exports, screenshots, and placeholder configs after validation.
   - Never delete user real data as part of cleanup unless the user explicitly requested that exact destructive operation.

## Practical Checks

- Dialog width: confirm controls are not clipped at the right edge.
- Dialog height: confirm body scroll works and footer remains reachable.
- Nested scrolling: test both dialog body scroll and editor/content scroll when code editors or text views are involved.
- Footer buttons: confirm natural button width and right alignment rather than full-row stretching.
- Content density: compare labels, select controls, dividers, and repeated metadata against the user screenshot.
- Tables/lists: verify visible row counts, horizontal/vertical scrolling, row actions, and whether fixed columns remain reachable.
- Popovers/menus: verify open position, clipping, keyboard dismissal, and focus return.
- Destructive flows: open confirmation dialogs but do not confirm deletion unless the user explicitly asks for the destructive action.
- Data safety: if a test involves delete, clear, overwrite, regenerate, crawl, export, or shortcut registration, confirm the target is test data before triggering the action.
- Secret handling: never type, screenshot, log, or quote real API keys or real Cookies; use documented placeholders such as `sk-test-redacted-value` and `qa_session=redacted-test-cookie`.

## Reporting

Summarize validation in concrete terms:

- artifact launched, for example `target/release/bundle/macos/Jaco.app`
- Computer Use app path confirmed from `get_app_state`; explicitly note if it was not the target local bundle
- window or dialog inspected
- interactions performed, for example click edit, scroll down, focus editor, press `Page_Down`
- QA doc or test case followed, when applicable
- test data isolation method used, including whether data was pre-seeded
- commands run and whether they passed
- warnings that did not block the bundle, especially `actool` icon injection warnings
