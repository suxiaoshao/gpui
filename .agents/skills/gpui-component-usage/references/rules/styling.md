# Styling Rules

Use this file when translating a design, shadcn/ui example, or existing Web mental model into GPUI and gpui-component.

## Mental Model

- GPUI's layout and style methods are similar in spirit to Tailwind CSS utilities: small composable calls build the final layout.
- gpui-component follows shadcn/ui for many component structures, sizes, variants, and visual states.
- The implementation language is still GPUI: use `div()`, flex helpers, `Styled`, theme access, component builders, and existing app view patterns.

## Desktop Differences

- Prefer desktop cursor behavior. Buttons generally keep the default cursor unless the component is a link-like affordance.
- Preserve keyboard navigation and focus behavior. Do not let a visual translation break tab order, focus handles, or escape/enter behavior.
- Use the GPUI root/window overlay model for dialogs, sheets, notifications, and popovers.
- Respect macOS and Windows control expectations when they conflict with Web habits.

## Theme and Color

- Prefer theme colors from `cx.theme()` and gpui-component variants over hard-coded colors.
- Use semantic component variants before custom style overrides.
- Keep custom style overrides narrow and local to the app feature.
- When translating Tailwind examples, convert spacing, sizing, typography, and color intent into GPUI style calls rather than preserving class names.

## Visual Hierarchy

- Build hierarchy with spacing, alignment, size, contrast, and component variants.
- Avoid unnecessary card-like nesting in product surfaces.
- Use restrained visual accents. Do not add decorative gradients or multiple competing emphasis colors unless the app already uses that language.
- Keep text sizing appropriate to the UI surface: compact tool panels and dialogs should not use hero-scale headings.

## Version Drift

The bundled component docs are a portable snapshot. If the current dependency exposes a different API, trust the actual dependency source or docs.rs for the exact version, then adapt the app code to the installed API.
