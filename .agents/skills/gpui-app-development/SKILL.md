---
name: gpui-app-development
description: Route GPUI application work in this workspace to the correct repo-local conventions and focused skills. Use when implementing, refactoring, reviewing, or debugging code under app/ or shared app-support crates, especially when deciding module placement, ownership, shared state, fallible operations, forms, components, resources, localization, or validation.
---

# GPUI App Development

Use this skill as the application-development entrypoint. It identifies which
repo-local skill and source material to read; it does not duplicate crate APIs or
integration contracts.

## Workflow

1. Identify the affected app, feature, owners, dependencies, and consumers.
2. Inspect the target app's current module, construction, state, rendering, and
   test patterns.
3. Select every focused skill that applies from the routing table below.
4. Read each selected skill completely, then follow the public docs, development
   docs, implementation, and tests it directs you to.
5. Implement within the user's requested scope and run only focused validation
   for the changed code.

When multiple libraries apply, load all corresponding skills. Determine their
composition from those skills and the current code; do not infer one library's
responsibilities from this routing skill.

## Skill routing

| Task or decision | Load |
| --- | --- |
| GPUI contexts, windows, Entity, Global, tasks, subscriptions, events, focus, rendering, elements, or tests | `gpui` |
| App module placement, ownership boundaries, cross-feature coordination, or choosing among the libraries below | `gpui-app-development` |
| Existing controls, component APIs, custom control decisions, or GPUI translation of Web UI patterns | `gpui-component-usage` |
| App icons, Lucide icons, runtime assets, or bundle assets | `gpui-app-icon-usage` |
| User-visible text, Fluent locales, language settings, or bundle localization | `gpui-i18n` |
| Shared authoritative in-memory state, selection, observation, or explicit `Store<S>` integration | `gpui-store` |
| Fallible work that loads, refreshes, retries, repairs, cancels, retains stale data, or exposes operation state to UI | `gpui-operation` |
| Typed editable data, validation, bound controls, form projection, rebasing, or submit preparation | `gpui-form` |
| Running or inspecting the desktop app to diagnose visible or lifecycle behavior | `gpui-computer-use-debugging` |

Do not load `gpui-store`, `gpui-operation`, or `gpui-form` merely because a
related concept appears in nearby code. Load them when the task changes or
deliberately integrates that library's responsibility.

## Workspace routing

- Start from `app/{name}/src/main.rs`, then follow the target app's established
  `app`, `foundation`, `features`, and `state` boundaries.
- Inspect existing app code before introducing a new owner, Global, Store,
  operation, form, service, or shared abstraction.
- Search focused shared crates such as `crates/window-ext`,
  `crates/platform-ext`, `crates/app-theme`, and `crates/app-assets` before
  duplicating cross-app behavior.
- Keep app-specific product policy in the app. Move behavior to a shared crate
  only when it is genuinely reusable.
- Treat the selected crate's current public docs and exported code as its
  contract. Use its development docs for target architecture when the focused
  skill explicitly directs you there.

## Validation routing

- Follow the validation section of every selected focused skill.
- Run `cargo fmt` for Rust code changes and the most focused app or crate checks
  that cover the changed behavior.
- Use `gpui-computer-use-debugging` only when runtime UI validation is requested
  or necessary for the specific behavior.
- For changes only to this skill, validate its structure and run
  `git diff --check`; crate tests are unnecessary.
