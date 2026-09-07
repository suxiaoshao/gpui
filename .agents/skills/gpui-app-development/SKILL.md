---
name: gpui-app-development
description: Route implementation, review, or debugging of GPUI workspace apps and shared app-support code to relevant project conventions and skills.
---

# GPUI App Development

## Skill routing

Select the entry for the responsibility being changed or deliberately integrated:

| Responsibility | Skill |
| --- | --- |
| Framework contexts, entities, tasks, events, focus, rendering and tests | `gpui` |
| Controls, component APIs and Web UI translation | `gpui-component-usage` |
| Icons, runtime assets and bundle assets | `gpui-app-icon-usage` |
| Fluent text, language settings and bundle localization | `gpui-i18n` |
| Shared authoritative state, selection and observation | `gpui-store` |
| Fallible work, refresh, recovery, cancellation and operation state | `gpui-operation` |
| Editable sessions, bindings, validation, rebase and submission | `gpui-form` |
| Runtime UI or lifecycle evidence | `gpui-computer-use-debugging` |

Nearby mentions of state, async work or editable data do not require integrating these libraries.

## Ownership and state

- Apps use established `app`, `foundation`, `features` and `state` boundaries, starting at `app/{name}/src/main.rs`. Existing construction and state flows establish ownership.
- Keep product policy in its app. Shared behavior belongs in a matching crate such as `window-ext`, `platform-ext`, `app-theme` or `app-assets` when reusable across apps.
- Keep one authoritative source per fact. Compute cheap derived values; a stored cache needs a source, invalidation triggers, update ordering and stale-update handling.
- Let the runtime state, such as `Option<Task>` or an operation variant, express activity instead of parallel `is_loading` or `agent_running` fields.
- Current public docs and exported code establish implemented contracts; development plans describe target architecture only where the focused skill identifies them.

## GPUI dependency entrypoints

Applications use `gpui_kit` (0.6.0), `gpui_kit::component`, `gpui_kit::assets`,
`gpui_kit::application()` and `gpui_kit::init(cx)`. Shared crates may retain
`gpui`/`gpui_component` aliases; the workspace resolves a single `gpui-pre` 0.3.3.
