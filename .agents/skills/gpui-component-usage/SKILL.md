---
name: gpui-component-usage
description: Choose and compose gpui-component controls in GPUI apps, including translation of Web or shadcn UI designs.
---

# GPUI Component Usage

Find the UI role in the [component index](references/components/index.md), then read the selected component reference. Use existing controls, delegates, variants and overlays before adding the feature-specific gap. Apps import `gpui_kit::component`; root Cargo dependencies determine the version.

Bundled references are an upstream snapshot. When an API is uncertain, inspect the actual dependency source and relevant story. If a reference is missing, use the exact-version upstream docs; provenance is recorded in [attribution](references/third-party/gpui-component-docs.md).

Translate Web design intent into GPUI elements and component builders. Preserve desktop cursor, keyboard focus and window/root overlay behavior. Establish hierarchy through spacing, alignment, type and contrast; use existing theme tokens and semantic variants.

- [Traits](references/rules/traits.md) and [primitives](references/rules/primitives.md): find reusable styling and composition APIs.
- [Theme and size](references/rules/theme-and-size.md): theme tokens, variants and sizing.
- [State and interaction](references/rules/state-and-interaction.md): delegates, native state, focus and form projection.
