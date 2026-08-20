---
name: gpui-base-usage
description: Use gpui-base in GPUI applications that deliberately own their visual design system. Applies to base-only primitives, initialization, input/editor behavior, accessibility ownership, and custom text selection; do not use it as a styled gpui-component substitute.
---

# GPUI Base Usage

Use this skill only after confirming that the target app directly depends on
`gpui-base` and intentionally does not use the complete `gpui-component`
presentation layer.

## Workflow

1. Inspect the app manifest and dependency tree. A base-only app must not gain
   `gpui-component`, `gpui-component-assets`, or a shared theme/assets crate
   that pulls them in transitively.
2. Verify APIs against the exact pinned `longbridge/gpui-component` revision.
   This skill is a repository-authored guide; upstream does not ship a
   `skills/gpui-base` directory.
3. Call `gpui_base::init(cx)` once before opening app windows. Do not also call
   `gpui_component::init(cx)` in a base-only app.
4. Choose the smallest behavior primitive, keep its identity and controlled
   state stable, then supply all product styling in the application.
5. Read only the relevant reference below. Validate behavior, focus, keyboard,
   accessibility, and platform interaction rather than treating compilation as
   proof of a complete control.

## Boundaries

- Base owns reusable interaction behavior and infrastructure. The application
  owns visual tokens, layout, icons, chrome, product policy, and domain state.
- A base primitive is not guaranteed to expose every styled component slot or
  every accessibility semantic. Inspect its target implementation.
- Do not copy a styled component into the app. If base lacks a behavior
  primitive, keep the missing surface app-specific or contribute it upstream.
- Use the `gpui` skill for contexts, Entity, tasks, actions, focus, windows,
  custom Elements, and tests.

## References

- Architecture and dependency boundary: `references/architecture-and-boundary.md`
- Primitive ownership and composition: `references/primitives.md`
- Input, Textarea, and Editor: `references/input-textarea-editor.md`
- Accessibility responsibility: `references/accessibility.md`
- Window-level selection for custom renderers: `references/text-selection.md`
