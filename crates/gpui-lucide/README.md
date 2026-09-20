# gpui-lucide

Lucide SVG bytes that work directly with GPUI Kit's `Into<Icon>` controls:

```rust
use gpui_kit::component::{Icon, Sizable, button::Button};
use gpui_lucide::{IconName, SvgIcon};

let button = Button::new("search").icon(IconName::Search);
let icon = Icon::new(IconName::Brain).small();
let logo = SvgIcon::new(include_bytes!("../assets/logo.svg"));
```

`SvgIcon` is a copyable handle to static SVG bytes. `IconName` aliases that type;
its associated constants are generated from the complete catalog in the pinned
`gpui-kit-assets` package. It is not an enum dispatching over all SVG payloads.
There is no application selection list, `icon_assets!`, name lookup, asset-path
registration, or global catalog of byte references. Unreferenced constants can
be discarded by the linker. A runtime choice between two constants retains
both, as expected.

Conversion delegates to the upstream `Icon::data`, so button sizing, theme
colors, transformations, native icon handling and cloning remain owned by GPUI
Kit. The upstream conversion copies bytes into shared storage; a static source
does not mean rendering or creating an `Icon` is allocation-free. `SvgIcon` can
also be passed directly as an element or converted to an entity with `.view(cx)`.

The build script uses the public `icons-dir` Cargo metadata from
`gpui-kit-assets`. It does not access that crate's private Rust module, download
files at build time, or read this repository's Lucide submodule. The upstream
package retains its Lucide licenses and supplies the SVG catalog.

Keep `gpui_kit::assets::Assets` registered for GPUI Component's own default
icons. Gupi's custom brand assets likewise remain application-owned. No extra
asset registration is needed for this crate's icons or `SvgIcon::new`.

## Verification

`cargo test -p gpui-lucide --locked` checks custom and built-in SVG conversion
and the `Button::icon` integration. The `one_icon` and `two_icons` examples
exercise the actual upstream `Icon` conversion while referencing one or two
SVGs, for inspecting linker selection without launching a window. Binary sizes
depend on the toolchain and dependency build profile; they are not per-icon
size promises.

On 2026-09-20, the macOS arm64 examples were built with dev dependencies and
`cargo rustc --example <name> -- -C opt-level=3 -C debuginfo=0`.
Searching each binary for every one of the upstream package's 1,830 complete
SVG payloads found only `search` in `one_icon`, and only `brain`/`search` in
`two_icons`. Both executables ran successfully. This checks real `Into<Icon>`
linkage, not just a standalone mock of the generated constants.
