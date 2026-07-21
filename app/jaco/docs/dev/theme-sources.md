# Jaco bundled theme sources

Jaco embeds the JSON presets from `longbridge/gpui-component` so themes remain available without
network access.

## Current snapshot

- Source repository: `https://github.com/longbridge/gpui-component`
- Source commit: `5b45bcb26b9343d91a123a4d5ed8a654360512e5`
- Source directory: `themes/`
- Local directory: `app/jaco/assets/themes/gpui-component/`
- Inventory: the 22 upstream JSON files, including `aurora.json`

## Jaco overlay

The previous Jaco tab presentation overlay is a fallback, not an unconditional patch. Evaluate each
theme variant from the locked upstream snapshot independently. If upstream explicitly distinguishes
active and inactive tabs through different background or foreground values, keep all five upstream
tab values. Otherwise, reapply the existing Jaco values for that variant when a historical overlay
exists. The allowlisted keys are `tab.active.background`, `tab.active.foreground`, `tab.background`,
`tab.foreground`, and `tab_bar.background`; never mix upstream and local values within those five
keys, and do not add other product-specific changes.

At `5b45bcb`, the historical overlay remains only for these variants because the upstream snapshot
does not provide a complete active/inactive distinction:

- Ayu Dark
- Catppuccin Macchiato
- Catppuccin Mocha
- Fahrenheit
- macOS Classic Dark
- Molokai Light
- Molokai Dark
- Spaceduck
- Tokyo Night
- Tokyo Storm
- Tokyo Moon

Every other variant uses the five upstream tab values as a single group. All fields outside this
allowlist are byte-for-value equivalent to the locked upstream JSON after parsing.

## Updating the snapshot

For a later gpui-component upgrade, start from the complete `themes/` directory at the new locked
commit, then apply the per-variant rule above: prefer a new upstream active/inactive distinction and
use the historical allowlisted overlay only when upstream still lacks one. Verify the exact filename
inventory, parse every theme set, compare all non-overlay values with upstream, and confirm that
Aurora gradients remain renderable `ThemeToken` backgrounds.
